use std::sync::{Arc, Mutex};

use eventsub::EventSubManager;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use specta_typescript::Typescript;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use token::TokenManager;
use tokio::task;
use tracing::{debug, info};
use twitch_api::{client::ClientDefault, HelixClient};
use twitch_oauth2::tokens::errors::ValidationError;

mod eventsub;
mod token;
mod types;

type SharedUserToken = Mutex<types::UserToken>;
type SharedTwitchToken = Mutex<twitch_oauth2::UserToken>;

#[tauri::command]
#[specta::specta]
fn get_followed_streams(
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Stream>, String> {
    let client = client.inner();

    let token_guard = token.lock().unwrap().clone();
    let streams = tauri::async_runtime::block_on(
        client
            .get_followed_streams(&token_guard)
            .try_collect::<Vec<_>>(),
    )
    .unwrap();

    Ok(streams.into_iter().map(types::Stream::from).collect())
}

#[tauri::command]
#[specta::specta]
fn get_followed_channels(
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Broadcaster>, String> {
    let client = client.inner();

    let token_guard = token.lock().unwrap().clone();
    let channels = tauri::async_runtime::block_on(
        client
            .get_followed_channels(token_guard.user_id.to_string(), &token_guard)
            .try_collect::<Vec<_>>(),
    )
    .unwrap();

    let mut users: Vec<twitch_api::helix::users::User> = Vec::new();
    for chunk in channels.chunks(100) {
        let ids: Vec<twitch_api::types::UserId> =
            chunk.iter().map(|b| b.broadcaster_id.clone()).collect();

        debug!("ids={:?}", ids);
        let mut u: Vec<twitch_api::helix::users::User> = tauri::async_runtime::block_on(
            client
                .get_users_from_ids(&twitch_api::types::Collection::from(ids), &token_guard)
                .try_collect::<Vec<_>>(),
        )
        .unwrap();
        users.append(&mut u);
    }

    Ok(users.into_iter().map(types::Broadcaster::from).collect())
}

#[tauri::command]
#[specta::specta]
async fn login(
    app_handle: AppHandle,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::UserToken, String> {
    let client = client.inner();
    info!("login");

    let mut token_manager: TokenManager;
    let store = app_handle.store("account.json").unwrap();

    if let Some(binding) = store.get("token") {
        let token: types::UserToken = serde_json::from_value(binding.clone()).unwrap();
        token_manager = match token.to_twitch_token(client.clone()).await {
            Ok(token) => {
                TokenManager::from_existing(token.clone(), client.clone(), app_handle.clone())
            }
            Err(ValidationError::NotAuthorized) => {
                TokenManager::new(client.clone(), app_handle.clone()).await
            }
            Err(err) => panic!("{err}"),
        };
    } else {
        token_manager = TokenManager::new(client.clone(), app_handle.clone()).await;
    }

    let user_token = types::UserToken::from_twitch_token(token_manager.clone().twitch_token());

    let eventsub_manager = EventSubManager::create(
        None,
        token_manager.clone().twitch_token().clone(),
        client.clone(),
    );

    task::spawn(async move {
        eventsub_manager
            .run(|e, _ts| async move {
                // self.handle_event(e, ts).await
                debug!("got event {:?}", e);
                Ok(())
            })
            .await
            .expect("eventmanager failed");
    });

    app_handle.manage::<SharedUserToken>(Mutex::new(user_token.clone()));
    app_handle.manage::<SharedTwitchToken>(Mutex::new(token_manager.clone().twitch_token()));

    let app_handle_ref = app_handle.clone();
    let store_ref = store.clone();
    token_manager.on_refresh = Arc::new(Box::new(move |token| {
        let user_token_binding = app_handle_ref.state::<SharedUserToken>();
        let twitch_token_binding = app_handle_ref.state::<SharedTwitchToken>();
        let mut user_token_ref = user_token_binding.lock().unwrap();
        let mut twitch_token_ref = twitch_token_binding.lock().unwrap();

        *twitch_token_ref = token.clone();

        let new_token = types::UserToken::from_twitch_token(token);
        *user_token_ref = new_token.clone();

        debug!(
            new_token.access_token,
            new_token.login, new_token.user_id, "updating token store"
        );

        store_ref.set("token", json!(new_token));
    }));

    debug!("setting token");
    store.set("token", json!(user_token));
    token_manager.manage();

    Ok(user_token)
}

#[derive(Serialize, Deserialize, Type)]
pub struct MyStruct {
    a: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    color_eyre::install().expect("failed to install color_eyre");

    tracing_subscriber::fmt()
        // enable everything
        .with_max_level(tracing::Level::DEBUG)
        // sets this to be the default, global collector for this application.
        .init();

    let builder = tauri::Builder::default().plugin(tauri_plugin_store::Builder::new().build());

    let handlers = tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<MyStruct>()
        .typ::<types::UserToken>()
        // Then register them (separated by a comma)
        .commands(collect_commands![
            get_followed_streams,
            get_followed_channels,
            login,
        ]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    handlers
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::BigInt)
                .header("/* eslint-disable */"),
            "../src/lib/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            info!("{}, {argv:?}, {cwd}", app.package_info().name);
            app.emit("single-instance", argv).unwrap();
        }))
        .plugin(tauri_plugin_window_state::Builder::new().build());

    let _builder = builder
        .invoke_handler(handlers.invoke_handler())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            // This is also required if you want to use events
            handlers.mount_events(app);

            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default());

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let win_builder = win_builder.title("").inner_size(1000.0, 600.0);

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

            let window = win_builder.build().unwrap();

            // set background color only when building for macOS
            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::{NSColor, NSWindow};
                use cocoa::base::{id, nil};

                let ns_window = window.ns_window().unwrap() as id;
                unsafe {
                    //rgb(24, 31, 42)
                    let bg_color = NSColor::colorWithRed_green_blue_alpha_(
                        nil,
                        20.0 / 255.0,
                        26.0 / 255.0,
                        39.0 / 255.0,
                        1.0,
                    );
                    ns_window.setBackgroundColor_(bg_color);
                }
            }

            let client: HelixClient<'static, reqwest::Client> =
                twitch_api::HelixClient::with_client(
                    ClientDefault::default_client_with_name(Some(
                        "pepo".parse().expect("invalid client name"),
                    ))
                    .unwrap(),
                );

            info!("made client");

            app.manage(client);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

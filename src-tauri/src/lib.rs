use eventsub::EventSubManager;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use tauri_svelte_synced_store::{StateSyncer, StateSyncerConfig};
use token::TokenManager;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace};
use twitch_api::{client::ClientDefault, HelixClient};
use twitch_oauth2::tokens::errors::ValidationError;

use crate::badgemanager::BadgeManager;
use crate::emotemanager::EmoteManager;
use crate::types::AuthState;

mod badgemanager;
mod emote;
mod emotemanager;
mod eventsub;
mod message;
mod token;
mod types;

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type, Default)]
struct InternalState {
    version: String,
    name: String,
}

type SharedUserToken = Mutex<types::UserToken>;
type SharedTwitchToken = Mutex<twitch_oauth2::UserToken>;
type SharedEventSubManager = Mutex<EventSubManager>;
type SharedBadgeManager = Mutex<BadgeManager>;
type SharedEmoteManager = Mutex<EmoteManager>;

tauri_svelte_synced_store::state_handlers!(
    AuthState = "auth_state",
    InternalState = "internal_state"
);

#[tauri::command]
#[specta::specta]
fn send_chat_message(
    broadcaster_id: String,
    message: String,
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!(
        "sending chat message: broadcaster_id={}, message={}",
        broadcaster_id, message
    );

    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
    let user_id = token_guard.user_id.clone();
    match tauri::async_runtime::block_on(client.send_chat_message(
        broadcaster_id,
        user_id,
        message.as_str(),
        &token_guard,
    )) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("request failed: {:?}", e)),
    }
}

#[tauri::command]
#[specta::specta]
async fn join_chat(
    channel_name: String,
    _app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    badge_manager_ref: State<'_, SharedBadgeManager>,
    emote_manager_ref: State<'_, SharedEmoteManager>,
    token_ref: State<'_, SharedTwitchToken>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::ChannelInfo, String> {
    debug!("join: channel={}", channel_name);

    let token = token_ref.lock().await;
    let client = client_ref.inner();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();
    let badge_manager = badge_manager_ref.lock().await.clone();
    let emote_manager = emote_manager_ref.lock().await.clone();

    let channel = client
        .get_channel_from_login(&channel_name, &token.clone())
        .await
        .unwrap()
        .expect("missing channel");

    debug!("join: got channel info - {:?}", channel.clone());

    badge_manager
        .load_channel(channel.broadcaster_id.to_string(), client.clone())
        .await;

    emote_manager
        .load_channel(channel.broadcaster_id.to_string(), client.clone())
        .await;

    match eventsub_manager
        .join_chat(
            channel.broadcaster_id.clone(),
            channel_name,
            client,
            token.clone(),
        )
        .await
    {
        Ok(_) => debug!("joined channel"),
        Err(e) => error!("join_chat - {:?}", e),
    };

    Ok(types::ChannelInfo::from(channel))
}

#[tauri::command]
#[specta::specta]
async fn leave_chat(
    channel_name: String,
    _app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    token_ref: State<'_, SharedTwitchToken>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!("leave: channel={}", channel_name);

    let token = token_ref.lock().await;
    let client = client_ref.inner();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();

    match eventsub_manager
        .leave_chat(channel_name, client, token.clone())
        .await
    {
        Ok(_) => debug!("left channel"),
        Err(e) => error!("leave_chat - {:?}", e),
    };

    Ok(())
}

#[tauri::command]
#[specta::specta]
fn get_followed_streams(
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Stream>, String> {
    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
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

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
    let channels = match tauri::async_runtime::block_on(
        client
            .get_followed_channels(token_guard.user_id.to_string(), &token_guard)
            .try_collect::<Vec<_>>(),
    ) {
        Ok(channels) => channels,
        Err(err) => {
            error!("failed to get followed channels: err={}", err.to_string());
            return Err(err.to_string());
        }
    };

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
    state_syncer: State<'_, StateSyncer>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::UserToken, String> {
    let client = client.inner();
    info!("login");

    let mut token_manager: TokenManager;
    let store = app_handle.store("account.json").unwrap();

    if let Some(binding) = store.get("token") {
        debug!("loading token from file store");
        let token: types::UserToken = serde_json::from_value(binding.clone()).unwrap();
        token_manager = match token.to_twitch_token(client.clone()).await {
            Ok(token) => TokenManager::from_existing(token.clone(), client.clone()),
            Err(ValidationError::NotAuthorized) => TokenManager::new(client.clone()),
            Err(err) => panic!("{err}"),
        };
    } else {
        debug!("no token found on disk, setting empty token");
        token_manager = TokenManager::new(client.clone());
    }

    let twitch_token: twitch_oauth2::UserToken;
    let user_token: types::UserToken;
    if token_manager.clone().is_token_valid().await {
        debug!("token is already valid");
        twitch_token = token_manager.clone().get_token().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    } else {
        let device_code = token_manager.clone().start_device_code_flow().await;

        {
            let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
            let mut auth_state = auth_state_ref.lock().unwrap();

            auth_state.phase = types::AuthPhase::WaitingForAuth;
            auth_state.device_code = device_code.user_code;
        }

        debug!("pausing to show verification code");
        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("login {}", device_code.verification_uri);
        app_handle
            .opener()
            .open_url(device_code.verification_uri.clone(), None::<&str>)
            .unwrap();

        twitch_token = token_manager.clone().finish_device_code_flow().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    }

    {
        let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
        let mut auth_state = auth_state_ref.lock().unwrap();

        auth_state.phase = types::AuthPhase::Authorized;
        auth_state.token = Some(user_token.clone());
    }

    let eventsub_manager = EventSubManager::new();
    let client_ref = client.clone();
    let twitch_token_ref = twitch_token.clone();
    let badge_manager = BadgeManager::new(client_ref.clone(), twitch_token_ref.clone())
        .await
        .expect("unable to create badge manager");
    let emote_manager = EmoteManager::new(client_ref.clone(), twitch_token_ref.clone())
        .expect("unable to create emote manager");

    let events = eventsub_manager
        .clone()
        .start(client_ref.clone(), twitch_token_ref.clone())
        .expect("unable to start eventsubmanager");

    app_handle.manage::<SharedEventSubManager>(Mutex::new(eventsub_manager.clone()));
    app_handle.manage::<SharedBadgeManager>(Mutex::new(badge_manager.clone()));
    app_handle.manage::<SharedEmoteManager>(Mutex::new(emote_manager.clone()));

    let app_ref = app_handle.clone();
    let badge_manager_ref = badge_manager.clone();
    let emote_manager_ref = emote_manager.clone();
    std::thread::spawn(move || {
        use twitch_api::eventsub::{Message as M, Payload as P};

        for msg in events {
            match msg.event {
                twitch_api::eventsub::Event::ChannelChatMessageV1(P {
                    message: M::Notification(chat_message),
                    ..
                }) => {
                    let channel_msg = types::ChannelMessage::new(
                        chat_message.clone(),
                        msg.ts.to_string(),
                        badge_manager_ref.clone(),
                        emote_manager_ref.clone(),
                    );
                    let key = format!("chat_message:{}", chat_message.broadcaster_user_login);
                    trace!("chat message: id={} msg={:?}", key, channel_msg);
                    app_ref
                        .emit(&key, channel_msg)
                        .expect("unable to emit state")
                }
                _ => debug!("event notification: {:?}", msg.event),
            }
        }
    });

    app_handle.manage::<SharedUserToken>(Mutex::new(user_token.clone()));
    app_handle.manage::<SharedTwitchToken>(Mutex::new(twitch_token));

    let app_handle_ref = app_handle.clone();
    let store_ref = store.clone();
    token_manager.on_refresh = Arc::new(Box::new(move |token| {
        let user_token_binding = app_handle_ref.state::<SharedUserToken>();
        let twitch_token_binding = app_handle_ref.state::<SharedTwitchToken>();

        let mut user_token_ref = tauri::async_runtime::block_on(user_token_binding.lock());
        let mut twitch_token_ref = tauri::async_runtime::block_on(twitch_token_binding.lock());

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    color_eyre::install().expect("failed to install color_eyre");

    tracing_subscriber::fmt()
        // enable everything
        .with_max_level(tracing::Level::DEBUG)
        // sets this to be the default, global collector for this application.
        .init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build());

    let handlers = tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<types::UserToken>()
        .typ::<types::ChannelMessage>()
        .typ::<types::AuthState>()
        .typ::<types::AuthPhase>()
        // Then register them (separated by a comma)
        .commands(collect_commands![
            get_followed_streams,
            get_followed_channels,
            join_chat,
            leave_chat,
            login,
            send_chat_message,
            emit_state,
            update_state,
        ]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    handlers
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::Number)
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build());

    let _builder = builder
        .invoke_handler(handlers.invoke_handler())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .unwrap();

            // This is also required if you want to use events
            handlers.mount_events(app);

            let mut internal_state = InternalState::default();
            internal_state.version = app.package_info().version.to_string();
            internal_state.name = app.package_info().name.to_string();

            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default());

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

            let window = win_builder.build().unwrap();

            let _ = window.set_title(&format!(
                "{} {}",
                internal_state.name.clone(),
                internal_state.version.clone()
            ));

            // set background color only when building for macOS
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSColor, NSWindow};

                unsafe {
                    let ns_window = &*(window.ns_window().unwrap() as *mut NSWindow);

                    //rgb(24, 31, 42)
                    let bg_color = NSColor::colorWithRed_green_blue_alpha(
                        20.0 / 255.0,
                        26.0 / 255.0,
                        39.0 / 255.0,
                        1.0,
                    );
                    ns_window.setBackgroundColor(bg_color.downcast_ref());
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

            let state_syncer = StateSyncer::new(StateSyncerConfig::default(), app.handle().clone());

            state_syncer.set("auth_state", AuthState::default());
            state_syncer.set("internal_state", internal_state);
            app.manage::<StateSyncer>(state_syncer);

            app.manage(client);

            #[cfg(debug_assertions)]
            window.open_devtools();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

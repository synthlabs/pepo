use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use specta_typescript::Typescript;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{Emitter, Manager};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use token::TokenManager;
use twitch_api::{client::ClientDefault, HelixClient};
use types::UserToken;

mod token;
mod types;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
#[specta::specta]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta::specta]
async fn login(app_handle: tauri::AppHandle) -> Result<types::UserToken, String> {
    println!("login");

    let client: HelixClient<'static, reqwest::Client> = twitch_api::HelixClient::with_client(
        ClientDefault::default_client_with_name(Some("pepo".parse().expect("invalid client name")))
            .unwrap(),
    );

    println!("made client");

    let mut token_manager: TokenManager;
    let store = app_handle.store("account.json").unwrap();

    if let Some(binding) = store.get("token") {
        let token: UserToken = serde_json::from_value(binding.clone()).unwrap();
        let token = token.to_twitch_token(client.clone()).await.unwrap();
        token_manager = TokenManager::from_existing(token.clone(), client.clone());
    } else {
        token_manager = TokenManager::new(client, app_handle.clone()).await;
    }

    let user_token = types::UserToken::from_twitch_token(token_manager.clone().user_token());

    // TODO: delete
    println!("{:#?}", user_token);

    app_handle.manage(Mutex::new(user_token.clone()));

    let app_handle_ref = app_handle.clone();
    let store_ref = store.clone();
    token_manager.on_refresh = Arc::new(Box::new(move |token| {
        let binding = app_handle_ref.state::<Mutex<types::UserToken>>();
        let mut user_token_ref = binding.lock().unwrap();
        let new_token = UserToken::from_twitch_token(token);
        *user_token_ref = new_token.clone();

        println!("updating token store");
        store_ref.set("token", json!(new_token));
    }));

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
    let builder = tauri::Builder::default().plugin(tauri_plugin_store::Builder::new().build());

    let handlers = tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<MyStruct>()
        .typ::<types::UserToken>()
        // Then register them (separated by a comma)
        .commands(collect_commands![greet, login,]);

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
            println!("{}, {argv:?}, {cwd}", app.package_info().name);
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

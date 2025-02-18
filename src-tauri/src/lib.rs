use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{Emitter, Manager};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::collect_commands;
use twitch_api::{client::ClientDefault, HelixClient};
use twitch_oauth2::Scope;

mod types;

const CLIENT_ID: &str = "46oisb828x3q9lu42ctuhphonrlho9";

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

    let client: HelixClient<reqwest::Client> = twitch_api::HelixClient::with_client(
        ClientDefault::default_client_with_name(Some("pepo".parse().expect("invalid client name")))
            .unwrap(),
    );

    println!("made client");

    // First we need to get a token, preferably you'd also store this information somewhere safe to reuse when restarting the application.
    // For now we'll just get a new token every time the application starts.
    // One way to store the token is to store the access_token and refresh_token in a file and load it when the application starts with
    // `twitch_oauth2::UserToken::from_existing`
    let mut builder = twitch_oauth2::tokens::DeviceUserTokenBuilder::new(
        CLIENT_ID.to_string(),
        vec![
            Scope::UserReadChat,
            Scope::UserWriteChat,
            Scope::UserReadFollows,
            Scope::UserReadEmotes,
            Scope::UserReadBlockedUsers,
            Scope::UserReadSubscriptions,
        ],
    );

    println!("made token builder");

    let code = match builder.start(&client).await {
        Ok(code) => code,
        Err(err) => return Err(err.to_string()),
    };

    println!("Please go to: {}", code.verification_uri);
    app_handle
        .opener()
        .open_url(code.verification_uri.clone(), None::<&str>)
        .unwrap();

    let token = builder
        .wait_for_code(&client, tokio::time::sleep)
        .await
        .map_err(|err| err.to_string())?;

    let user_token = types::UserToken::from_twitch_token(token.clone());
    println!("{:#?}", user_token);

    app_handle.manage(Mutex::new(user_token.clone()));

    Ok(user_token)
}

#[derive(Serialize, Deserialize, Type)]
pub struct MyStruct {
    a: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

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

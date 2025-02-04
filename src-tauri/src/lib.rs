use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::Emitter;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_specta::collect_commands;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
#[specta::specta]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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
        // Then register them (separated by a comma)
        .commands(collect_commands![greet,]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    handlers
        .export(Typescript::default(), "../src/lib/bindings.ts")
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
            let win_builder = win_builder.title("").inner_size(1200.0, 600.0);

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

pub const COMMANDS: &[&str] = &["internal_ping"];

#[cfg(feature = "runtime")]
mod runtime {
    use tauri::{plugin::TauriPlugin, Runtime};
    use tauri_specta::collect_commands;

    #[tauri::command]
    #[specta::specta]
    fn internal_ping() -> String {
        "pong from internal".to_owned()
    }

    fn init<R: Runtime>() -> TauriPlugin<R> {
        tauri::plugin::Builder::<R>::new("internal")
            .invoke_handler(tauri::generate_handler![internal_ping])
            .build()
    }

    pub fn apply_plugins<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
        builder.plugin(init())
    }

    #[cfg(debug_assertions)]
    pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
        tauri_specta::Builder::<tauri::Wry>::new()
            .plugin_name("internal")
            .commands(collect_commands![internal_ping])
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(feature = "runtime")]
pub use runtime::{apply_plugins, setup};

#[cfg(all(debug_assertions, feature = "runtime"))]
pub use runtime::specta_builder;

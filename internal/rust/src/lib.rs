pub const COMMANDS: &[&str] = &["internal_ping"];

#[cfg(feature = "runtime")]
mod runtime {
    use tauri::{plugin::TauriPlugin, Runtime};

    #[cfg(debug_assertions)]
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

    pub struct InternalBuildInfo {
        pub app_version: String,
        pub app_commit: String,
        pub build_time: String,
    }

    pub fn apply_plugins<R: Runtime>(
        builder: tauri::Builder<R>,
        _build_info: InternalBuildInfo,
    ) -> tauri::Builder<R> {
        builder.plugin(init())
    }

    pub fn detect_language(
        _app_handle: tauri::AppHandle,
        _channel_login: &str,
        _message_id: &str,
        _text: &str,
    ) {
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
pub use runtime::{apply_plugins, detect_language, setup, InternalBuildInfo};

#[cfg(all(debug_assertions, feature = "runtime"))]
pub use runtime::specta_builder;

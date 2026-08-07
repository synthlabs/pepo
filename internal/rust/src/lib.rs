include!("../commands.rs");

#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "runtime", derive(specta::Type))]
pub struct InternalBuildInfo {
    pub app_version: String,
    pub app_commit: String,
    pub build_time: String,
}

#[cfg(feature = "runtime")]
mod runtime {
    use super::InternalBuildInfo;
    use tauri::State;
    use tauri::{plugin::TauriPlugin, Manager, Runtime};

    #[cfg(debug_assertions)]
    use tauri_specta::collect_commands;

    #[tauri::command]
    #[specta::specta]
    fn internal_build_info(build_info: State<'_, InternalBuildInfo>) -> InternalBuildInfo {
        build_info.inner().clone()
    }

    fn init<R: Runtime>(build_info: InternalBuildInfo) -> TauriPlugin<R> {
        tauri::plugin::Builder::<R>::new("internal")
            .invoke_handler(tauri::generate_handler![internal_build_info])
            .setup(move |app, _api| {
                app.manage(build_info);
                Ok(())
            })
            .build()
    }

    pub fn apply_plugins<R: Runtime>(
        builder: tauri::Builder<R>,
        build_info: InternalBuildInfo,
    ) -> tauri::Builder<R> {
        builder.plugin(init(build_info))
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
            .commands(collect_commands![internal_build_info])
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(feature = "runtime")]
pub use runtime::{apply_plugins, detect_language, setup};

#[cfg(all(debug_assertions, feature = "runtime"))]
pub use runtime::specta_builder;

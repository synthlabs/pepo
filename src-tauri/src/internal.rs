#[cfg(internal_enabled)]
mod local {
    pub use pepo_internal::{apply_plugins, detect_language, setup, InternalBuildInfo};

    #[cfg(debug_assertions)]
    pub use pepo_internal::specta_builder;
}

#[cfg(not(internal_enabled))]
mod local {
    #[allow(dead_code)]
    pub struct InternalBuildInfo {
        pub app_version: String,
        pub app_commit: String,
        pub build_time: String,
    }

    pub fn apply_plugins<R: tauri::Runtime>(
        builder: tauri::Builder<R>,
        _build_info: InternalBuildInfo,
    ) -> tauri::Builder<R> {
        builder
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn detect_language(_channel_login: &str, _message_id: &str, _text: &str) {}
}

pub use local::{apply_plugins, detect_language, setup, InternalBuildInfo};

#[cfg(all(debug_assertions, internal_enabled))]
pub use local::specta_builder;

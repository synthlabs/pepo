#[cfg(internal_enabled)]
mod local {
    pub use pepo_internal::{apply_plugins, detect_language, setup};

    #[cfg(debug_assertions)]
    pub use pepo_internal::specta_builder;
}

#[cfg(not(internal_enabled))]
mod local {
    pub fn apply_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
        builder
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn detect_language(_text: &str) {}
}

pub use local::{apply_plugins, detect_language, setup};

#[cfg(all(debug_assertions, internal_enabled))]
pub use local::specta_builder;

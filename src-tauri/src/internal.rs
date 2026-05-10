#[cfg(internal_enabled)]
mod local {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../internal/rust/mod.rs"
    ));
}

#[cfg(not(internal_enabled))]
mod local {
    pub fn extend_specta(
        builder: tauri_specta::Builder<tauri::Wry>,
    ) -> tauri_specta::Builder<tauri::Wry> {
        builder
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub use local::{extend_specta, setup};

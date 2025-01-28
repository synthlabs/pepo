#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
        #[cfg(desktop)]
        app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
        #[cfg(desktop)]
        app.handle().plugin(tauri_plugin_window_state::Builder::new().build())?;
        Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

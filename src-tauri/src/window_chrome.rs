use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

pub const BACKGROUND_COLOR: tauri::webview::Color = tauri::webview::Color(20, 26, 39, 255);

pub fn configure<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "linux")]
    let builder = builder.theme(Some(tauri::Theme::Dark));
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
    builder.background_color(BACKGROUND_COLOR)
}

pub fn apply(_window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(handle) = _window.ns_window() {
        use objc2_app_kit::{NSColor, NSWindow};

        unsafe {
            let window = &*(handle as *mut NSWindow);
            let color = NSColor::colorWithRed_green_blue_alpha(
                f64::from(BACKGROUND_COLOR.0) / 255.0,
                f64::from(BACKGROUND_COLOR.1) / 255.0,
                f64::from(BACKGROUND_COLOR.2) / 255.0,
                1.0,
            );
            window.setBackgroundColor(color.downcast_ref());
        }
    }

    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = _window.hwnd() {
        use windows::Win32::Foundation::COLORREF;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        };

        unsafe {
            let use_dark_mode: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark_mode as *const i32 as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );

            let caption_color = COLORREF(
                u32::from(BACKGROUND_COLOR.0)
                    | (u32::from(BACKGROUND_COLOR.1) << 8)
                    | (u32::from(BACKGROUND_COLOR.2) << 16),
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &caption_color as *const COLORREF as *const std::ffi::c_void,
                std::mem::size_of::<COLORREF>() as u32,
            );
        }
    }
}

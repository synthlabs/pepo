use super::*;
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{POINT, RECT},
    Graphics::{
        Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        Gdi::{GetDC, GetPixel, ReleaseDC},
    },
    UI::{
        Input::KeyboardAndMouse::{mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP},
        WindowsAndMessaging::{GetCursorPos, SetCursorPos},
    },
};

fn caption_color(window: &WebviewWindow) -> u32 {
    let mut bounds = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            window.hwnd().unwrap(),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut bounds as *mut RECT as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .unwrap();
        let dc = GetDC(None);
        assert!(!dc.0.is_null());
        // Sample above the title text, inside the visible caption border.
        let color = GetPixel(dc, (bounds.left + bounds.right) / 2, bounds.top + 6);
        ReleaseDC(None, dc);
        color.0
    }
}

#[track_caller]
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "native window condition timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_caption_color(window: &WebviewWindow, expected: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let actual = caption_color(window);
        if actual == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{} caption was {actual:#010x}, expected {expected:#010x}; foreground={}",
            window.label(),
            native_foreground(window)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct RestorePointer(POINT);

impl Drop for RestorePointer {
    fn drop(&mut self) {
        unsafe {
            mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
            let _ = SetCursorPos(self.0.x, self.0.y);
        }
    }
}

fn drag(x: i32, y: i32, start: impl FnOnce()) {
    unsafe {
        SetCursorPos(x, y).unwrap();
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
    }
    std::thread::sleep(Duration::from_millis(80));
    start();
    std::thread::sleep(Duration::from_millis(100));
    unsafe { SetCursorPos(x + 30, y + 20).unwrap() };
    std::thread::sleep(Duration::from_millis(80));
    unsafe { mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0) };
    std::thread::sleep(Duration::from_millis(100));
}

#[test]
#[ignore = "opens temporary windows and drives the pointer; requires an interactive Windows desktop"]
fn native_usercard_focus_and_initial_sizing() {
    let app = tauri::Builder::default()
        .any_thread()
        .manage(UserCardRuntime::default())
        .build(tauri::generate_context!())
        .unwrap();
    let handle = app.handle().clone();
    let worker = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut point = POINT::default();
            unsafe { GetCursorPos(&mut point).unwrap() };
            let _pointer = RestorePointer(point);
            let parent = crate::window_chrome::configure(WebviewWindowBuilder::new(
                &handle,
                "user-card-check-parent",
                WebviewUrl::External("about:blank".parse().unwrap()),
            ))
            .title("Pepo usercard verification")
            .inner_size(600.0, 400.0)
            .position(100.0, 100.0)
            .build()
            .unwrap();
            crate::window_chrome::apply(&parent);
            parent.set_focus().unwrap();
            wait_for(|| native_foreground(&parent));
            assert_caption_color(&parent, 0x00271A14);

            for chrome in [UserCardChrome::Compact, UserCardChrome::Native] {
                let label = format!("user-card-check-{chrome:?}");
                let builder = WebviewWindowBuilder::new(
                    &handle,
                    &label,
                    WebviewUrl::External("about:blank".parse().unwrap()),
                )
                .title(&label)
                .inner_size(360.0, 240.0)
                .min_inner_size(320.0, MIN_CARD_HEIGHT)
                .decorations(chrome == UserCardChrome::Native)
                .position(200.0, 200.0)
                .focused(false)
                .visible(false);
                let builder = if chrome == UserCardChrome::Native {
                    crate::window_chrome::configure(builder)
                } else {
                    builder
                };
                let card = builder.build().unwrap();
                if chrome == UserCardChrome::Native {
                    crate::window_chrome::apply(&card);
                }
                let runtime = handle.state::<UserCardRuntime>();
                {
                    let mut registry = runtime.registry.lock().unwrap();
                    insert(&mut registry, &label, target(Some("1"), "maya", "room"), 1);
                    let entry = registry.cards.get_mut(&label).unwrap();
                    entry.native_ready = true;
                    entry.state.chrome = chrome;
                    entry.initial_placement = Some(InitialPlacement {
                        parent: parent.label().into(),
                        anchor: CardAnchor::default(),
                        mode: UserCardPlacement::Os,
                    });
                }
                let events = Arc::new(Mutex::new(Vec::new()));
                let observed = events.clone();
                let event_card = card.clone();
                let event_handle = handle.clone();
                let event_label = label.clone();
                card.on_window_event(move |event| {
                    if let WindowEvent::Focused(focused) = event {
                        let active = native_foreground(&event_card);
                        observed.lock().unwrap().push((*focused, active));
                        let close = event_handle
                            .state::<UserCardRuntime>()
                            .registry
                            .lock()
                            .unwrap()
                            .focus(&event_label, *focused, active, true);
                        if close {
                            event_handle
                                .state::<UserCardRuntime>()
                                .registry
                                .lock()
                                .unwrap()
                                .cards
                                .remove(&event_label);
                            let _ = event_card.close();
                        }
                    }
                });
                let ready = tauri::async_runtime::block_on(ready_user_card(
                    handle.clone(),
                    card.clone(),
                    172.0,
                ));
                assert!(
                    ready.is_ok(),
                    "first show failed: {ready:?}; focus events: {:?}",
                    events.lock().unwrap()
                );
                wait_for(|| native_foreground(&card));
                if chrome == UserCardChrome::Native {
                    assert_caption_color(&card, 0x00271A14);
                    assert_eq!(caption_color(&card), caption_color(&parent));
                }
                let scale = card.scale_factor().unwrap();
                wait_for(|| {
                    (card.inner_size().unwrap().height as f64 / scale - 172.0).abs() <= 1.0
                });
                card.set_size(tauri::LogicalSize::new(360.0, 300.0))
                    .unwrap();
                wait_for(|| {
                    (card.inner_size().unwrap().height as f64 / scale - 300.0).abs() <= 1.0
                });
                tauri::async_runtime::block_on(ready_user_card(
                    handle.clone(),
                    card.clone(),
                    140.0,
                ))
                .unwrap();
                assert!((card.inner_size().unwrap().height as f64 / scale - 300.0).abs() <= 1.0);

                for x in [12, 120, 270] {
                    wait_for(|| native_foreground(&card));
                    let origin = card.outer_position().unwrap();
                    drag(origin.x + x, origin.y + 12, || {
                        card.start_dragging().unwrap()
                    });
                    assert!(
                        runtime.registry.lock().unwrap().cards.contains_key(&label),
                        "{label} closed while dragging at x={x}; focus events: {:?}",
                        events.lock().unwrap()
                    );
                }
                let origin = card.outer_position().unwrap();
                let size = card.outer_size().unwrap();
                let before_resize = card.inner_size().unwrap();
                drag(
                    origin.x + size.width as i32 - 2,
                    origin.y + size.height as i32 - 2,
                    || {},
                );
                wait_for(|| card.inner_size().unwrap() != before_resize);
                assert!(
                    runtime.registry.lock().unwrap().cards.contains_key(&label),
                    "{label} closed during resize; focus events: {:?}",
                    events.lock().unwrap()
                );
                assert!(
                    events.lock().unwrap().contains(&(false, true)),
                    "expected a real WebView blur while the native window remained active"
                );

                runtime
                    .registry
                    .lock()
                    .unwrap()
                    .cards
                    .get_mut(&label)
                    .unwrap()
                    .state
                    .pinned = true;
                parent.set_focus().unwrap();
                wait_for(|| native_foreground(&parent));
                assert!(runtime.registry.lock().unwrap().cards.contains_key(&label));
                card.set_focus().unwrap();
                wait_for(|| native_foreground(&card));
                runtime
                    .registry
                    .lock()
                    .unwrap()
                    .cards
                    .get_mut(&label)
                    .unwrap()
                    .state
                    .pinned = false;
                parent.set_focus().unwrap();
                wait_for(|| !runtime.registry.lock().unwrap().cards.contains_key(&label));
                wait_for(|| handle.get_webview_window(&label).is_none());
            }
        }));
        handle.exit(0);
        result
    });
    app.run_return(|_, _| {});
    if let Err(error) = worker.join().unwrap() {
        std::panic::resume_unwind(error);
    }
}

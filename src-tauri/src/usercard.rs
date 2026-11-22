use crate::chat::{
    runtime::ChatRuntime,
    users::{ObservedUser, UserProfile},
    ChatEngine,
};
use crate::types::{AppearanceTheme, BadgeRef, UserCardChrome, UserCardPlacement};
use crate::{app_settings, network::NetworkRuntime, token::TokenManager, StateSyncer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::watch;

const STATE_EVENT: &str = "user-card-state";
const MIN_CARD_HEIGHT: f64 = 120.0;

#[derive(Clone, Debug, Deserialize, specta::Type)]
pub struct UserCardTarget {
    pub user_id: Option<String>,
    pub login: String,
    pub display_name: String,
    pub channel_id: String,
    pub channel_login: String,
    pub anchor: CardAnchor,
}

#[derive(Clone, Debug, Default, Deserialize, specta::Type)]
pub struct CardAnchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    Loading,
    Ready,
    Unavailable,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
pub struct UserCardState {
    pub user: ObservedUser,
    pub profile_image_url: Option<String>,
    pub channel_id: String,
    pub channel_login: String,
    pub badges: Option<Vec<BadgeRef>>,
    pub profile_status: ProfileStatus,
    pub error: Option<String>,
    pub pinned: bool,
    pub chrome: UserCardChrome,
    pub revision: u64,
}

struct Card {
    target: UserCardTarget,
    state: UserCardState,
    activation: u64,
    armed: bool,
    native_ready: bool,
    initial_placement: Option<InitialPlacement>,
    request: u64,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
}

struct InitialPlacement {
    parent: String,
    anchor: CardAnchor,
    mode: UserCardPlacement,
}

impl Drop for Card {
    fn drop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

#[derive(Default)]
struct Registry {
    cards: HashMap<String, Card>,
}

impl Registry {
    fn find(&self, target: &UserCardTarget) -> Option<String> {
        self.cards
            .iter()
            .find(|(_, card)| match (&target.user_id, &card.target.user_id) {
                (Some(a), Some(b)) => a == b,
                _ => target.login.eq_ignore_ascii_case(&card.target.login),
            })
            .map(|(label, _)| label.clone())
    }

    fn focus(
        &mut self,
        label: &str,
        focused: bool,
        native_active: bool,
        close_on_unfocus: bool,
    ) -> bool {
        let Some(card) = self.cards.get_mut(label) else {
            return false;
        };
        if focused || native_active {
            card.armed = true;
        }
        !focused && !native_active && card.armed && !card.state.pinned && close_on_unfocus
    }

    fn resolve(&mut self, label: &str, id: &str, login: &str) -> Option<String> {
        let card = self.cards.get_mut(label)?;
        card.target.user_id = Some(id.into());
        card.target.login = login.into();
        let other = self
            .cards
            .iter()
            .find(|(other, card)| {
                other.as_str() != label && card.target.user_id.as_deref() == Some(id)
            })
            .map(|(label, _)| label.clone());
        let Some(other) = other else {
            return Some(label.into());
        };
        let incoming = self.cards.remove(label)?;
        let survivor = self.cards.get_mut(&other)?;
        if incoming.activation > survivor.activation {
            survivor.target = incoming.target.clone();
            survivor.activation = incoming.activation;
        }
        survivor.state.pinned |= incoming.state.pinned;
        Some(other)
    }
}

type LookupResult = Result<Option<UserProfile>, String>;
struct Flight {
    id: u64,
    sender: watch::Sender<Option<LookupResult>>,
}

#[derive(Default)]
pub struct UserCardRuntime {
    registry: Mutex<Registry>,
    flights: Mutex<HashMap<String, Flight>>,
    next: AtomicU64,
}

impl UserCardRuntime {
    fn next(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn close_all(&self, app: &AppHandle) {
        let cards = std::mem::take(&mut self.registry.lock().unwrap().cards);
        let labels: Vec<_> = cards.keys().cloned().collect();
        drop(cards);
        self.flights.lock().unwrap().clear();
        for label in labels {
            if let Some(window) = app.get_webview_window(&label) {
                let _ = window.close();
            }
        }
    }

    fn publish(&self, app: &AppHandle, label: &str) {
        let state = self
            .registry
            .lock()
            .unwrap()
            .cards
            .get_mut(label)
            .map(|card| {
                card.state.revision += 1;
                card.state.clone()
            });
        if let Some(state) = state {
            let _ = app.emit_to(label, STATE_EVENT, &state);
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.set_title(&format!(
                    "{} (@{}) — Pepo",
                    state.user.display_name, state.user.login
                ));
            }
        }
    }

    fn sync_cache(&self, app: &AppHandle, label: &str, engine: &ChatEngine) {
        let target = self
            .registry
            .lock()
            .unwrap()
            .cards
            .get(label)
            .map(|card| card.target.clone());
        let Some(target) = target else {
            return;
        };
        let Some(cached) =
            engine.cached_user(target.user_id.as_deref(), &target.login, &target.channel_id)
        else {
            return;
        };
        let changed = {
            let mut registry = self.registry.lock().unwrap();
            let Some(card) = registry.cards.get_mut(label) else {
                return;
            };
            if card.target.channel_id != target.channel_id
                || card.target.user_id != target.user_id
                || card.target.login != target.login
            {
                return;
            }
            let old = card.state.clone();
            card.target.user_id = Some(cached.user.user_id.clone());
            card.target.login = cached.user.login.clone();
            card.state.user = cached.user;
            card.state.profile_image_url = cached.profile_image_url;
            card.state.badges = cached.badges;
            card.state.channel_id = card.target.channel_id.clone();
            card.state.channel_login = card.target.channel_login.clone();
            old != card.state
        };
        if changed {
            self.publish(app, label);
        }
    }

    fn track(&self, label: &str, task: tauri::async_runtime::JoinHandle<()>) {
        let mut registry = self.registry.lock().unwrap();
        if let Some(card) = registry.cards.get_mut(label) {
            card.tasks.retain(|task| !task.inner().is_finished());
            card.tasks.push(task);
        } else {
            task.abort();
        }
    }

    fn watch_cache(&self, app: AppHandle, label: String, engine: Arc<ChatEngine>) {
        let mut changes = engine.changes();
        let owner = label.clone();
        let task = tauri::async_runtime::spawn(async move {
            loop {
                app.state::<UserCardRuntime>()
                    .sync_cache(&app, &label, &engine);
                if changes.changed().await.is_err() || !engine.active() {
                    break;
                }
            }
        });
        self.track(&owner, task);
    }

    fn lookup(
        &self,
        app: AppHandle,
        target: UserCardTarget,
        engine: Arc<ChatEngine>,
    ) -> watch::Receiver<Option<LookupResult>> {
        let key = format!(
            "{:p}:{}",
            Arc::as_ptr(&engine),
            target.user_id.as_deref().unwrap_or(&target.login)
        );
        let mut flights = self.flights.lock().unwrap();
        if let Some(flight) = flights.get(&key) {
            return flight.sender.subscribe();
        }
        let (sender, receiver) = watch::channel(None);
        let id = self.next();
        flights.insert(
            key.clone(),
            Flight {
                id,
                sender: sender.clone(),
            },
        );
        tauri::async_runtime::spawn(async move {
            let started = engine.user_cache_clock();
            let result = tokio::select! {
                _ = sender.closed() => None,
                result = fetch_profile(&app, &target) => Some(result),
            };
            if let Some(result) = result {
                if engine.active() {
                    if let Ok(Some(profile)) = &result {
                        engine.observe_user_profile(profile.clone(), started);
                    }
                    sender.send_replace(Some(result));
                }
            }
            let runtime = app.state::<UserCardRuntime>();
            let mut flights = runtime.flights.lock().unwrap();
            if flights.get(&key).is_some_and(|flight| flight.id == id) {
                flights.remove(&key);
            }
        });
        receiver
    }

    fn refresh(&self, app: AppHandle, label: String, engine: Arc<ChatEngine>) {
        let request = self.next();
        let target = {
            let mut registry = self.registry.lock().unwrap();
            let Some(card) = registry.cards.get_mut(&label) else {
                return;
            };
            card.request = request;
            card.state.profile_status = ProfileStatus::Loading;
            card.state.error = None;
            card.target.clone()
        };
        self.publish(&app, &label);
        let mut receiver = self.lookup(app.clone(), target, engine.clone());
        let owner = label.clone();
        let task = tauri::async_runtime::spawn(async move {
            let result = loop {
                if let Some(result) = receiver.borrow_and_update().clone() {
                    break result;
                }
                if receiver.changed().await.is_err() {
                    return;
                }
            };
            if !engine.active() {
                return;
            }
            let runtime = app.state::<UserCardRuntime>();
            let mut registry = runtime.registry.lock().unwrap();
            if !registry
                .cards
                .get(&label)
                .is_some_and(|card| card.request == request)
            {
                return;
            }
            let survivor = match &result {
                Ok(Some(profile)) => registry.resolve(&label, &profile.user_id, &profile.login),
                _ => Some(label.clone()),
            };
            let Some(survivor) = survivor else {
                return;
            };
            let Some(card) = registry.cards.get_mut(&survivor) else {
                return;
            };
            card.state.profile_status = match &result {
                Ok(Some(_)) => ProfileStatus::Ready,
                Ok(None) => ProfileStatus::Unavailable,
                Err(_) => ProfileStatus::Error,
            };
            card.state.error = result.err();
            drop(registry);
            runtime.sync_cache(&app, &survivor, &engine);
            runtime.publish(&app, &survivor);
            if survivor != label {
                if let Some(window) = app.get_webview_window(&survivor) {
                    let _ = window.set_focus();
                }
                if let Some(window) = app.get_webview_window(&label) {
                    let _ = window.close();
                }
            }
        });
        self.track(&owner, task);
    }
}

async fn fetch_profile(app: &AppHandle, target: &UserCardTarget) -> LookupResult {
    let _request = app
        .state::<NetworkRuntime>()
        .request_guard(app, "twitch")
        .await;
    let manager = app.state::<TokenManager>();
    let snapshot = manager
        .active_snapshot()
        .await
        .ok_or("Sign in again to load this profile.")?;
    let snapshot = manager
        .ready_token_for_session(&snapshot.identity)
        .await
        .map_err(|_| "Could not authenticate with Twitch. Try again.".to_string())?;
    let client = app.state::<twitch_api::HelixClient<'static, reqwest::Client>>();
    let result = if let Some(id) = &target.user_id {
        client.get_user_from_id(id.as_str(), &snapshot.token).await
    } else {
        client
            .get_user_from_login(target.login.as_str(), &snapshot.token)
            .await
    };
    result
        .map(|user| user.map(UserProfile::from))
        .map_err(|error| {
            let failure = crate::network::Failure::helix(error);
            tracing::warn!(category = failure.category, "user profile lookup failed");
            "Could not load this profile from Twitch. Try again.".into()
        })
}

fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 25
        && login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn fit_position(x: f64, y: f64, width: f64, height: f64, area: (f64, f64, f64, f64)) -> (i32, i32) {
    let (left, top, right, bottom) = area;
    (
        x.clamp(left, (right - width).max(left)).round() as i32,
        y.clamp(top, (bottom - height).max(top)).round() as i32,
    )
}

fn fit_height(content_height: f64, available_height: Option<f64>) -> Result<f64, String> {
    if !content_height.is_finite() || content_height <= 0.0 {
        return Err("The user card content height is invalid.".into());
    }
    let maximum = available_height.unwrap_or(f64::MAX).max(MIN_CARD_HEIGHT);
    Ok(content_height.ceil().clamp(MIN_CARD_HEIGHT, maximum))
}

fn fit_card(window: &WebviewWindow, content_height: f64) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let inner = window.inner_size().map_err(|error| error.to_string())?;
    let frame_height = window
        .outer_size()
        .map_err(|error| error.to_string())?
        .height
        .saturating_sub(inner.height) as f64
        / scale;
    let available = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .map(|monitor| monitor.work_area().size.height as f64 / scale - frame_height);
    let height = fit_height(content_height, available)?;
    window
        .set_size(tauri::LogicalSize::new(inner.width as f64 / scale, height))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn native_foreground(window: &WebviewWindow) -> bool {
    window.hwnd().is_ok_and(|handle| unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 == handle.0
    })
}

#[cfg(not(target_os = "windows"))]
fn native_foreground(_window: &WebviewWindow) -> bool {
    false
}

fn place_card(
    window: &WebviewWindow,
    parent: &WebviewWindow,
    anchor: &CardAnchor,
    mode: UserCardPlacement,
) -> tauri::Result<()> {
    if mode == UserCardPlacement::Os
        || cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Ok(());
    }
    let origin = parent.inner_position()?;
    let scale = parent.scale_factor()?;
    let anchor_x = origin.x as f64 + anchor.x * scale;
    let anchor_y = origin.y as f64 + (anchor.y + anchor.height) * scale;
    let monitor = parent
        .monitor_from_point(anchor_x, anchor_y)?
        .or(parent.current_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let size = window.outer_size()?;
    let area = monitor.work_area();
    let bounds = (
        area.position.x as f64,
        area.position.y as f64,
        area.position.x as f64 + area.size.width as f64,
        area.position.y as f64 + area.size.height as f64,
    );
    let (x, y) = if mode == UserCardPlacement::Centered {
        let parent_size = parent.outer_size()?;
        let parent_origin = parent.outer_position()?;
        (
            parent_origin.x as f64 + (parent_size.width as f64 - size.width as f64) / 2.0,
            parent_origin.y as f64 + (parent_size.height as f64 - size.height as f64) / 2.0,
        )
    } else {
        let right = anchor_x + anchor.width * scale + 8.0 * scale;
        (
            if right + size.width as f64 <= bounds.2 {
                right
            } else {
                anchor_x - size.width as f64 - 8.0 * scale
            },
            anchor_y + 8.0 * scale,
        )
    };
    let (x, y) = fit_position(x, y, size.width as f64, size.height as f64, bounds);
    window.set_position(tauri::PhysicalPosition::new(x, y))
}

#[tauri::command]
#[specta::specta]
pub async fn open_user_card(
    app: AppHandle,
    window: WebviewWindow,
    mut target: UserCardTarget,
) -> Result<(), String> {
    target.login = target.login.trim().to_ascii_lowercase();
    target.channel_login = target.channel_login.trim().to_ascii_lowercase();
    target.user_id = target.user_id.filter(|id| !id.is_empty());
    if !valid_login(&target.login)
        || !valid_login(&target.channel_login)
        || target.channel_id.is_empty()
        || [
            target.anchor.x,
            target.anchor.y,
            target.anchor.width,
            target.anchor.height,
        ]
        .iter()
        .any(|n| !n.is_finite())
    {
        return Err("This user reference is incomplete.".into());
    }
    let engine = app.state::<ChatRuntime>().current()?;
    let runtime = app.state::<UserCardRuntime>();
    if let Some(cached) =
        engine.cached_user(target.user_id.as_deref(), &target.login, &target.channel_id)
    {
        target.user_id = Some(cached.user.user_id);
        target.login = cached.user.login;
        target.display_name = cached.user.display_name;
    }
    let app_preferences = app_settings(app.state::<StateSyncer>().inner());
    let theme = match app_preferences.appearance.theme {
        AppearanceTheme::Dark => Some(tauri::Theme::Dark),
        AppearanceTheme::Light => Some(tauri::Theme::Light),
        AppearanceTheme::System => None,
    };
    let settings = app_preferences.chat;
    let activation = runtime.next();
    let label = format!("user-card-{activation}");
    let existing = {
        let mut registry = runtime.registry.lock().unwrap();
        if let Some(existing) = registry.find(&target) {
            let card = registry.cards.get_mut(&existing).unwrap();
            card.target = target.clone();
            card.activation = activation;
            card.state.channel_id = target.channel_id.clone();
            card.state.channel_login = target.channel_login.clone();
            card.state.badges = None;
            Some(existing)
        } else {
            registry.cards.insert(
                label.clone(),
                Card {
                    state: UserCardState {
                        user: ObservedUser {
                            user_id: target.user_id.clone().unwrap_or_default(),
                            login: target.login.clone(),
                            display_name: if target.display_name.is_empty() {
                                target.login.clone()
                            } else {
                                target.display_name.clone()
                            },
                            color: None,
                        },
                        profile_image_url: None,
                        channel_id: target.channel_id.clone(),
                        channel_login: target.channel_login.clone(),
                        badges: None,
                        profile_status: ProfileStatus::Loading,
                        error: None,
                        pinned: false,
                        chrome: settings.user_card_chrome,
                        revision: 0,
                    },
                    target: target.clone(),
                    activation,
                    armed: false,
                    native_ready: false,
                    initial_placement: Some(InitialPlacement {
                        parent: window.label().into(),
                        anchor: target.anchor.clone(),
                        mode: settings.user_card_placement,
                    }),
                    request: 0,
                    tasks: Vec::new(),
                },
            );
            None
        }
    };
    if let Some(existing) = existing {
        runtime.sync_cache(&app, &existing, &engine);
        runtime.publish(&app, &existing);
        if let Some(card) = app.get_webview_window(&existing) {
            card.set_focus().map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("user-card".into()))
        .title(&format!("{} — Pepo", target.display_name))
        .inner_size(360.0, 240.0)
        .min_inner_size(320.0, MIN_CARD_HEIGHT)
        .prevent_overflow()
        .resizable(true)
        .theme(theme)
        .decorations(settings.user_card_chrome == UserCardChrome::Native)
        .focused(false)
        .visible(false)
        .background_color(crate::window_chrome::BACKGROUND_COLOR);
    let builder = if settings.user_card_chrome == UserCardChrome::Native {
        crate::window_chrome::configure(builder)
    } else {
        builder
    };
    let built = builder.build();
    let card = match built {
        Ok(card)
            if engine.active() && runtime.registry.lock().unwrap().cards.contains_key(&label) =>
        {
            card
        }
        Ok(card) => {
            let _ = card.close();
            return Err("The chat session ended.".into());
        }
        Err(error) => {
            runtime.registry.lock().unwrap().cards.remove(&label);
            return Err(error.to_string());
        }
    };
    if settings.user_card_chrome == UserCardChrome::Native {
        crate::window_chrome::apply(&card);
    }
    if let Err(error) = place_card(&card, &window, &target.anchor, settings.user_card_placement) {
        tracing::debug!(%error, "using desktop placement for user card");
    }
    let event_app = app.clone();
    let event_label = label.clone();
    card.on_window_event(move |event| {
        let runtime = event_app.state::<UserCardRuntime>();
        match event {
            WindowEvent::Focused(focused) => {
                let native_active = !focused
                    && event_app
                        .get_webview_window(&event_label)
                        .is_some_and(|card| native_foreground(&card));
                let policy = app_settings(event_app.state::<StateSyncer>().inner())
                    .chat
                    .user_card_close_on_unfocus;
                let close = runtime.registry.lock().unwrap().focus(
                    &event_label,
                    *focused,
                    native_active,
                    policy,
                );
                if close {
                    runtime.registry.lock().unwrap().cards.remove(&event_label);
                    if let Some(card) = event_app.get_webview_window(&event_label) {
                        let _ = card.close();
                    }
                }
            }
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                runtime.registry.lock().unwrap().cards.remove(&event_label);
                event_app
                    .state::<NetworkRuntime>()
                    .release_owner(&event_label);
            }
            _ => {}
        }
    });
    runtime.sync_cache(&app, &label, &engine);
    runtime.watch_cache(app.clone(), label.clone(), engine.clone());
    if let Some(entry) = runtime.registry.lock().unwrap().cards.get_mut(&label) {
        entry.native_ready = true;
    }
    runtime.refresh(app.clone(), label, engine);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_user_card_state(app: AppHandle, window: WebviewWindow) -> Result<UserCardState, String> {
    app.state::<UserCardRuntime>()
        .registry
        .lock()
        .unwrap()
        .cards
        .get(window.label())
        .map(|card| card.state.clone())
        .ok_or("This user card has closed.".into())
}

#[tauri::command]
#[specta::specta]
pub async fn ready_user_card(
    app: AppHandle,
    window: WebviewWindow,
    content_height: f64,
) -> Result<(), String> {
    fit_height(content_height, None)?;
    let placement = loop {
        {
            let runtime = app.state::<UserCardRuntime>();
            let mut registry = runtime.registry.lock().unwrap();
            let card = registry
                .cards
                .get_mut(window.label())
                .ok_or("This user card has closed.")?;
            if card.native_ready {
                break card.initial_placement.take();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    if let Some(placement) = placement {
        fit_card(&window, content_height)?;
        if let Some(parent) = app.get_webview_window(&placement.parent) {
            if let Err(error) = place_card(&window, &parent, &placement.anchor, placement.mode) {
                tracing::debug!(%error, "using desktop placement for user card");
            }
        }
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn set_user_card_pinned(
    app: AppHandle,
    window: WebviewWindow,
    pinned: bool,
) -> Result<(), String> {
    let runtime = app.state::<UserCardRuntime>();
    {
        let mut registry = runtime.registry.lock().unwrap();
        let card = registry
            .cards
            .get_mut(window.label())
            .ok_or("This user card has closed.")?;
        card.state.pinned = pinned;
    }
    runtime.publish(&app, window.label());
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_user_card(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    get_user_card_state(app.clone(), window.clone())?;
    let engine = app.state::<ChatRuntime>().current()?;
    app.state::<UserCardRuntime>()
        .refresh(app.clone(), window.label().into(), engine);
    Ok(())
}

#[cfg(test)]
mod tests;

use eventsub::EventSubManager;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use tauri_svelte_synced_store::{StateSyncer, StateSyncerConfig};
use token::TokenManager;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace};
use twitch_api::{client::ClientDefault, HelixClient};
use twitch_oauth2::tokens::errors::ValidationError;

use crate::badgemanager::BadgeManager;
use crate::emote::cache::EmoteCacheTrait;
use crate::emotemanager::EmoteManager;
use crate::types::{AuthState, ChannelCache};

mod badgemanager;
mod emote;
mod emotemanager;
mod eventsub;
mod message;
mod token;
mod types;

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
struct InternalState {
    version: String,
    name: String,
    sidebar_open: bool,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            version: String::new(),
            name: String::new(),
            sidebar_open: true,
        }
    }
}

type SharedUserToken = Mutex<types::UserToken>;
type SharedTwitchToken = Mutex<twitch_oauth2::UserToken>;
type SharedEventSubManager = Mutex<EventSubManager>;
type SharedBadgeManager = Mutex<BadgeManager>;
type SharedEmoteManager = Mutex<EmoteManager>;
type SharedPollHandle = Mutex<Option<tauri::async_runtime::JoinHandle<()>>>;

tauri_svelte_synced_store::state_handlers!(
    AuthState = "auth_state",
    InternalState = "internal_state",
    ChannelCache = "channel_cache"
);

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<types::UserToken>()
        .typ::<types::ChannelMessage>()
        .typ::<types::AuthState>()
        .typ::<types::AuthPhase>()
        .typ::<types::ChannelCache>()
        .typ::<types::ChannelStatus>()
        .typ::<InternalState>()
        .commands(collect_commands![
            get_followed_streams,
            get_followed_channels,
            get_channel_info,
            join_chat,
            leave_chat,
            login,
            logout,
            send_chat_message,
            emit_state,
            update_state,
            search_emotes,
        ])
}

#[tauri::command]
#[specta::specta]
fn send_chat_message(
    broadcaster_id: String,
    message: String,
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!(
        "sending chat message: broadcaster_id={}, message={}",
        broadcaster_id, message
    );

    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
    let user_id = token_guard.user_id.clone();
    match tauri::async_runtime::block_on(client.send_chat_message(
        broadcaster_id,
        user_id,
        message.as_str(),
        &token_guard,
    )) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("request failed: {:?}", e)),
    }
}

#[tauri::command]
#[specta::specta]
async fn search_emotes(
    query: String,
    broadcaster_id: String,
    limit: Option<usize>,
    emote_manager_ref: State<'_, SharedEmoteManager>,
) -> Result<Vec<emote::Emote>, String> {
    let emote_manager = emote_manager_ref.lock().await.clone();
    let cache = emote_manager.get_emote_cache(broadcaster_id);
    Ok(cache.search_emotes(&query, limit.unwrap_or(25)))
}

#[tauri::command]
#[specta::specta]
async fn get_channel_info(
    channel_name: String,
    token_ref: State<'_, SharedTwitchToken>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::ChannelInfo, String> {
    debug!("get_channel_info: channel={}", channel_name);

    let token = token_ref.lock().await;
    let client = client_ref.inner();

    let channel = client
        .get_channel_from_login(&channel_name, &token.clone())
        .await
        .map_err(|e| format!("failed to get channel: {:?}", e))?
        .ok_or_else(|| format!("channel not found: {}", channel_name))?;

    let mut channel_info = types::ChannelInfo::from(channel);

    if let Ok(Some(user)) = client
        .get_user_from_id(channel_info.broadcaster_id.as_str(), &token.clone())
        .await
    {
        channel_info.profile_image_url = user.profile_image_url.unwrap_or_default();
    }

    Ok(channel_info)
}

#[tauri::command]
#[specta::specta]
async fn join_chat(
    channel_name: String,
    _app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    badge_manager_ref: State<'_, SharedBadgeManager>,
    emote_manager_ref: State<'_, SharedEmoteManager>,
    token_ref: State<'_, SharedTwitchToken>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::ChannelInfo, String> {
    debug!("join: channel={}", channel_name);

    let token = token_ref.lock().await;
    let client = client_ref.inner();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();
    let badge_manager = badge_manager_ref.lock().await.clone();
    let emote_manager = emote_manager_ref.lock().await.clone();

    let channel = client
        .get_channel_from_login(&channel_name, &token.clone())
        .await
        .unwrap()
        .expect("missing channel");

    debug!("join: got channel info - {:?}", channel.clone());

    badge_manager
        .load_channel(channel.broadcaster_id.to_string(), client.clone())
        .await;

    emote_manager
        .load_channel(channel.broadcaster_id.to_string())
        .await;

    match eventsub_manager
        .join_chat(
            channel.broadcaster_id.clone(),
            channel_name,
            client,
            token.clone(),
        )
        .await
    {
        Ok(_) => debug!("joined channel"),
        Err(e) => error!("join_chat - {:?}", e),
    };

    let mut channel_info = types::ChannelInfo::from(channel);

    if let Ok(Some(user)) = client
        .get_user_from_id(channel_info.broadcaster_id.as_str(), &token.clone())
        .await
    {
        channel_info.profile_image_url = user.profile_image_url.unwrap_or_default();
    }

    Ok(channel_info)
}

#[tauri::command]
#[specta::specta]
async fn leave_chat(
    channel_name: String,
    _app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    token_ref: State<'_, SharedTwitchToken>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!("leave: channel={}", channel_name);

    let token = token_ref.lock().await;
    let client = client_ref.inner();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();

    match eventsub_manager
        .leave_chat(channel_name, client, token.clone())
        .await
    {
        Ok(_) => debug!("left channel"),
        Err(e) => error!("leave_chat - {:?}", e),
    };

    Ok(())
}

#[tauri::command]
#[specta::specta]
fn get_followed_streams(
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Stream>, String> {
    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
    let streams = tauri::async_runtime::block_on(
        client
            .get_followed_streams(&token_guard)
            .try_collect::<Vec<_>>(),
    )
    .unwrap();

    Ok(streams.into_iter().map(types::Stream::from).collect())
}

#[tauri::command]
#[specta::specta]
fn get_followed_channels(
    _app_handle: AppHandle,
    token: State<'_, SharedTwitchToken>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Broadcaster>, String> {
    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token.lock()).clone();
    let channels = match tauri::async_runtime::block_on(
        client
            .get_followed_channels(token_guard.user_id.to_string(), &token_guard)
            .try_collect::<Vec<_>>(),
    ) {
        Ok(channels) => channels,
        Err(err) => {
            error!("failed to get followed channels: err={}", err.to_string());
            return Err(err.to_string());
        }
    };

    let mut users: Vec<twitch_api::helix::users::User> = Vec::new();
    for chunk in channels.chunks(100) {
        let ids: Vec<twitch_api::types::UserId> =
            chunk.iter().map(|b| b.broadcaster_id.clone()).collect();

        debug!("ids={:?}", ids);
        let mut u: Vec<twitch_api::helix::users::User> = tauri::async_runtime::block_on(
            client
                .get_users_from_ids(&twitch_api::types::Collection::from(ids), &token_guard)
                .try_collect::<Vec<_>>(),
        )
        .unwrap();
        users.append(&mut u);
    }

    Ok(users.into_iter().map(types::Broadcaster::from).collect())
}

async fn poll_channel_cache(app_handle: &AppHandle) -> Result<(), String> {
    let token = {
        let token_state = app_handle.state::<SharedTwitchToken>();
        let guard = token_state.lock().await;
        guard.clone()
    };
    let client = app_handle.state::<HelixClient<'static, reqwest::Client>>();
    let state_syncer = app_handle.state::<StateSyncer>();

    info!("polling channel cache");

    let channels: Vec<twitch_api::helix::channels::FollowedBroadcaster> = client
        .get_followed_channels(token.user_id.to_string(), &token)
        .try_collect()
        .await
        .map_err(|e| format!("failed to get followed channels: {}", e))?;

    let mut users: HashMap<String, twitch_api::helix::users::User> = HashMap::new();
    for chunk in channels.chunks(100) {
        let ids: Vec<twitch_api::types::UserId> =
            chunk.iter().map(|b| b.broadcaster_id.clone()).collect();
        let chunk_users: Vec<twitch_api::helix::users::User> = client
            .get_users_from_ids(&twitch_api::types::Collection::from(ids), &token)
            .try_collect()
            .await
            .map_err(|e| format!("failed to get users: {}", e))?;
        for user in chunk_users {
            users.insert(user.login.to_string(), user);
        }
    }

    let streams: Vec<twitch_api::helix::streams::Stream> = client
        .get_followed_streams(&token)
        .try_collect()
        .await
        .map_err(|e| format!("failed to get followed streams: {}", e))?;

    let live_streams: HashMap<String, types::Stream> = streams
        .into_iter()
        .map(|s| (s.user_login.to_string(), types::Stream::from(s)))
        .collect();

    let mut cache = types::ChannelCache::default();
    for channel in &channels {
        let login = channel.broadcaster_login.to_string();
        let user = users.get(&login);
        let stream = live_streams.get(&login).cloned();
        let is_live = stream.is_some();

        cache.channels.insert(
            login.clone(),
            types::ChannelStatus {
                broadcaster_id: channel.broadcaster_id.to_string(),
                login: login.clone(),
                display_name: channel.broadcaster_name.to_string(),
                profile_image_url: user
                    .and_then(|u| u.profile_image_url.clone())
                    .unwrap_or_default(),
                is_live,
                stream,
            },
        );
    }

    info!(
        "channel cache updated: {} channels, {} live",
        cache.channels.len(),
        cache.channels.values().filter(|c| c.is_live).count()
    );
    state_syncer.update::<types::ChannelCache>("channel_cache", cache, true);

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn login(
    app_handle: AppHandle,
    state_syncer: State<'_, StateSyncer>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
    quick: bool,
) -> Result<types::UserToken, String> {
    let client = client.inner();
    info!(quick, "login");

    let mut token_manager: TokenManager;
    let store = app_handle.store("account.json").unwrap();

    if let Some(binding) = store.get("token") {
        debug!("loading token from file store");
        let token: types::UserToken = serde_json::from_value(binding.clone()).unwrap();
        token_manager = match token.to_twitch_token(client.clone()).await {
            Ok(token) => TokenManager::from_existing(token.clone(), client.clone()),
            Err(ValidationError::NotAuthorized) => TokenManager::new(client.clone()),
            Err(err) => panic!("{err}"),
        };
    } else {
        debug!("no token found on disk, setting empty token");
        token_manager = TokenManager::new(client.clone());
    }

    let twitch_token: twitch_oauth2::UserToken;
    let user_token: types::UserToken;
    if token_manager.clone().is_token_valid().await {
        debug!("token is already valid");
        twitch_token = token_manager.clone().get_token().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    } else if quick {
        return Err("no valid token".to_string());
    } else {
        let device_code = token_manager.clone().start_device_code_flow().await;

        {
            let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
            let mut auth_state = auth_state_ref.lock().unwrap();

            auth_state.phase = types::AuthPhase::WaitingForAuth;
            auth_state.device_code = device_code.user_code;
        }

        debug!("pausing to show verification code");
        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("login {}", device_code.verification_uri);
        app_handle
            .opener()
            .open_url(device_code.verification_uri.clone(), None::<&str>)
            .unwrap();

        twitch_token = token_manager.clone().finish_device_code_flow().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    }

    {
        let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
        let mut auth_state = auth_state_ref.lock().unwrap();

        auth_state.phase = types::AuthPhase::Authorized;
        auth_state.token = Some(user_token.clone());
    }

    // Create empty managers — no network calls, just register state so
    // other Tauri commands can access them immediately.
    let eventsub_manager = EventSubManager::new();
    let badge_manager = BadgeManager::empty(twitch_token.clone());
    let emote_manager = EmoteManager::empty(client.clone(), twitch_token.clone());

    // Register or update shared state (safe for re-login)
    if !app_handle.manage::<SharedEventSubManager>(Mutex::new(eventsub_manager.clone())) {
        *app_handle.state::<SharedEventSubManager>().lock().await = eventsub_manager.clone();
    }
    if !app_handle.manage::<SharedBadgeManager>(Mutex::new(badge_manager.clone())) {
        *app_handle.state::<SharedBadgeManager>().lock().await = badge_manager.clone();
    }
    if !app_handle.manage::<SharedEmoteManager>(Mutex::new(emote_manager.clone())) {
        *app_handle.state::<SharedEmoteManager>().lock().await = emote_manager.clone();
    }
    if !app_handle.manage::<SharedUserToken>(Mutex::new(user_token.clone())) {
        *app_handle.state::<SharedUserToken>().lock().await = user_token.clone();
    }
    if !app_handle.manage::<SharedTwitchToken>(Mutex::new(twitch_token.clone())) {
        *app_handle.state::<SharedTwitchToken>().lock().await = twitch_token.clone();
    }

    // Token refresh callback + persistence
    let app_handle_ref = app_handle.clone();
    let store_ref = store.clone();
    token_manager.on_refresh = Arc::new(Box::new(move |token| {
        let user_token_binding = app_handle_ref.state::<SharedUserToken>();
        let twitch_token_binding = app_handle_ref.state::<SharedTwitchToken>();

        let mut user_token_ref = tauri::async_runtime::block_on(user_token_binding.lock());
        let mut twitch_token_ref = tauri::async_runtime::block_on(twitch_token_binding.lock());

        *twitch_token_ref = token.clone();

        let new_token = types::UserToken::from_twitch_token(token);
        *user_token_ref = new_token.clone();

        debug!(
            new_token.access_token,
            new_token.login, new_token.user_id, "updating token store"
        );

        store_ref.set("token", json!(new_token));
    }));

    debug!("setting token");
    store.set("token", json!(user_token.clone()));
    token_manager.manage();

    // --- Background tasks ---

    // Profile image fetch
    {
        let client = client.clone();
        let twitch_token = twitch_token.clone();
        let state_syncer = state_syncer.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(Some(user_info)) = client
                .get_user_from_id(&twitch_token.user_id, &twitch_token)
                .await
            {
                let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
                let mut auth_state = auth_state_ref.lock().unwrap();
                if let Some(ref mut token) = auth_state.token {
                    token.profile_image_url = user_info.profile_image_url.unwrap_or_default();
                }
            }
        });
    }

    // Global badges
    {
        let badge_manager = badge_manager.clone();
        let client = client.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = badge_manager.load_global(client).await {
                error!("failed to load global badges: {}", e);
            }
        });
    }

    // Global + user emotes
    {
        let emote_manager = emote_manager.clone();
        tauri::async_runtime::spawn(async move {
            emote_manager.load_global();
            emote_manager.load_user_emotes();
        });
    }

    // EventSub connection + message processing thread
    {
        let eventsub_manager = eventsub_manager.clone();
        let client = client.clone();
        let twitch_token = twitch_token.clone();
        let app_ref = app_handle.clone();
        let badge_manager_ref = badge_manager.clone();
        let emote_manager_ref = emote_manager.clone();
        tauri::async_runtime::spawn(async move {
            let events = match eventsub_manager.start(client, twitch_token) {
                Ok(events) => events,
                Err(e) => {
                    error!("failed to start eventsub: {}", e);
                    return;
                }
            };

            std::thread::spawn(move || {
                use twitch_api::eventsub::{Message as M, Payload as P};

                for msg in events {
                    match msg {
                        eventsub::EventSubMessage::AuthFailed(reason) => {
                            error!("EventSub auth failed: {}", reason);
                            clear_auth(&app_ref);
                            break;
                        }
                        eventsub::EventSubMessage::Notification(notification) => {
                            match notification.event {
                                twitch_api::eventsub::Event::ChannelChatMessageV1(P {
                                    message: M::Notification(chat_message),
                                    ..
                                }) => {
                                    let channel_msg = types::ChannelMessage::new(
                                        chat_message.clone(),
                                        notification.ts.to_string(),
                                        badge_manager_ref.clone(),
                                        emote_manager_ref.clone(),
                                    );
                                    let key = format!(
                                        "chat_message:{}",
                                        chat_message.broadcaster_user_login
                                    );
                                    trace!("chat message: id={} msg={:?}", key, channel_msg);
                                    app_ref
                                        .emit(&key, channel_msg)
                                        .expect("unable to emit state")
                                }
                                _ => debug!("event notification: {:?}", notification.event),
                            }
                        }
                    }
                }
            });
        });
    }

    // Channel cache: initial poll + recurring loop
    let poll_app = app_handle.clone();
    let poll_handle = tauri::async_runtime::spawn(async move {
        if let Err(e) = poll_channel_cache(&poll_app).await {
            error!("initial channel cache poll failed: {}", e);
        }

        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if let Err(e) = poll_channel_cache(&poll_app).await {
                error!("channel cache poll failed: {}", e);
            }
        }
    });

    // Store handle, aborting any existing poll task
    {
        let poll_handle_state = app_handle.state::<SharedPollHandle>();
        let mut existing = poll_handle_state.lock().await;
        if let Some(handle) = existing.take() {
            handle.abort();
        }
        *existing = Some(poll_handle);
    }

    info!("login returned");
    Ok(user_token)
}

/// Clears auth state, channel cache, stored token, and aborts the poll task.
/// Used by both explicit logout and automatic auth expiration handling.
fn clear_auth(app_handle: &AppHandle) {
    let state_syncer = app_handle.state::<StateSyncer>();

    // Abort poll task
    {
        let poll_handle_state = app_handle.state::<SharedPollHandle>();
        let mut poll_handle = tauri::async_runtime::block_on(poll_handle_state.lock());
        if let Some(handle) = poll_handle.take() {
            handle.abort();
        }
    }

    state_syncer.update::<AuthState>("auth_state", AuthState::default(), true);
    state_syncer.update::<types::ChannelCache>(
        "channel_cache",
        types::ChannelCache::default(),
        true,
    );

    if let Ok(store) = app_handle.store("account.json") {
        store.delete("token");
    }
}

#[tauri::command]
#[specta::specta]
fn logout(app_handle: AppHandle, _state_syncer: State<'_, StateSyncer>) -> Result<(), String> {
    info!("logout");
    clear_auth(&app_handle);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    color_eyre::install().expect("failed to install color_eyre");

    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(1_000_000) // 1 MB per log file
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .level(log::LevelFilter::Debug)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build());

    let handlers = specta_builder();

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    handlers
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header("/* eslint-disable */"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("src-tauri has no parent directory")
                .join("src/lib/bindings.ts"),
        )
        .expect("Failed to export typescript bindings");

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            info!("{}, {argv:?}, {cwd}", app.package_info().name);
            app.emit("single-instance", argv).unwrap();
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build());

    let _builder = builder
        .invoke_handler(handlers.invoke_handler())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .unwrap();

            // This is also required if you want to use events
            handlers.mount_events(app);

            let mut sync_cfg = StateSyncerConfig::default();
            sync_cfg.persist_keys.insert("auth_state".to_owned(), true);
            sync_cfg
                .persist_keys
                .insert("internal_state".to_owned(), true);
            sync_cfg
                .persist_keys
                .insert("channel_cache".to_owned(), true);
            let state_syncer = StateSyncer::new(sync_cfg, app.handle().clone());

            let mut internal_state: InternalState = state_syncer.load("internal_state");
            internal_state.version = app.package_info().version.to_string();
            internal_state.name = app.package_info().name.to_string();

            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default());

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

            let window = win_builder.build().unwrap();

            let _ = window.set_title(&format!(
                "{} {}",
                internal_state.name.clone(),
                internal_state.version.clone()
            ));

            // set background color only when building for macOS
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSColor, NSWindow};

                unsafe {
                    let ns_window = &*(window.ns_window().unwrap() as *mut NSWindow);

                    //rgb(24, 31, 42)
                    let bg_color = NSColor::colorWithRed_green_blue_alpha(
                        20.0 / 255.0,
                        26.0 / 255.0,
                        39.0 / 255.0,
                        1.0,
                    );
                    ns_window.setBackgroundColor(bg_color.downcast_ref());
                }
            }

            // set title bar color only when building for Windows
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::Foundation::COLORREF;
                use windows::Win32::Graphics::Dwm::{
                    DwmSetWindowAttribute, DWMWA_CAPTION_COLOR,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                };

                let hwnd = window.hwnd().unwrap();

                unsafe {
                    // enable dark mode for title bar text/buttons
                    let use_dark_mode: i32 = 1;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_USE_IMMERSIVE_DARK_MODE,
                        &use_dark_mode as *const i32 as *const std::ffi::c_void,
                        std::mem::size_of::<i32>() as u32,
                    );

                    // RGB(20, 26, 39) -> COLORREF 0x00BBGGRR = 0x00271A14
                    let caption_color = COLORREF(0x00271A14);
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_CAPTION_COLOR,
                        &caption_color as *const COLORREF as *const std::ffi::c_void,
                        std::mem::size_of::<COLORREF>() as u32,
                    );
                }
            }

            let client: HelixClient<'static, reqwest::Client> =
                twitch_api::HelixClient::with_client(
                    ClientDefault::default_client_with_name(Some(
                        "pepo".parse().expect("invalid client name"),
                    ))
                    .unwrap(),
                );

            info!("made client");

            state_syncer.set("auth_state", AuthState::default());
            state_syncer.set("internal_state", internal_state);
            state_syncer.set("channel_cache", ChannelCache::default());
            app.manage::<StateSyncer>(state_syncer);
            app.manage::<SharedPollHandle>(Mutex::new(None));

            app.manage(client);

            #[cfg(debug_assertions)]
            window.open_devtools();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use eventsub::EventSubManager;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use tauri_svelte_synced_store::{StateSyncer, StateSyncerConfig};
use token::TokenManager;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};
use twitch_api::{client::ClientDefault, HelixClient};
use twitch_oauth2::TwitchToken;

use crate::badgemanager::BadgeManager;
use crate::emote::cache::EmoteCacheTrait;
use crate::emotemanager::EmoteManager;
use crate::types::{AuthState, ChannelCache, Settings};

mod badgemanager;
mod emote;
mod emotemanager;
mod eventsub;
mod internal;
mod logging;
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

pub(crate) type SharedTwitchToken = Arc<Mutex<twitch_oauth2::UserToken>>;
type SharedEventSubManager = Mutex<EventSubManager>;
type SharedBadgeManager = Mutex<BadgeManager>;
type SharedEmoteManager = Mutex<EmoteManager>;
type SharedPollHandle = Mutex<Option<tauri::async_runtime::JoinHandle<()>>>;
type SharedEventSubHandles = Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>;
/// Guards against overlapping token refreshes (supervisor tick vs. focus event).
type RefreshLock = Mutex<()>;

tauri_svelte_synced_store::state_handlers!(
    AuthState = "auth_state",
    InternalState = "internal_state",
    ChannelCache = "channel_cache",
    Settings = "settings"
);

#[cfg(debug_assertions)]
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has no parent directory")
        .to_path_buf()
}

#[cfg(debug_assertions)]
fn typescript_exporter() -> Typescript {
    Typescript::default()
        .formatter(specta_typescript::formatter::prettier)
        .bigint(specta_typescript::BigIntExportBehavior::Number)
        .header("/* eslint-disable */")
}

#[cfg(debug_assertions)]
fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>, path: std::path::PathBuf) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create bindings output directory");
    }

    builder
        .export(typescript_exporter(), path)
        .expect("Failed to export typescript bindings");
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<types::UserToken>()
        .typ::<types::ChannelMessage>()
        .typ::<types::ChannelMessageTranslation>()
        .typ::<types::ChannelMessageTranslationUpdate>()
        .typ::<types::AuthState>()
        .typ::<types::AuthPhase>()
        .typ::<types::ChannelCache>()
        .typ::<types::ChannelStatus>()
        .typ::<types::Settings>()
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
    state_syncer: State<'_, StateSyncer>,
) -> Result<Vec<emote::Emote>, String> {
    let emote_manager = emote_manager_ref.lock().await.clone();
    let settings = state_syncer.snapshot::<Settings>("settings").normalized();
    let cache = emote_manager.get_emote_cache(broadcaster_id, &settings.emotes);
    Ok(cache.search_emotes(
        &query,
        limit.unwrap_or(settings.emotes.autocomplete_result_limit),
    ))
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
    state_syncer: State<'_, StateSyncer>,
) -> Result<types::ChannelInfo, String> {
    debug!("join: channel={}", channel_name);

    let token = { token_ref.lock().await.clone() };
    let client = client_ref.inner().clone();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();
    let badge_manager = badge_manager_ref.lock().await.clone();
    let emote_manager = emote_manager_ref.lock().await.clone();
    let settings = state_syncer.snapshot::<Settings>("settings").normalized();

    let channel = client
        .get_channel_from_login(&channel_name, &token)
        .await
        .map_err(|e| format!("failed to get channel: {:?}", e))?
        .ok_or_else(|| format!("channel not found: {}", channel_name))?;

    debug!("join: got channel info - {:?}", channel.clone());

    let broadcaster_id = channel.broadcaster_id.to_string();

    if let Err(e) = eventsub_manager
        .join_chat(
            channel.broadcaster_id.clone(),
            channel_name.clone(),
            &client,
            token.clone(),
        )
        .await
    {
        error!("join_chat - {:?}", e);
        return Err(format!("failed to join channel chat: {:?}", e));
    }

    debug!("joined channel");

    let emote_settings = settings.emotes;
    tauri::async_runtime::spawn(async move {
        debug!(
            broadcaster_id,
            channel = channel_name,
            "loading channel cosmetics after EventSub join"
        );
        let badge_broadcaster_id = broadcaster_id.clone();
        let emote_broadcaster_id = broadcaster_id.clone();
        let load_badges = badge_manager.load_channel(badge_broadcaster_id, client);
        let load_emotes = emote_manager.load_channel(emote_broadcaster_id, &emote_settings);
        tokio::join!(load_badges, load_emotes);
    });

    Ok(types::ChannelInfo::from(channel))
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

    debug!("polling channel cache");

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

fn is_auth_failure_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid oauth token")
}

async fn handle_channel_cache_poll_error(
    app_handle: &AppHandle,
    context: &str,
    error: String,
) -> bool {
    if is_auth_failure_error(&error) {
        error!(
            "channel cache poll failed due to auth; clearing auth: {}",
            error
        );
        clear_auth_async(app_handle, false).await;
        return true;
    }

    logging::error_repeated(
        format!("channel_cache_poll:{context}:{error}"),
        format!("{context} channel cache poll failed: {error}"),
        Duration::from_secs(300),
    );
    false
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

    let auth_state_snapshot = state_syncer.snapshot::<AuthState>("auth_state");
    let token_manager = if let Some(token) = auth_state_snapshot.token.clone() {
        debug!("loading token from persisted auth_state");
        match token.to_twitch_token(client.clone()).await {
            Ok(token) => TokenManager::from_existing(token, client.clone()),
            Err(err) => {
                error!("failed to restore token from auth_state: {}", err);
                TokenManager::new(client.clone())
            }
        }
    } else {
        debug!("no token found in persisted auth_state");
        TokenManager::new(client.clone())
    };

    let twitch_token: twitch_oauth2::UserToken;
    let mut user_token: types::UserToken;
    if token_manager.is_token_valid().await {
        debug!("token is already valid");
        twitch_token = token_manager.get_token().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    } else if quick {
        return Err("no valid token".to_string());
    } else {
        let device_code = token_manager.start_device_code_flow().await;

        {
            let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
            let mut auth_state = auth_state_ref.lock().unwrap();

            auth_state.phase = types::AuthPhase::WaitingForAuth;
            auth_state.device_code = device_code.user_code;
            auth_state.token = None;
        }

        debug!("pausing to show verification code");
        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("login {}", device_code.verification_uri);
        app_handle
            .opener()
            .open_url(device_code.verification_uri.clone(), None::<&str>)
            .unwrap();

        twitch_token = token_manager.finish_device_code_flow().await;
        user_token = types::UserToken::from_twitch_token(twitch_token.clone());
    }

    if let Some(existing_token) = auth_state_snapshot.token {
        user_token.profile_image_url = existing_token.profile_image_url;
    }

    {
        let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
        let mut auth_state = auth_state_ref.lock().unwrap();

        auth_state.phase = types::AuthPhase::Authorized;
        auth_state.device_code = String::new();
        auth_state.token = Some(user_token.clone());
    }

    let new_token_ref = Arc::new(Mutex::new(twitch_token.clone()));
    let token_ref = if app_handle.manage::<SharedTwitchToken>(new_token_ref.clone()) {
        new_token_ref
    } else {
        let existing_token_ref = app_handle.state::<SharedTwitchToken>().inner().clone();
        *existing_token_ref.lock().await = twitch_token.clone();
        existing_token_ref
    };

    // Create empty managers — no network calls, just register state so
    // other Tauri commands can access them immediately.
    let eventsub_manager = EventSubManager::new();
    let badge_manager = BadgeManager::empty(token_ref.clone());
    let emote_manager = EmoteManager::empty(client.clone(), token_ref.clone());

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
    // Token refresh is handled by the always-on `token_refresh_supervisor`
    // (spawned once at startup). Login only needs to publish the new token into
    // `SharedTwitchToken` + auth_state (done above); the supervisor reads the
    // shared token each tick, so there is no per-login refresh task to manage.

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
        let emote_settings = state_syncer
            .snapshot::<Settings>("settings")
            .normalized()
            .emotes;
        tauri::async_runtime::spawn(async move {
            emote_manager.load_global(&emote_settings);
            emote_manager.load_user_emotes();
        });
    }

    // EventSub connection + message processing thread
    {
        let eventsub_manager = eventsub_manager.clone();
        let client = client.clone();
        let token_ref = token_ref.clone();
        let app_ref = app_handle.clone();
        let badge_manager_ref = badge_manager.clone();
        let emote_manager_ref = emote_manager.clone();
        let state_syncer_ref = state_syncer.inner().clone();

        {
            let eventsub_handle_state = app_handle.state::<SharedEventSubHandles>();
            let mut existing = eventsub_handle_state.lock().await;
            for handle in existing.drain(..) {
                handle.abort();
            }
        }

        let eventsub_runtime = match eventsub_manager.start(client, token_ref) {
            Ok(runtime) => runtime,
            Err(e) => {
                error!("failed to start eventsub: {}", e);
                return Err("failed to start eventsub".to_string());
            }
        };
        let eventsub::EventSubRuntime { events, handles } = eventsub_runtime;

        {
            let eventsub_handle_state = app_handle.state::<SharedEventSubHandles>();
            *eventsub_handle_state.lock().await = handles;
        }

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
                                let settings = state_syncer_ref
                                    .snapshot::<Settings>("settings")
                                    .normalized();
                                let channel_msg = types::ChannelMessage::new(
                                    chat_message.clone(),
                                    notification.ts.to_string(),
                                    badge_manager_ref.clone(),
                                    emote_manager_ref.clone(),
                                    settings.emotes,
                                    app_ref.clone(),
                                );
                                let key =
                                    format!("chat_message:{}", chat_message.broadcaster_user_login);
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
    }

    // Channel cache: initial poll + recurring loop
    let poll_app = app_handle.clone();
    let poll_handle = tauri::async_runtime::spawn(async move {
        if let Err(e) = poll_channel_cache(&poll_app).await {
            if handle_channel_cache_poll_error(&poll_app, "initial", e).await {
                return;
            }
        }

        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if let Err(e) = poll_channel_cache(&poll_app).await {
                if handle_channel_cache_poll_error(&poll_app, "recurring", e).await {
                    return;
                }
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

async fn clear_auth_async(app_handle: &AppHandle, abort_poll: bool) {
    let state_syncer = app_handle.state::<StateSyncer>();

    {
        let poll_handle_state = app_handle.state::<SharedPollHandle>();
        let mut poll_handle = poll_handle_state.lock().await;
        if let Some(handle) = poll_handle.take() {
            if abort_poll {
                handle.abort();
            }
        }
    }
    {
        let eventsub_handle_state = app_handle.state::<SharedEventSubHandles>();
        let mut eventsub_handles = eventsub_handle_state.lock().await;
        for handle in eventsub_handles.drain(..) {
            handle.abort();
        }
    }
    // The token refresh supervisor is long-lived; it idles automatically once
    // auth_state is reset below (it gates on AuthPhase::Authorized).

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

/// Clears auth state, channel cache, and background auth tasks.
/// Used by both explicit logout and automatic auth expiration handling.
fn clear_auth(app_handle: &AppHandle) {
    tauri::async_runtime::block_on(clear_auth_async(app_handle, true));
}

/// Persist a freshly refreshed token into `auth_state`, preserving the cached
/// profile image. Mirrors what the old `on_refresh` callback wrote.
fn persist_refreshed_token(state_syncer: &StateSyncer, token: &twitch_oauth2::UserToken) {
    let mut new_token = types::UserToken::from_twitch_token(token.clone());
    let auth_state = state_syncer.snapshot::<AuthState>("auth_state");
    if let Some(existing_token) = auth_state.token {
        new_token.profile_image_url = existing_token.profile_image_url;
    }
    state_syncer.update::<AuthState>(
        "auth_state",
        AuthState {
            phase: types::AuthPhase::Authorized,
            device_code: String::new(),
            token: Some(new_token.clone()),
        },
        true,
    );
    info!(
        login = %new_token.login,
        user_id = %new_token.user_id,
        expires_in = new_token.expires_in,
        "token refresh persisted to auth_state"
    );
}

/// Best-effort token maintenance: optionally validate, then refresh the shared
/// token when it is within 10 minutes of expiry. Transient failures are logged
/// and retried on the next call; a genuinely dead token is caught reactively by
/// the channel-cache 401 path (`handle_channel_cache_poll_error`). No-ops unless
/// authorized. A non-blocking `RefreshLock` prevents overlapping refreshes
/// (supervisor tick vs. focus event).
async fn refresh_token_if_needed(app_handle: &AppHandle, force_validate: bool) {
    let state_syncer = app_handle.state::<StateSyncer>();
    if !matches!(
        state_syncer.snapshot::<AuthState>("auth_state").phase,
        types::AuthPhase::Authorized
    ) {
        return;
    }

    let Some(shared_token) = app_handle.try_state::<SharedTwitchToken>() else {
        return;
    };

    let refresh_lock = app_handle.state::<RefreshLock>();
    let _refresh_guard = match refresh_lock.try_lock() {
        Ok(guard) => guard,
        Err(_) => return, // another refresh is already in progress
    };

    let client = app_handle.state::<HelixClient<'static, reqwest::Client>>();
    let snapshot = { shared_token.lock().await.clone() };

    if force_validate {
        if let Err(e) = snapshot.validate_token(client.inner()).await {
            warn!("token validation failed (will retry): {}", e);
        }
    }

    let remaining = snapshot.expires_in();
    info!("token: expires_in={:?}", remaining);
    if remaining >= Duration::from_secs(600) {
        return;
    }

    info!("refreshing token");
    let mut refreshed = snapshot.clone();
    match refreshed.refresh_token(client.inner()).await {
        Ok(_) => {
            let mut guard = shared_token.lock().await;
            // Skip the write-back if a concurrent login swapped the token underneath us.
            if guard.access_token.secret() == snapshot.access_token.secret() {
                *guard = refreshed.clone();
                drop(guard);
                persist_refreshed_token(state_syncer.inner(), &refreshed);
            }
        }
        Err(e) => warn!("token refresh failed (will retry next tick): {}", e),
    }
}

/// Long-lived token maintenance loop, spawned once at startup. It idles while
/// logged out and reads the shared token each tick, so it always operates on the
/// current session's token without being aborted/respawned on login. This single
/// owner replaces the old per-login refresh task, whose abort/respawn races could
/// leave it silently dead.
async fn token_refresh_supervisor(app_handle: AppHandle) {
    info!("token refresh supervisor started");
    let mut last_validation = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let force_validate = last_validation.elapsed() > Duration::from_secs(300);
        refresh_token_if_needed(&app_handle, force_validate).await;
        if force_validate {
            last_validation = Instant::now();
        }
    }
}

fn inbound_build_info() -> inbound::BuildInfo {
    inbound::BuildInfo {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        app_commit: option_env!("INBOUND_GIT_SHA")
            .unwrap_or("unknown")
            .to_owned(),
        build_time: option_env!("INBOUND_BUILD_TIME")
            .unwrap_or("unknown")
            .to_owned(),
    }
}

fn internal_build_info(build_info: &inbound::BuildInfo) -> internal::InternalBuildInfo {
    internal::InternalBuildInfo {
        app_version: build_info.app_version.clone(),
        app_commit: build_info.app_commit.clone(),
        build_time: build_info.build_time.clone(),
    }
}

struct PepoScrubber;

impl inbound::Scrubber<tauri::Wry> for PepoScrubber {
    fn scrub(&self, app: &AppHandle) -> Result<Option<serde_json::Value>, String> {
        let state_syncer = app.state::<StateSyncer>();
        let auth_state = state_syncer.snapshot::<AuthState>("auth_state");
        let settings = state_syncer.snapshot::<Settings>("settings").normalized();
        let mut value = serde_json::json!({
            "auth_state": auth_state,
            "settings": settings,
        });

        if let Some(auth) = value
            .get_mut("auth_state")
            .and_then(|value| value.as_object_mut())
        {
            if auth
                .get("device_code")
                .and_then(|value| value.as_str())
                .is_some_and(|value| !value.is_empty())
            {
                auth.insert(
                    "device_code".to_owned(),
                    serde_json::Value::String("[redacted]".to_owned()),
                );
            }

            if let Some(token) = auth
                .get_mut("token")
                .and_then(|value| value.as_object_mut())
            {
                token.insert(
                    "access_token".to_owned(),
                    serde_json::Value::String("[redacted]".to_owned()),
                );
                token.insert(
                    "refresh_token".to_owned(),
                    serde_json::Value::String("[redacted]".to_owned()),
                );
            }
        }

        Ok(Some(value))
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
    let pepo_log_level = logging::pepo_log_level();
    let inbound_build = inbound_build_info();
    let internal_build = internal_build_info(&inbound_build);

    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(9_000_000) // 9 MB per log file
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .level(log::LevelFilter::Info)
                .level_for("pepo_lib", pepo_log_level)
                .level_for("pepo_internal", pepo_log_level)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tauri_svelte_synced_store", log::LevelFilter::Warn)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                    inbound::capture::log_target(),
                ])
                .build(),
        )
        .plugin(inbound::init(inbound::Config {
            app: inbound::AppId::Pepo,
            build: inbound_build,
            scrubber: Some(Arc::new(PepoScrubber)),
        }))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build());

    let public_handlers = specta_builder();

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    export_bindings(&public_handlers, repo_root().join("src/lib/bindings.ts"));

    #[cfg(all(debug_assertions, internal_enabled))]
    export_bindings(
        &internal::specta_builder(),
        repo_root().join("internal/frontend/bindings.ts"),
    );

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            info!("{}, {argv:?}, {cwd}", app.package_info().name);
            app.emit("single-instance", argv).unwrap();
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build());

    let builder = internal::apply_plugins(builder, internal_build);

    let _builder = builder
        .invoke_handler(public_handlers.invoke_handler())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .unwrap();

            // This is also required if you want to use events
            public_handlers.mount_events(app);
            internal::setup(app)?;

            let mut sync_cfg = StateSyncerConfig::default();
            sync_cfg.persist_keys.insert("auth_state".to_owned(), true);
            sync_cfg
                .persist_keys
                .insert("internal_state".to_owned(), true);
            sync_cfg
                .persist_keys
                .insert("channel_cache".to_owned(), true);
            sync_cfg.persist_keys.insert("settings".to_owned(), true);
            let state_syncer = StateSyncer::new(sync_cfg, app.handle().clone());

            let _auth_state: AuthState = state_syncer.load("auth_state");
            let mut internal_state: InternalState = state_syncer.load("internal_state");
            let mut settings: Settings = state_syncer.load("settings");
            if settings.schema_version == 0 {
                settings.layout.sidebar_open = internal_state.sidebar_open;
            }
            settings = settings.normalized();
            internal_state.version = app.package_info().version.to_string();
            internal_state.name = app.package_info().name.to_string();

            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(&format!(
                    "{} {}",
                    internal_state.name.clone(),
                    internal_state.version.clone()
                ))
                .visible(false)
                .background_color(tauri::webview::Color(20, 26, 39, 255));

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

            let window = win_builder.build().unwrap();

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
                    DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
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

            // Revalidate/refresh the token whenever the window regains focus —
            // covers returning to the app after the machine has slept.
            {
                let focus_app = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(focused) = event {
                        if *focused {
                            let app = focus_app.clone();
                            tauri::async_runtime::spawn(async move {
                                info!("window focused; checking token freshness");
                                refresh_token_if_needed(&app, true).await;
                            });
                        }
                    }
                });
            }

            let client: HelixClient<'static, reqwest::Client> =
                twitch_api::HelixClient::with_client(
                    ClientDefault::default_client_with_name(Some(
                        "pepo".parse().expect("invalid client name"),
                    ))
                    .unwrap(),
                );

            info!("made client");

            state_syncer.set("internal_state", internal_state);
            state_syncer.set("channel_cache", ChannelCache::default());
            state_syncer.set("settings", settings);
            app.manage::<StateSyncer>(state_syncer);
            app.manage::<SharedPollHandle>(Mutex::new(None));
            app.manage::<RefreshLock>(Mutex::new(()));
            app.manage::<SharedEventSubHandles>(Mutex::new(Vec::new()));

            app.manage(client);

            // Single long-lived token maintenance loop; see token_refresh_supervisor.
            tauri::async_runtime::spawn(token_refresh_supervisor(app.handle().clone()));

            #[cfg(debug_assertions)]
            window.open_devtools();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

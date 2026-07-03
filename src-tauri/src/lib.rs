use eventsub::EventSubManager;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
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

use crate::badgemanager::BadgeManager;
use crate::emote::cache::EmoteCacheTrait;
use crate::emotemanager::EmoteManager;
use crate::types::{AppSettings, AuthState, ChannelCache};

mod badgemanager;
mod badgepersist;
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

type SharedEventSubManager = Mutex<EventSubManager>;
type SharedBadgeManager = Mutex<BadgeManager>;
type SharedEmoteManager = Mutex<EmoteManager>;
type SharedPollHandle = Mutex<Option<tauri::async_runtime::JoinHandle<()>>>;
type SharedEventSubHandles = Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>;
/// Guards against overlapping token refreshes (supervisor tick vs. focus event).
type RefreshLock = Mutex<()>;

const APP_SETTINGS_KEY: &str = "app_settings";

tauri_svelte_synced_store::state_handlers!(
    AuthState = "auth_state",
    InternalState = "internal_state",
    ChannelCache = "channel_cache",
    AppSettings = "app_settings"
);

fn app_settings(state_syncer: &StateSyncer) -> AppSettings {
    state_syncer
        .snapshot::<AppSettings>(APP_SETTINGS_KEY)
        .normalized()
}

fn make_eventsub_settings_reader(state_syncer: StateSyncer) -> eventsub::EventSubSettingsReader {
    Arc::new(move || app_settings(&state_syncer).eventsub)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChannelCacheCandidate {
    broadcaster_id: String,
    login: String,
    display_name: String,
}

impl ChannelCacheCandidate {
    fn new(broadcaster_id: String, login: String, display_name: String) -> Self {
        let login = normalize_channel_login(&login);
        let display_name = if display_name.trim().is_empty() {
            login.clone()
        } else {
            display_name
        };

        Self {
            broadcaster_id,
            login,
            display_name,
        }
    }
}

fn normalize_channel_login(login: &str) -> String {
    login.trim().to_lowercase()
}

fn merge_channel_cache_candidates(
    followed: Vec<ChannelCacheCandidate>,
    joined: Vec<ChannelCacheCandidate>,
) -> Vec<ChannelCacheCandidate> {
    let mut candidates = HashMap::new();

    for candidate in followed {
        candidates.insert(candidate.login.clone(), candidate);
    }

    for candidate in joined {
        candidates
            .entry(candidate.login.clone())
            .or_insert(candidate);
    }

    let mut candidates: Vec<_> = candidates.into_values().collect();
    candidates.sort_by(|a, b| a.login.cmp(&b.login));
    candidates
}

fn channel_cache_from_candidates(
    candidates: &[ChannelCacheCandidate],
    users: &HashMap<String, twitch_api::helix::users::User>,
    live_streams: &HashMap<String, types::Stream>,
    previous_cache: &types::ChannelCache,
) -> types::ChannelCache {
    let mut cache = types::ChannelCache::default();

    for candidate in candidates {
        upsert_channel_cache_candidate(
            &mut cache,
            candidate,
            users.get(&candidate.login),
            live_streams.get(&candidate.login).cloned(),
            previous_cache.channels.get(&candidate.login),
        );
    }

    cache
}

fn upsert_channel_cache_candidate(
    cache: &mut types::ChannelCache,
    candidate: &ChannelCacheCandidate,
    user: Option<&twitch_api::helix::users::User>,
    stream: Option<types::Stream>,
    previous_status: Option<&types::ChannelStatus>,
) {
    let display_name = user
        .map(|user| user.display_name.to_string())
        .filter(|display_name| !display_name.trim().is_empty())
        .or_else(|| {
            (!candidate.display_name.trim().is_empty()).then(|| candidate.display_name.clone())
        })
        .or_else(|| previous_status.map(|status| status.display_name.clone()))
        .unwrap_or_else(|| candidate.login.clone());
    let profile_image_url = user
        .and_then(|user| user.profile_image_url.clone())
        .or_else(|| previous_status.map(|status| status.profile_image_url.clone()))
        .unwrap_or_default();
    let is_live = stream.is_some();

    cache.channels.insert(
        candidate.login.clone(),
        types::ChannelStatus {
            broadcaster_id: candidate.broadcaster_id.clone(),
            login: candidate.login.clone(),
            display_name,
            profile_image_url,
            is_live,
            stream,
        },
    );
}

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
        .typ::<types::AppSettings>()
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
    token_manager: State<'_, TokenManager>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!(
        "sending chat message: broadcaster_id={}, message={}",
        broadcaster_id, message
    );

    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token_manager.active_twitch_token())
        .ok_or_else(|| "no active token".to_owned())?;
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
    let settings = app_settings(state_syncer.inner());
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
    token_manager: State<'_, TokenManager>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<types::ChannelInfo, String> {
    debug!("get_channel_info: channel={}", channel_name);

    let token = token_manager
        .active_twitch_token()
        .await
        .ok_or_else(|| "no active token".to_owned())?;
    let client = client_ref.inner();

    let channel = client
        .get_channel_from_login(&channel_name, &token)
        .await
        .map_err(|e| format!("failed to get channel: {:?}", e))?
        .ok_or_else(|| format!("channel not found: {}", channel_name))?;

    let mut channel_info = types::ChannelInfo::from(channel);

    if let Ok(Some(user)) = client
        .get_user_from_id(channel_info.broadcaster_id.as_str(), &token)
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
    app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    badge_manager_ref: State<'_, SharedBadgeManager>,
    emote_manager_ref: State<'_, SharedEmoteManager>,
    token_manager: State<'_, TokenManager>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
    state_syncer: State<'_, StateSyncer>,
) -> Result<types::ChannelInfo, String> {
    debug!("join: channel={}", channel_name);

    let token = token_manager
        .active_twitch_token()
        .await
        .ok_or_else(|| "no active token".to_owned())?;
    let client = client_ref.inner().clone();
    let eventsub_manager = eventsub_manager_ref.lock().await.clone();
    let badge_manager = badge_manager_ref.lock().await.clone();
    let emote_manager = emote_manager_ref.lock().await.clone();
    let settings = app_settings(state_syncer.inner());

    let channel = client
        .get_channel_from_login(&channel_name, &token)
        .await
        .map_err(|e| format!("failed to get channel: {:?}", e))?
        .ok_or_else(|| format!("channel not found: {}", channel_name))?;

    debug!("join: got channel info - {:?}", channel.clone());

    let channel_info = types::ChannelInfo::from(channel.clone());
    let broadcaster_id = channel.broadcaster_id.to_string();
    let emote_settings = settings.emotes;
    let provider_settings = settings.providers;

    spawn_joined_channel_cache_refresh(app_handle, channel_info.clone(), token.clone());

    badge_manager.hydrate_global(&provider_settings).await;
    badge_manager
        .hydrate_channel(&broadcaster_id, &provider_settings)
        .await;
    emote_manager.preload(&broadcaster_id, &emote_settings, &provider_settings);

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

    tauri::async_runtime::spawn(async move {
        debug!(
            broadcaster_id,
            channel = channel_name,
            "loading channel cosmetics after EventSub join"
        );
        let badge_broadcaster_id = broadcaster_id.clone();
        let emote_broadcaster_id = broadcaster_id.clone();
        let load_badges =
            badge_manager.load_channel(badge_broadcaster_id, client, &provider_settings);
        let load_emotes =
            emote_manager.load_channel(emote_broadcaster_id, &emote_settings, &provider_settings);
        tokio::join!(load_badges, load_emotes);
    });

    Ok(channel_info)
}

#[tauri::command]
#[specta::specta]
async fn leave_chat(
    channel_name: String,
    _app_handle: AppHandle,
    eventsub_manager_ref: State<'_, SharedEventSubManager>,
    token_manager: State<'_, TokenManager>,
    client_ref: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), String> {
    debug!("leave: channel={}", channel_name);

    let token = token_manager
        .active_twitch_token()
        .await
        .ok_or_else(|| "no active token".to_owned())?;
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
    token_manager: State<'_, TokenManager>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<Vec<types::Stream>, String> {
    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token_manager.active_twitch_token())
        .ok_or_else(|| "no active token".to_owned())?;
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
    token_manager: State<'_, TokenManager>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
    state_syncer: State<'_, StateSyncer>,
) -> Result<Vec<types::Broadcaster>, String> {
    let client = client.inner();
    let channel_cache_settings = app_settings(state_syncer.inner()).channel_cache;

    let token_guard = tauri::async_runtime::block_on(token_manager.active_twitch_token())
        .ok_or_else(|| "no active token".to_owned())?;
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
    for chunk in channels.chunks(channel_cache_settings.user_lookup_chunk_size) {
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

fn spawn_joined_channel_cache_refresh(
    app_handle: AppHandle,
    channel_info: types::ChannelInfo,
    token: twitch_oauth2::UserToken,
) {
    tauri::async_runtime::spawn(async move {
        let login = channel_info.broadcaster_login.clone();
        if let Err(err) = refresh_joined_channel_cache(&app_handle, channel_info, &token).await {
            warn!(
                channel = login,
                "failed to refresh joined channel cache: {}", err
            );
        }
    });
}

async fn refresh_joined_channel_cache(
    app_handle: &AppHandle,
    channel_info: types::ChannelInfo,
    token: &twitch_oauth2::UserToken,
) -> Result<(), String> {
    let client_ref = app_handle.state::<HelixClient<'static, reqwest::Client>>();
    let client = client_ref.inner();
    let state_syncer = app_handle.state::<StateSyncer>();
    let candidate = ChannelCacheCandidate::new(
        channel_info.broadcaster_id,
        channel_info.broadcaster_login,
        channel_info.broadcaster_name,
    );

    let user = match client
        .get_user_from_id(candidate.broadcaster_id.as_str(), token)
        .await
    {
        Ok(user) => user,
        Err(err) => {
            warn!(
                channel = candidate.login,
                "failed to fetch joined channel user metadata: {}", err
            );
            None
        }
    };

    let live_streams = fetch_live_streams_for_logins(client, &[candidate.login.clone()], token)
        .await
        .map_err(|err| format!("failed to get joined channel stream: {err}"))?;
    let mut cache = state_syncer.snapshot::<types::ChannelCache>("channel_cache");
    let previous_status = cache.channels.get(&candidate.login).cloned();

    upsert_channel_cache_candidate(
        &mut cache,
        &candidate,
        user.as_ref(),
        live_streams.get(&candidate.login).cloned(),
        previous_status.as_ref(),
    );
    state_syncer.update::<types::ChannelCache>("channel_cache", cache, true);

    Ok(())
}

async fn fetch_live_streams_for_logins(
    client: &HelixClient<'static, reqwest::Client>,
    logins: &[String],
    token: &twitch_oauth2::UserToken,
) -> Result<HashMap<String, types::Stream>, String> {
    if logins.is_empty() {
        return Ok(HashMap::new());
    }

    let login_collection = twitch_api::types::Collection::from(logins);
    let streams: Vec<twitch_api::helix::streams::Stream> = client
        .get_streams_from_logins(&login_collection, token)
        .try_collect()
        .await
        .map_err(|e| e.to_string())?;

    Ok(streams
        .into_iter()
        .map(|stream| {
            let stream = types::Stream::from(stream);
            (normalize_channel_login(&stream.user_login), stream)
        })
        .collect())
}

async fn poll_channel_cache(app_handle: &AppHandle) -> Result<(), String> {
    let token = app_handle
        .state::<TokenManager>()
        .active_twitch_token()
        .await
        .ok_or_else(|| "no active token".to_owned())?;
    let client = app_handle.state::<HelixClient<'static, reqwest::Client>>();
    let client = client.inner();
    let state_syncer = app_handle.state::<StateSyncer>();
    let channel_cache_settings = app_settings(state_syncer.inner()).channel_cache;
    let previous_cache = state_syncer.snapshot::<types::ChannelCache>("channel_cache");
    let desired_channels = {
        let eventsub_manager = app_handle
            .state::<SharedEventSubManager>()
            .lock()
            .await
            .clone();
        eventsub_manager.desired_channels_snapshot()
    };

    debug!("polling channel cache");

    let channels: Vec<twitch_api::helix::channels::FollowedBroadcaster> = client
        .get_followed_channels(token.user_id.to_string(), &token)
        .try_collect()
        .await
        .map_err(|e| format!("failed to get followed channels: {}", e))?;

    let followed_candidates: Vec<ChannelCacheCandidate> = channels
        .iter()
        .map(|channel| {
            ChannelCacheCandidate::new(
                channel.broadcaster_id.to_string(),
                channel.broadcaster_login.to_string(),
                channel.broadcaster_name.to_string(),
            )
        })
        .collect();
    let joined_candidates: Vec<ChannelCacheCandidate> = desired_channels
        .into_iter()
        .map(|(login, id)| {
            let login = normalize_channel_login(&login);
            let display_name = previous_cache
                .channels
                .get(&login)
                .map(|status| status.display_name.clone())
                .unwrap_or_else(|| login.clone());
            ChannelCacheCandidate::new(id.to_string(), login, display_name)
        })
        .collect();
    let candidates = merge_channel_cache_candidates(followed_candidates, joined_candidates);

    let mut users: HashMap<String, twitch_api::helix::users::User> = HashMap::new();
    for chunk in candidates.chunks(channel_cache_settings.user_lookup_chunk_size) {
        let ids: Vec<twitch_api::types::UserId> = chunk
            .iter()
            .map(|candidate| twitch_api::types::UserId::new(candidate.broadcaster_id.clone()))
            .collect();
        let id_collection = twitch_api::types::Collection::from(ids);
        let chunk_users: Vec<twitch_api::helix::users::User> = client
            .get_users_from_ids(&id_collection, &token)
            .try_collect()
            .await
            .map_err(|e| format!("failed to get users: {}", e))?;
        for user in chunk_users {
            users.insert(user.login.to_string(), user);
        }
    }

    let logins: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.login.clone())
        .collect();
    let live_streams = fetch_live_streams_for_logins(client, &logins, &token)
        .await
        .map_err(|e| format!("failed to get streams: {}", e))?;
    let cache = channel_cache_from_candidates(&candidates, &users, &live_streams, &previous_cache);

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

    let channel_cache_settings =
        app_settings(app_handle.state::<StateSyncer>().inner()).channel_cache;
    let message = format!("{context} channel cache poll failed: {error}");
    if channel_cache_settings.error_log_throttle_enabled {
        logging::error_repeated(
            format!("channel_cache_poll:{context}:{error}"),
            message,
            channel_cache_settings.error_log_throttle(),
        );
    } else {
        error!("{message}");
    }
    false
}

#[tauri::command]
#[specta::specta]
async fn login(
    app_handle: AppHandle,
    state_syncer: State<'_, StateSyncer>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
    token_manager: State<'_, TokenManager>,
    quick: bool,
) -> Result<types::UserToken, String> {
    let client = client.inner();
    let token_manager = token_manager.inner().clone();
    info!(quick, "login");

    let auth_state_snapshot = state_syncer.snapshot::<AuthState>("auth_state");
    token_manager
        .ensure_loaded(auth_state_snapshot.token.clone())
        .await?;

    let twitch_token: twitch_oauth2::UserToken;
    let user_token: types::UserToken;
    if clear_stale_authorized_auth_if_needed(&app_handle, &token_manager, &auth_state_snapshot)
        .await
    {
        if quick {
            return Err("no valid token".to_string());
        }
    }

    if token_manager.is_active_token_valid().await {
        debug!("token is already valid");
        twitch_token = token_manager
            .active_twitch_token()
            .await
            .ok_or_else(|| "no active token".to_owned())?;
        user_token = token_manager
            .active_public_token()
            .await
            .ok_or_else(|| "no active token".to_owned())?;
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
        tokio::time::sleep(
            app_settings(state_syncer.inner())
                .auth
                .login_activation_delay(),
        )
        .await;

        info!("login {}", device_code.verification_uri);
        app_handle
            .opener()
            .open_url(device_code.verification_uri.clone(), None::<&str>)
            .unwrap();

        twitch_token = token_manager.finish_device_code_flow().await;
        user_token = token_manager
            .active_public_token()
            .await
            .ok_or_else(|| "no active token".to_owned())?;
    }

    {
        let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
        let mut auth_state = auth_state_ref.lock().unwrap();

        auth_state.phase = types::AuthPhase::Authorized;
        auth_state.device_code = String::new();
        auth_state.token = Some(user_token.clone());
    }

    // Create empty managers — no network calls, just register state so
    // other Tauri commands can access them immediately.
    let eventsub_manager = EventSubManager::new();
    let badge_manager = BadgeManager::empty(token_manager.clone(), app_handle.clone());
    let emote_manager =
        EmoteManager::empty(client.clone(), token_manager.clone(), app_handle.clone());

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
    // spawned once at startup.

    // --- Background tasks ---

    // Profile image fetch
    {
        let client = client.clone();
        let twitch_token = twitch_token.clone();
        let token_manager = token_manager.clone();
        let state_syncer = state_syncer.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(Some(user_info)) = client
                .get_user_from_id(&twitch_token.user_id, &twitch_token)
                .await
            {
                match token_manager
                    .update_profile_image(
                        twitch_token.user_id.as_str(),
                        user_info.profile_image_url.unwrap_or_default(),
                    )
                    .await
                {
                    Ok(Some(token)) => persist_authorized_token(&state_syncer, token),
                    Ok(None) => {}
                    Err(err) => error!("failed to persist profile image: {}", err),
                }
            }
        });
    }

    // Global badges
    {
        let badge_manager = badge_manager.clone();
        let client = client.clone();
        let provider_settings = app_settings(state_syncer.inner()).providers;
        tauri::async_runtime::spawn(async move {
            if let Err(e) = badge_manager.load_global(client, &provider_settings).await {
                error!("failed to load global badges: {}", e);
            }
        });
    }

    // Global + user emotes
    {
        let emote_manager = emote_manager.clone();
        let settings = app_settings(state_syncer.inner());
        let emote_settings = settings.emotes;
        let provider_settings = settings.providers;
        tauri::async_runtime::spawn(async move {
            emote_manager.load_global(&emote_settings, &provider_settings);
            emote_manager.load_user_emotes();
        });
    }

    // EventSub connection + message processing thread
    {
        let eventsub_manager = eventsub_manager.clone();
        let client = client.clone();
        let token_manager = token_manager.clone();
        let app_ref = app_handle.clone();
        let badge_manager_ref = badge_manager.clone();
        let emote_manager_ref = emote_manager.clone();
        let state_syncer_ref = state_syncer.inner().clone();
        let eventsub_settings_reader = make_eventsub_settings_reader(state_syncer.inner().clone());

        {
            let eventsub_handle_state = app_handle.state::<SharedEventSubHandles>();
            let mut existing = eventsub_handle_state.lock().await;
            for handle in existing.drain(..) {
                handle.abort();
            }
        }

        let eventsub_runtime =
            match eventsub_manager.start(client, token_manager, eventsub_settings_reader) {
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
                                let settings = app_settings(&state_syncer_ref);
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
            let channel_cache_settings =
                app_settings(poll_app.state::<StateSyncer>().inner()).channel_cache;
            tokio::time::sleep(channel_cache_settings.poll_interval()).await;

            let channel_cache_settings =
                app_settings(poll_app.state::<StateSyncer>().inner()).channel_cache;
            if !channel_cache_settings.recurring_poll_enabled {
                continue;
            }

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

    if let Some(token_manager) = app_handle.try_state::<TokenManager>() {
        if let Err(err) = token_manager.remove_active_token().await {
            error!("failed to remove active token while clearing auth: {}", err);
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

/// Clears auth state, channel cache, and background auth tasks.
/// Used by both explicit logout and automatic auth expiration handling.
fn clear_auth(app_handle: &AppHandle) {
    tauri::async_runtime::block_on(clear_auth_async(app_handle, true));
}

fn persist_authorized_token(state_syncer: &StateSyncer, token: types::UserToken) {
    state_syncer.update::<AuthState>(
        "auth_state",
        AuthState {
            phase: types::AuthPhase::Authorized,
            device_code: String::new(),
            token: Some(token.clone()),
        },
        true,
    );
    info!(
        login = %token.login,
        user_id = %token.user_id,
        expires_in = token.expires_in,
        "active token persisted to auth_state"
    );
}

fn is_stale_authorized_auth(auth_state: &AuthState, has_active_runtime_token: bool) -> bool {
    matches!(auth_state.phase, types::AuthPhase::Authorized) && !has_active_runtime_token
}

async fn clear_stale_authorized_auth_if_needed(
    app_handle: &AppHandle,
    token_manager: &TokenManager,
    auth_state: &AuthState,
) -> bool {
    if !is_stale_authorized_auth(
        auth_state,
        token_manager.active_public_token().await.is_some(),
    ) {
        return false;
    }

    warn!(
        login = auth_state
            .token
            .as_ref()
            .map(|token| token.login.as_str())
            .unwrap_or(""),
        user_id = auth_state
            .token
            .as_ref()
            .map(|token| token.user_id.as_str())
            .unwrap_or(""),
        "authorized auth_state has no active runtime token; clearing auth"
    );
    clear_auth_async(app_handle, false).await;
    true
}

/// Best-effort token maintenance for every token currently loaded by the
/// manager. A non-blocking `RefreshLock` prevents overlapping refreshes
/// (supervisor tick vs. focus event).
async fn refresh_token_if_needed(app_handle: &AppHandle, force_validate: bool) {
    let state_syncer = app_handle.state::<StateSyncer>();
    let Some(token_manager) = app_handle.try_state::<TokenManager>() else {
        return;
    };

    let refresh_lock = app_handle.state::<RefreshLock>();
    let _refresh_guard = match refresh_lock.try_lock() {
        Ok(guard) => guard,
        Err(_) => return, // another refresh is already in progress
    };

    let auth_state = state_syncer.snapshot::<AuthState>("auth_state");
    if let Err(err) = token_manager.ensure_loaded(auth_state.token.clone()).await {
        warn!("failed to load persisted tokens for refresh: {}", err);
        return;
    }

    if clear_stale_authorized_auth_if_needed(app_handle, token_manager.inner(), &auth_state).await {
        return;
    }

    let auth_settings = app_settings(state_syncer.inner()).auth;
    match token_manager
        .refresh_all_tokens(force_validate, auth_settings.refresh_threshold())
        .await
    {
        Ok(Some(active_token)) if matches!(auth_state.phase, types::AuthPhase::Authorized) => {
            persist_authorized_token(state_syncer.inner(), active_token);
        }
        Ok(_) => {}
        Err(err) => warn!("token refresh failed (will retry next tick): {}", err),
    }
}

/// Long-lived token maintenance loop, spawned once at startup.
async fn token_refresh_supervisor(app_handle: AppHandle) {
    info!("token refresh supervisor started");
    let mut last_validation = Instant::now();
    loop {
        let auth_settings = app_settings(app_handle.state::<StateSyncer>().inner()).auth;
        tokio::time::sleep(auth_settings.refresh_supervisor_tick()).await;

        let auth_settings = app_settings(app_handle.state::<StateSyncer>().inner()).auth;
        let force_validate = last_validation.elapsed() > auth_settings.validation_interval();
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
        let settings = app_settings(state_syncer.inner());
        let mut value = serde_json::json!({
            "auth_state": auth_state,
            "app_settings": settings,
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
            sync_cfg
                .persist_keys
                .insert(APP_SETTINGS_KEY.to_owned(), true);
            let state_syncer = StateSyncer::new(sync_cfg, app.handle().clone());

            let _auth_state: AuthState = state_syncer.load("auth_state");
            let mut internal_state: InternalState = state_syncer.load("internal_state");
            let settings: AppSettings = state_syncer.load(APP_SETTINGS_KEY);
            let settings = settings.normalized();
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
            state_syncer.set(APP_SETTINGS_KEY, settings);
            app.manage::<StateSyncer>(state_syncer);
            app.manage::<SharedPollHandle>(Mutex::new(None));
            app.manage::<RefreshLock>(Mutex::new(()));
            app.manage::<SharedEventSubHandles>(Mutex::new(Vec::new()));

            let token_manager = TokenManager::new(client.clone(), app.handle().clone());
            app.manage(token_manager);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_state(phase: types::AuthPhase, token: Option<types::UserToken>) -> AuthState {
        AuthState {
            phase,
            device_code: String::new(),
            token,
        }
    }

    fn user_token() -> types::UserToken {
        types::UserToken {
            access_token: "access".to_owned(),
            client_id: "client".to_owned(),
            login: "viewer".to_owned(),
            user_id: "1234".to_owned(),
            refresh_token: Some("refresh".to_owned()),
            expires_in: 3600,
            profile_image_url: String::new(),
        }
    }

    fn stream(login: &str, title: &str) -> types::Stream {
        types::Stream {
            game_id: "game-id".to_owned(),
            game_name: "Game".to_owned(),
            id: format!("stream-{login}"),
            language: "en".to_owned(),
            is_mature: false,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            tags: Vec::new(),
            thumbnail_url: String::new(),
            title: title.to_owned(),
            user_id: format!("user-{login}"),
            user_name: login.to_owned(),
            user_login: login.to_owned(),
            viewer_count: 42,
        }
    }

    #[test]
    fn merged_channel_cache_candidates_keep_followed_metadata() {
        let candidates = merge_channel_cache_candidates(
            vec![ChannelCacheCandidate::new(
                "1".to_owned(),
                "Maya".to_owned(),
                "Maya Followed".to_owned(),
            )],
            vec![
                ChannelCacheCandidate::new(
                    "1".to_owned(),
                    "maya".to_owned(),
                    "Maya Joined".to_owned(),
                ),
                ChannelCacheCandidate::new(
                    "2".to_owned(),
                    "luna".to_owned(),
                    "Luna Joined".to_owned(),
                ),
            ],
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].login, "luna");
        assert_eq!(candidates[0].display_name, "Luna Joined");
        assert_eq!(candidates[1].login, "maya");
        assert_eq!(candidates[1].display_name, "Maya Followed");
    }

    #[test]
    fn channel_cache_from_candidates_marks_live_and_offline_channels() {
        let candidates = vec![
            ChannelCacheCandidate::new("1".to_owned(), "maya".to_owned(), "Maya".to_owned()),
            ChannelCacheCandidate::new("2".to_owned(), "luna".to_owned(), "Luna".to_owned()),
        ];
        let live_streams = HashMap::from([("maya".to_owned(), stream("maya", "Live title"))]);

        let cache = channel_cache_from_candidates(
            &candidates,
            &HashMap::new(),
            &live_streams,
            &types::ChannelCache::default(),
        );

        let live = cache.channels.get("maya").unwrap();
        assert!(live.is_live);
        assert_eq!(live.stream.as_ref().unwrap().title, "Live title");

        let offline = cache.channels.get("luna").unwrap();
        assert!(!offline.is_live);
        assert!(offline.stream.is_none());
        assert_eq!(offline.display_name, "Luna");
    }

    #[test]
    fn stale_authorized_auth_requires_repair_without_runtime_token() {
        let auth_state = auth_state(types::AuthPhase::Authorized, Some(user_token()));

        assert!(is_stale_authorized_auth(&auth_state, false));
    }

    #[test]
    fn authorized_auth_with_runtime_token_does_not_require_repair() {
        let auth_state = auth_state(types::AuthPhase::Authorized, Some(user_token()));

        assert!(!is_stale_authorized_auth(&auth_state, true));
    }

    #[test]
    fn non_authorized_auth_without_runtime_token_does_not_require_repair() {
        for phase in [
            types::AuthPhase::Unauthorized,
            types::AuthPhase::WaitingForDeviceCode,
            types::AuthPhase::WaitingForAuth,
            types::AuthPhase::FailedAuth,
        ] {
            let auth_state = auth_state(phase, None);

            assert!(!is_stale_authorized_auth(&auth_state, false));
        }
    }
}

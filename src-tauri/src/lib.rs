use eventsub::EventSubManager;
use futures::TryStreamExt;
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use tauri_specta::collect_commands;
use tauri_svelte_synced_store::{StateSyncer, StateSyncerConfig};
use token::TokenManager;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use twitch_api::HelixClient;

use crate::badgemanager::BadgeManager;
use crate::emote::cache::EmoteCacheTrait;
use crate::emotemanager::EmoteManager;
use crate::types::{AppSettings, AuthState, ChannelCache};

mod badgemanager;
mod badgepersist;
pub mod chat;
use chat::runtime::{
    acknowledge_chat, get_chat_engine_status, get_chat_feed_context, post_system_message,
    subscribe_chat, unsubscribe_chat, ChatRuntime,
};
#[cfg(test)]
mod channel_cache_poll_tests;
mod chat_send;
mod emote;
mod emotemanager;
mod eventsub;
mod internal;
mod logging;
mod message;
mod network;
use network::{
    begin_background_request, end_background_request, get_network_status, retry_network_operation,
    NetworkRuntime,
};
#[cfg(target_os = "linux")]
mod platform;
mod token;
mod types;
mod usercard;
mod window_chrome;
use usercard::{
    get_user_card_state, open_user_card, ready_user_card, refresh_user_card, set_user_card_pinned,
    UserCardRuntime,
};

type SharedEventSubManager = Mutex<EventSubManager>;
type SharedBadgeManager = Mutex<BadgeManager>;
type SharedEmoteManager = Mutex<EmoteManager>;
type SharedPollHandle = Mutex<Option<tauri::async_runtime::JoinHandle<()>>>;
type SharedEventSubHandles = Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>;

const APP_SETTINGS_KEY: &str = "app_settings";

#[tauri::command]
#[specta::specta]
fn update_state(
    state: tauri_svelte_synced_store::StateUpdate,
    app_handle: AppHandle,
    state_syncer: State<'_, StateSyncer>,
) -> Result<bool, tauri_svelte_synced_store::SyncError> {
    let is_app_settings = state.name == APP_SETTINGS_KEY;
    state_syncer.update_from_frontend(state)?;
    if is_app_settings {
        if let Some(chat_runtime) = app_handle.try_state::<ChatRuntime>() {
            if let Ok(engine) = chat_runtime.current() {
                let settings = app_settings(&state_syncer);
                engine.configure_translation(settings.translation);
                engine.configure(settings.chat);
            }
        }
    }
    Ok(true)
}

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
    let is_partner = user
        .map(|user| user.broadcaster_type == Some(twitch_api::types::BroadcasterType::Partner))
        .or_else(|| previous_status.map(|status| status.is_partner))
        .unwrap_or_default();

    cache.channels.insert(
        candidate.login.clone(),
        types::ChannelStatus {
            broadcaster_id: candidate.broadcaster_id.clone(),
            login: candidate.login.clone(),
            display_name,
            profile_image_url,
            is_live,
            is_partner,
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
pub fn export_bindings(
    builder: &tauri_specta::Builder<tauri::Wry>,
    path: impl AsRef<std::path::Path>,
) {
    struct RequiredContainerDefaults(Typescript);

    impl tauri_specta::LanguageExt for RequiredContainerDefaults {
        type Error = <Typescript as tauri_specta::LanguageExt>::Error;

        fn export(
            self,
            cfg: &tauri_specta::BuilderConfiguration,
            path: &std::path::Path,
        ) -> Result<(), Self::Error> {
            let mut cfg = cfg.clone();
            // Container-level #[serde(default)] supports reading partial disk stores;
            // serialized IPC payloads always include every field.
            cfg.types.iter_mut(|ndt| {
                if let Some(specta::datatype::DataType::Struct(strct)) = ndt.ty.as_mut() {
                    strct
                        .attributes
                        .remove(specta::markers::SERDE_CONTAINER_DEFAULT);
                }
            });
            self.0.export(&cfg, path)
        }
    }

    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create bindings output directory");
    }

    builder
        .export(
            RequiredContainerDefaults(Typescript::default().header("/* eslint-disable */")),
            path,
        )
        .expect("Failed to export typescript bindings");

    let raw = std::fs::read_to_string(path).expect("Failed to read exported bindings");
    let cleaned = raw
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if cleaned != raw {
        std::fs::write(path, cleaned).expect("Failed to normalize exported bindings");
    }
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
        .dangerously_cast_bigints_to_number()
        .disable_serde_phases()
        .typ::<types::UserToken>()
        .typ::<types::ChannelMessage>()
        .typ::<types::ChannelMessageTranslation>()
        .typ::<types::ChannelMessageTranslationUpdate>()
        .typ::<types::AuthState>()
        .typ::<types::AuthPhase>()
        .typ::<types::ChannelCache>()
        .typ::<types::ChannelStatus>()
        .typ::<types::AppSettings>()
        .commands(collect_commands![
            get_followed_streams,
            get_followed_channels,
            get_channel_info,
            subscribe_chat,
            post_system_message,
            unsubscribe_chat,
            acknowledge_chat,
            get_chat_engine_status,
            get_chat_feed_context,
            login,
            logout,
            send_chat_message,
            tauri_svelte_synced_store::commands::subscribe_state,
            tauri_svelte_synced_store::commands::unsubscribe_state,
            update_state,
            search_emotes,
            get_network_status,
            begin_background_request,
            end_background_request,
            retry_network_operation,
            open_user_card,
            get_user_card_state,
            ready_user_card,
            set_user_card_pinned,
            refresh_user_card,
        ])
        .events(tauri_specta::collect_events![])
}

#[tauri::command]
#[specta::specta]
fn send_chat_message(
    broadcaster_id: String,
    message: String,
    diagnostics: Option<chat_send::ChatSendDiagnostics>,
    _app_handle: AppHandle,
    token_manager: State<'_, TokenManager>,
    client: State<'_, HelixClient<'static, reqwest::Client>>,
) -> Result<(), chat_send::ChatSendError> {
    let attempt = chat_send::SendAttempt::new(&broadcaster_id, &message, diagnostics);
    let client = client.inner();

    let token_guard = tauri::async_runtime::block_on(token_manager.active_twitch_token())
        .ok_or_else(|| attempt.missing_token())?;
    let user_id = token_guard.user_id.clone();
    match tauri::async_runtime::block_on(client.send_chat_message(
        broadcaster_id.as_str(),
        user_id.as_str(),
        message.as_str(),
        &token_guard,
    )) {
        Ok(response) => attempt.response(user_id.as_str(), response),
        Err(error) => Err(attempt.request_error(user_id.as_str(), error)),
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
    channel_info: types::ChatRoomInfo,
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
    channel_info: types::ChatRoomInfo,
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

    let live_streams =
        fetch_live_streams_for_logins(client, std::slice::from_ref(&candidate.login), token)
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

#[allow(clippy::result_large_err)]
async fn fetch_live_streams_for_logins<C: twitch_api::HttpClient>(
    client: &HelixClient<'_, C>,
    logins: &[String],
    token: &twitch_oauth2::UserToken,
) -> Result<HashMap<String, types::Stream>, twitch_api::helix::ClientRequestError<C::Error>> {
    if logins.is_empty() {
        return Ok(HashMap::new());
    }

    let login_collection = twitch_api::types::Collection::from(logins);
    let streams: Vec<twitch_api::helix::streams::Stream> = client
        .get_streams_from_logins(&login_collection, token)
        .try_collect()
        .await?;

    Ok(streams
        .into_iter()
        .map(|stream| {
            let stream = types::Stream::from(stream);
            (normalize_channel_login(&stream.user_login), stream)
        })
        .collect())
}

#[cfg(test)]
const CHANNEL_CACHE_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug)]
struct ChannelCacheRequestError {
    category: &'static str,
    status: Option<u16>,
}

impl std::fmt::Display for ChannelCacheRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} status={:?}", self.category, self.status)
    }
}

impl std::error::Error for ChannelCacheRequestError {}

fn channel_cache_request_category(error: &(dyn std::error::Error + 'static)) -> &'static str {
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        if error.is_timeout() {
            "timeout"
        } else if error.is_connect() {
            "connect"
        } else if error.is_body() {
            "response_body"
        } else {
            "request"
        }
    } else if let Some(error) = error.downcast_ref::<std::io::Error>() {
        match error.kind() {
            std::io::ErrorKind::TimedOut => "timeout",
            std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted => "connect",
            _ => "request",
        }
    } else {
        "request"
    }
}

struct ChannelCacheTransport<'a, C> {
    inner: &'a C,
    timeout: std::time::Duration,
}

impl<'a, C: twitch_api::HttpClient> twitch_api::HttpClient for ChannelCacheTransport<'a, C> {
    type Error = ChannelCacheRequestError;

    fn req(
        &self,
        request: twitch_api::client::Request,
    ) -> impl std::future::Future<Output = Result<twitch_api::client::Response, Self::Error>>
           + Send
           + use<'a, C> {
        let response = self.inner.req(request);
        let timeout = self.timeout;
        async move {
            let response = tokio::time::timeout(timeout, response)
                .await
                .map_err(|_| ChannelCacheRequestError {
                    category: "timeout",
                    status: None,
                })?
                .map_err(|error| ChannelCacheRequestError {
                    category: channel_cache_request_category(&error),
                    status: None,
                })?;
            // Capture HTTP status before Helix substitutes the JSON error status.
            if !response.status().is_success() {
                return Err(ChannelCacheRequestError {
                    category: "http",
                    status: Some(response.status().as_u16()),
                });
            }
            Ok(response)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChannelCacheFetchError {
    operation: &'static str,
    category: &'static str,
    status: Option<u16>,
}

impl ChannelCacheFetchError {
    fn from_helix(
        operation: &'static str,
        error: twitch_api::helix::ClientRequestError<ChannelCacheRequestError>,
    ) -> Self {
        use twitch_api::helix::{ClientRequestError, HelixRequestGetError};

        let (category, status) = match error {
            ClientRequestError::HelixRequestGetError(
                HelixRequestGetError::DeserializeError(_, _, _, status)
                | HelixRequestGetError::InvalidResponse { status, .. },
            ) => ("response", Some(status.as_u16())),
            ClientRequestError::HelixRequestGetError(_) => ("response", None),
            ClientRequestError::RequestError(error) => (error.category, error.status),
            ClientRequestError::CreateRequestError(_) => ("request_build", None),
            _ => ("response", None),
        };
        Self {
            operation,
            category,
            status,
        }
    }

    fn is_auth_rejection(&self) -> bool {
        self.status == Some(401)
    }

    fn session_unavailable() -> Self {
        Self {
            operation: "poll",
            category: "session_unavailable",
            status: None,
        }
    }
}

impl std::fmt::Display for ChannelCacheFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation={} category={}", self.operation, self.category)?;
        if let Some(status) = self.status {
            write!(f, " status={status}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ChannelCachePollError {
    failure: ChannelCacheFetchError,
    identity: Option<token::TokenIdentity>,
}

#[derive(Debug)]
struct ChannelCachePollUpdate {
    channels: Vec<twitch_api::helix::channels::FollowedBroadcaster>,
    cache: types::ChannelCache,
}

async fn poll_channel_cache(app_handle: &AppHandle) -> Result<(), ChannelCachePollError> {
    let polling_engine =
        app_handle
            .state::<ChatRuntime>()
            .current()
            .map_err(|_| ChannelCachePollError {
                failure: ChannelCacheFetchError::session_unavailable(),
                identity: None,
            })?;
    let snapshot = app_handle
        .state::<TokenManager>()
        .active_snapshot()
        .await
        .ok_or_else(|| ChannelCachePollError {
            failure: ChannelCacheFetchError::session_unavailable(),
            identity: None,
        })?;
    poll_channel_cache_with_token(
        app_handle,
        snapshot.token,
        &snapshot.identity,
        &polling_engine,
    )
    .await
    .map_err(|failure| ChannelCachePollError {
        failure,
        identity: Some(snapshot.identity),
    })
}

async fn poll_channel_cache_with_token(
    app_handle: &AppHandle,
    token: twitch_oauth2::UserToken,
    identity: &token::TokenIdentity,
    polling_engine: &chat::ChatEngine,
) -> Result<(), ChannelCacheFetchError> {
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

    let joined_candidates = desired_channels
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

    debug!("polling channel cache");
    let fetched = fetch_channel_cache(
        client,
        &token,
        channel_cache_settings,
        &previous_cache,
        joined_candidates,
        std::time::Duration::from_secs(
            app_settings(state_syncer.inner())
                .network
                .request_timeout_secs,
        ),
    )
    .await?;
    let token_manager = app_handle.state::<TokenManager>();
    let _session = token_manager
        .guard_session(Some(identity))
        .await
        .ok_or_else(ChannelCacheFetchError::session_unavailable)?;
    complete_channel_cache_poll(
        async { Ok(fetched) },
        |ChannelCachePollUpdate { channels, cache }| {
            let mut state = polling_engine.state.lock().unwrap();
            if !state.active {
                return;
            }
            state.membership.followed = channels
                .iter()
                .map(|channel| {
                    let login = normalize_channel_login(channel.broadcaster_login.as_str());
                    let live = cache
                        .channels
                        .get(&login)
                        .is_some_and(|status| status.is_live);
                    (login, live)
                })
                .collect();
            for channel in &channels {
                let login = normalize_channel_login(channel.broadcaster_login.as_str());
                state.directory.insert(
                    login.clone(),
                    types::ChatRoomInfo {
                        broadcaster_id: channel.broadcaster_id.to_string(),
                        broadcaster_login: login,
                        broadcaster_name: channel.broadcaster_name.to_string(),
                    },
                );
            }
            drop(state);
            polling_engine.wake();
            info!(
                "channel cache updated: {} channels, {} live",
                cache.channels.len(),
                cache.channels.values().filter(|c| c.is_live).count()
            );
            if let Ok(store) = app_handle.store("channel-directory.json") {
                store.set(token.user_id.as_str(), serde_json::json!(cache));
                if let Err(error) = store.save() {
                    warn!("could not save channel directory: {error}");
                }
            }
            app_handle
                .state::<NetworkRuntime>()
                .recovered(app_handle, "channels.refresh");
            state_syncer.update::<types::ChannelCache>("channel_cache", cache, true);
        },
    )
    .await
}

async fn complete_channel_cache_poll(
    fetch: impl std::future::Future<Output = Result<ChannelCachePollUpdate, ChannelCacheFetchError>>,
    publish: impl FnOnce(ChannelCachePollUpdate),
) -> Result<(), ChannelCacheFetchError> {
    let update = fetch.await?;
    publish(update);
    Ok(())
}

async fn fetch_channel_cache<C: twitch_api::HttpClient>(
    client: &HelixClient<'_, C>,
    token: &twitch_oauth2::UserToken,
    settings: types::ChannelCacheSettings,
    previous_cache: &types::ChannelCache,
    joined_candidates: Vec<ChannelCacheCandidate>,
    timeout: std::time::Duration,
) -> Result<ChannelCachePollUpdate, ChannelCacheFetchError> {
    let client = HelixClient::with_client(ChannelCacheTransport {
        inner: client.get_client(),
        timeout,
    });
    let channels: Vec<twitch_api::helix::channels::FollowedBroadcaster> = client
        .get_followed_channels(token.user_id.to_string(), token)
        .try_collect()
        .await
        .map_err(|error| ChannelCacheFetchError::from_helix("followed_channels", error))?;

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
    let candidates = merge_channel_cache_candidates(followed_candidates, joined_candidates);

    let mut users: HashMap<String, twitch_api::helix::users::User> = HashMap::new();
    for chunk in candidates.chunks(settings.user_lookup_chunk_size) {
        let ids: Vec<twitch_api::types::UserId> = chunk
            .iter()
            .map(|candidate| twitch_api::types::UserId::new(candidate.broadcaster_id.clone()))
            .collect();
        let id_collection = twitch_api::types::Collection::from(ids);
        let chunk_users: Vec<twitch_api::helix::users::User> = client
            .get_users_from_ids(&id_collection, token)
            .try_collect()
            .await
            .map_err(|error| ChannelCacheFetchError::from_helix("users", error))?;
        for user in chunk_users {
            users.insert(user.login.to_string(), user);
        }
    }

    let logins: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.login.clone())
        .collect();
    let live_streams = fetch_live_streams_for_logins(&client, &logins, token)
        .await
        .map_err(|error| ChannelCacheFetchError::from_helix("streams", error))?;
    let cache = channel_cache_from_candidates(&candidates, &users, &live_streams, previous_cache);

    Ok(ChannelCachePollUpdate { channels, cache })
}

fn is_auth_failure_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid oauth token")
}

async fn handle_channel_cache_poll_error(
    app_handle: &AppHandle,
    context: &str,
    failure: ChannelCachePollError,
) -> bool {
    let stop = channel_cache_poll_should_stop(
        &failure,
        async {
            let identity = failure
                .identity
                .as_ref()
                .ok_or(token::RecoveryError::SessionChanged)?;
            app_handle
                .state::<TokenManager>()
                .recover_rejected_token(identity)
                .await
                .map(|_| ())
        },
        |failure| async move {
            clear_auth_if_current_async(app_handle, false, Some(&failure.identity)).await;
        },
    )
    .await;
    if stop || failure.failure.is_auth_rejection() {
        return stop;
    }

    let error = failure.failure;
    let retryable = error.status.map_or_else(
        || {
            matches!(
                error.category,
                "timeout" | "connect" | "response_body" | "request"
            )
        },
        |status| status == 408 || status == 429 || (500..600).contains(&status),
    );
    let failure = network::Failure::new(
        error.category,
        format!("Channel list refresh failed ({}).", error.category),
        retryable,
    );
    app_handle.state::<NetworkRuntime>().failed(
        app_handle,
        "channels.refresh",
        "Channel list",
        &failure,
        !app_handle
            .state::<StateSyncer>()
            .snapshot::<types::ChannelCache>("channel_cache")
            .channels
            .is_empty(),
        false,
        Some(
            app_settings(app_handle.state::<StateSyncer>().inner())
                .channel_cache
                .poll_interval(),
        ),
    );
    if !retryable {
        error!("{context} channel cache poll failed: {error}");
    }
    false
}

async fn channel_cache_poll_should_stop<F: std::future::Future<Output = ()>>(
    failure: &ChannelCachePollError,
    recovery: impl std::future::Future<Output = Result<(), token::RecoveryError>>,
    clear_auth: impl FnOnce(token::AuthFailure) -> F,
) -> bool {
    if failure.identity.is_none() {
        return true;
    }
    if failure.failure.is_auth_rejection() {
        match recovery.await {
            Ok(_) => info!("channel cache token recovered; polling will retry"),
            Err(token::RecoveryError::ReauthenticationRequired(failure)) => {
                clear_auth(failure).await;
                return true;
            }
            Err(token::RecoveryError::SessionChanged) => return true,
            Err(error) => warn!(operation = "channel_cache_recovery", "{error}"),
        }
        return false;
    }

    false
}

#[cfg(test)]
async fn run_channel_cache_poll_loop<P, E>(
    settings: impl Fn() -> types::ChannelCacheSettings,
    poll: impl FnMut() -> P,
    handle_error: impl FnMut(&'static str, ChannelCachePollError) -> E,
) where
    P: std::future::Future<Output = Result<(), ChannelCachePollError>>,
    E: std::future::Future<Output = bool>,
{
    run_channel_cache_poll_loop_with_wake(settings, poll, handle_error, None).await;
}

async fn run_channel_cache_poll_loop_with_wake<P, E>(
    settings: impl Fn() -> types::ChannelCacheSettings,
    mut poll: impl FnMut() -> P,
    mut handle_error: impl FnMut(&'static str, ChannelCachePollError) -> E,
    wake: Option<Arc<tokio::sync::Notify>>,
) where
    P: std::future::Future<Output = Result<(), ChannelCachePollError>>,
    E: std::future::Future<Output = bool>,
{
    if let Err(error) = poll().await {
        if handle_error("initial", error).await {
            return;
        }
    }
    loop {
        let manual = if let Some(wake) = &wake {
            tokio::select! { _ = tokio::time::sleep(settings().poll_interval()) => false, _ = wake.notified() => true }
        } else {
            tokio::time::sleep(settings().poll_interval()).await;
            false
        };
        if !manual && !settings().recurring_poll_enabled {
            continue;
        }
        if let Err(error) = poll().await {
            if handle_error("recurring", error).await {
                return;
            }
        }
    }
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

    if clear_stale_authorized_auth_if_needed(&app_handle, &token_manager, &auth_state_snapshot)
        .await
        && quick
    {
        return Err("no valid token".to_string());
    }

    let user_token: types::UserToken = if token_manager.active_snapshot().await.is_some() {
        debug!("using saved token while validation runs in the background");
        token_manager
            .active_public_token()
            .await
            .ok_or_else(|| "no active token".to_owned())?
    } else if quick {
        return Err("no valid token".to_string());
    } else {
        let device_code = token_manager.start_device_code_flow().await?;

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

        token_manager.finish_device_code_flow().await?;
        token_manager
            .active_public_token()
            .await
            .ok_or_else(|| "no active token".to_owned())?
    };

    let directory_store = app_handle.store("channel-directory.json").ok();
    let cached_directory = directory_store
        .and_then(|store| store.get(&user_token.user_id))
        .and_then(|value| serde_json::from_value::<ChannelCache>(value).ok())
        .unwrap_or_else(|| {
            if auth_state_snapshot
                .token
                .as_ref()
                .is_some_and(|token| token.user_id == user_token.user_id)
            {
                state_syncer.snapshot::<ChannelCache>("channel_cache")
            } else {
                ChannelCache::default()
            }
        });
    state_syncer.update::<ChannelCache>("channel_cache", cached_directory.clone(), true);

    // Create empty managers - no network calls, just register state so
    // other Tauri commands can access them immediately.
    let eventsub_manager = EventSubManager::new();
    let badge_manager = BadgeManager::empty(token_manager.clone(), app_handle.clone());
    let emote_manager = EmoteManager::empty(
        client.clone(),
        token_manager.clone(),
        app_handle.clone(),
        user_token.user_id.clone(),
    );

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
    let chat_engine = app_handle
        .state::<ChatRuntime>()
        .activate(&app_handle, app_settings(state_syncer.inner()).chat);
    chat_engine.configure_translation(app_settings(state_syncer.inner()).translation);
    {
        let auth_state_ref = state_syncer.get::<AuthState>("auth_state");
        let mut auth_state = auth_state_ref.lock().unwrap();

        auth_state.phase = types::AuthPhase::Authorized;
        auth_state.device_code = String::new();
        auth_state.token = Some(user_token.clone());
    }

    // Token refresh is handled by the always-on `token_refresh_supervisor`
    // spawned once at startup.

    // --- Background tasks ---

    {
        let mut state = chat_engine.state.lock().unwrap();
        for (login, channel) in &cached_directory.channels {
            state.directory.insert(
                login.clone(),
                types::ChatRoomInfo {
                    broadcaster_id: channel.broadcaster_id.clone(),
                    broadcaster_login: login.clone(),
                    broadcaster_name: channel.display_name.clone(),
                },
            );
        }
    }
    app_handle
        .state::<NetworkRuntime>()
        .start(app_handle.clone());
    {
        let app = app_handle.clone();
        app_handle
            .state::<NetworkRuntime>()
            .enqueue(network::MetadataJob {
                key: "twitch:profile".into(),
                provider: "twitch".into(),
                feature: "Profile picture".into(),
                cached: !user_token.profile_image_url.is_empty(),
                priority: network::metadata_priority(&chat_engine, None),
                run: Arc::new(move || {
                    let app = app.clone();
                    Box::pin(async move {
                        let manager = app.state::<TokenManager>();
                        let token = manager.active_twitch_token().await.ok_or_else(|| {
                            network::Failure::new(
                                "authentication",
                                "Waiting for Twitch authentication.",
                                true,
                            )
                        })?;
                        let client = app.state::<HelixClient<'static, reqwest::Client>>();
                        if let Some(user) = client
                            .get_user_from_id(&token.user_id, &token)
                            .await
                            .map_err(network::Failure::helix)?
                        {
                            manager
                                .update_profile_image(
                                    token.user_id.as_str(),
                                    user.profile_image_url.unwrap_or_default(),
                                )
                                .await
                                .map_err(|_| {
                                    network::Failure::new(
                                        "persistence",
                                        "Could not save the profile picture.",
                                        false,
                                    )
                                })?;
                        }
                        Ok(())
                    })
                }),
            });
    }
    badge_manager
        .schedule(&app_handle, &chat_engine, None)
        .await;
    emote_manager.schedule(&app_handle, &chat_engine, None);

    // EventSub connection + message processing thread
    {
        let eventsub_manager = eventsub_manager.clone();
        let client = client.clone();
        let token_manager = token_manager.clone();
        let app_ref = app_handle.clone();
        let badge_manager_ref = badge_manager.clone();
        let chat_engine_ref = chat_engine.clone();
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
                if !chat_engine_ref.active() {
                    break;
                }
                match msg {
                    eventsub::EventSubMessage::AuthFailed(failure) => {
                        tauri::async_runtime::block_on(clear_auth_if_current_async(
                            &app_ref,
                            true,
                            Some(&failure.identity),
                        ));
                        break;
                    }
                    eventsub::EventSubMessage::Notification(notification) => {
                        if let Some(event) =
                            chat::moderation::ModerationEvent::from_notification(&notification)
                        {
                            chat_engine_ref.moderate(event);
                            continue;
                        }
                        match notification.event {
                            twitch_api::eventsub::Event::ChannelChatMessageV1(P {
                                message: M::Notification(chat_message),
                                ..
                            }) => {
                                let settings = app_settings(&state_syncer_ref);
                                let mut channel_msg = types::ChannelMessage::new(
                                    chat_message.clone(),
                                    notification.ts.to_string(),
                                    badge_manager_ref.clone(),
                                    emote_manager_ref.clone(),
                                    settings.emotes,
                                );
                                if let Some(message_type) = notification.message_type_override {
                                    channel_msg.message_type = message_type;
                                }
                                internal::ingest_chat_message(&chat_engine_ref, channel_msg);
                            }
                            _ => debug!("event notification: {:?}", notification.event),
                        }
                    }
                }
            }
        });
    }

    chat::runtime::start(app_handle.clone(), chat_engine.clone());
    {
        let app = app_handle.clone();
        let engine = Arc::downgrade(&chat_engine);
        let validation = tauri::async_runtime::spawn(async move {
            let mut attempt = 0;
            while engine.upgrade().is_some_and(|engine| engine.active()) {
                let manager = app.state::<TokenManager>();
                let network = app.state::<NetworkRuntime>();
                match manager.validate_startup().await {
                    Ok(()) => {
                        network.recovered(&app, "auth.validate");
                        break;
                    }
                    Err(token::RecoveryError::ReauthenticationRequired(failure)) => {
                        clear_auth_if_current_async(&app, true, Some(&failure.identity)).await;
                        break;
                    }
                    Err(token::RecoveryError::SessionChanged) => break,
                    Err(_) => {
                        attempt += 1;
                        let delay = app_settings(app.state::<StateSyncer>().inner())
                            .network
                            .retry_delay(attempt);
                        network.failed(
                            &app,
                            "auth.validate",
                            "Twitch authentication",
                            &network::Failure::new(
                                "connection",
                                "Waiting to confirm your saved Twitch session.",
                                true,
                            ),
                            true,
                            true,
                            Some(delay),
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        });
        app_handle
            .state::<SharedEventSubHandles>()
            .lock()
            .await
            .push(validation);
    }

    // Channel cache: initial poll + recurring loop
    let poll_app = app_handle.clone();
    let poll_handle = tauri::async_runtime::spawn(async move {
        run_channel_cache_poll_loop_with_wake(
            || app_settings(poll_app.state::<StateSyncer>().inner()).channel_cache,
            || poll_channel_cache(&poll_app),
            |context, error| handle_channel_cache_poll_error(&poll_app, context, error),
            Some(poll_app.state::<NetworkRuntime>().channels_retry.clone()),
        )
        .await;
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
    clear_auth_if_current_async(app_handle, abort_poll, None).await;
}

async fn clear_auth_if_current_async(
    app_handle: &AppHandle,
    abort_poll: bool,
    identity: Option<&token::TokenIdentity>,
) -> bool {
    let token_manager = app_handle.state::<TokenManager>();
    let Some(mut session) = token_manager.guard_session(identity).await else {
        return false;
    };
    app_handle.state::<ChatRuntime>().stop(app_handle);
    app_handle.state::<NetworkRuntime>().reset(app_handle);
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

    if let Err(err) = session.remove_active_token() {
        error!("failed to remove active token while clearing auth: {}", err);
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
    true
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

/// The token manager serializes maintenance with reactive auth recovery.
async fn refresh_token_if_needed(app_handle: &AppHandle, force_validate: bool) {
    let state_syncer = app_handle.state::<StateSyncer>();
    let Some(token_manager) = app_handle.try_state::<TokenManager>() else {
        return;
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
    let failures = token_manager
        .refresh_all_tokens(force_validate, auth_settings.refresh_threshold())
        .await;
    for failure in failures {
        clear_auth_if_current_async(app_handle, true, Some(&failure.identity)).await;
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
fn logout(app_handle: AppHandle) -> Result<(), String> {
    info!("logout");
    clear_auth(&app_handle);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    let nvidia_wayland_workaround_applied = platform::apply_nvidia_wayland_workaround();

    color_eyre::install().expect("failed to install color_eyre");
    let pepo_log_level = logging::pepo_log_level();
    let inbound_build = inbound_build_info();
    let internal_build = internal_build_info(&inbound_build);

    let mut sync_cfg = StateSyncerConfig::default();
    sync_cfg.persist_keys.insert("auth_state".to_owned(), true);
    sync_cfg
        .persist_keys
        .insert("channel_cache".to_owned(), true);
    sync_cfg
        .persist_keys
        .insert(APP_SETTINGS_KEY.to_owned(), true);

    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(9_000_000) // 9 MB per log file
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(10))
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
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_svelte_synced_store::plugin(sync_cfg));

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
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_filter(|label| label == "main")
                .build(),
        );

    let builder = internal::apply_plugins(builder, internal_build);
    let builder = builder.on_page_load(|webview, payload| {
        if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
            if let Some(runtime) = webview.try_state::<ChatRuntime>() {
                if let Ok(engine) = runtime.current() {
                    engine.close_owner(webview.label());
                }
            }
        }
    });

    builder
        .invoke_handler(public_handlers.invoke_handler())
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            if nvidia_wayland_workaround_applied {
                info!("NVIDIA on Wayland detected; disabled EGL explicit sync");
            }

            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .unwrap();

            // This is also required if you want to use events
            public_handlers.mount_events(app);
            internal::setup(app)?;

            let state_syncer = app.state::<StateSyncer>().inner().clone();

            let _auth_state: AuthState = state_syncer.load("auth_state");
            let _channel_cache: ChannelCache = state_syncer.load("channel_cache");
            let settings: AppSettings = state_syncer.load(APP_SETTINGS_KEY);
            let settings = settings.normalized();
            state_syncer.set(APP_SETTINGS_KEY, settings.clone());
            app.manage(NetworkRuntime::default());
            app.manage(UserCardRuntime::default());
            let catalog_app = app.handle().clone();
            let gate: synth_translate::CatalogRequestGate = Arc::new(move || {
                let app = catalog_app.clone();
                Box::pin(async move {
                    let network = app.state::<NetworkRuntime>().inner().clone();
                    network.request_guard(&app, "translation-catalog").await
                })
            });
            app.handle()
                .plugin(synth_translate::plugin::init_with_catalog_network(
                    Default::default(),
                    std::time::Duration::from_secs(settings.network.connect_timeout_secs),
                    std::time::Duration::from_secs(settings.network.request_timeout_secs),
                    Some(gate),
                ))?;
            let package_info = app.package_info();
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(format!("{} {}", package_info.name, package_info.version))
                .visible(false);

            let window = window_chrome::configure(win_builder).build().unwrap();
            window_chrome::apply(&window);

            // Revalidate/refresh the token whenever the window regains focus -
            // covers returning to the app after the machine has slept.
            {
                let focus_app = app.handle().clone();
                let owner = window.label().to_owned();
                window.on_window_event(move |event| {
                    if matches!(
                        event,
                        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
                    ) {
                        focus_app.state::<UserCardRuntime>().close_all(&focus_app);
                    }
                    if matches!(event, WindowEvent::Destroyed) {
                        focus_app.state::<NetworkRuntime>().release_owner(&owner);
                        if let Ok(engine) = focus_app.state::<ChatRuntime>().current() {
                            engine.close_owner(&owner);
                        }
                    }
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

            let client: HelixClient<'static, reqwest::Client> = HelixClient::with_client(
                reqwest::Client::builder()
                    .user_agent(concat!("pepo/", env!("CARGO_PKG_VERSION")))
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(std::time::Duration::from_secs(
                        settings.network.connect_timeout_secs,
                    ))
                    .timeout(std::time::Duration::from_secs(
                        settings.network.request_timeout_secs,
                    ))
                    .build()
                    .expect("valid Twitch HTTP client"),
            );

            info!("made client");

            app.manage::<SharedPollHandle>(Mutex::new(None));
            app.manage(ChatRuntime::default());
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

    #[cfg(debug_assertions)]
    #[test]
    fn specta_bindings_export_channel_sync_commands_without_internal_state() {
        let path =
            std::env::temp_dir().join(format!("pepo-specta-bindings-{}.ts", std::process::id()));
        export_bindings(&specta_builder(), &path);
        let generated = std::fs::read_to_string(&path).expect("generated bindings readable");
        let _ = std::fs::remove_file(&path);

        assert!(generated.contains("\"subscribe_state\""));
        assert!(generated.contains("\"unsubscribe_state\""));
        assert!(generated.contains("\"update_state\""));
        assert!(!generated.contains("InternalState"));
    }

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
    fn channel_cache_tracks_partner_status_and_preserves_it_without_user_metadata() {
        let candidate =
            ChannelCacheCandidate::new("1".to_owned(), "maya".to_owned(), "Maya".to_owned());
        let mut cache = types::ChannelCache::default();
        for (broadcaster_type, expected) in [("partner", true), ("affiliate", false), ("", false)] {
            let user: twitch_api::helix::users::User = serde_json::from_value(serde_json::json!({
                "id": "1", "login": "maya", "display_name": "Maya", "type": "",
                "broadcaster_type": broadcaster_type, "description": "", "profile_image_url": "",
                "offline_image_url": "", "view_count": 0, "created_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap();
            upsert_channel_cache_candidate(&mut cache, &candidate, Some(&user), None, None);
            assert_eq!(cache.channels["maya"].is_partner, expected);
            let previous = cache.channels["maya"].clone();
            upsert_channel_cache_candidate(&mut cache, &candidate, None, None, Some(&previous));
            assert_eq!(cache.channels["maya"].is_partner, expected);
        }
    }

    #[test]
    fn channel_cache_refresh_clears_previous_partner_status_for_a_non_partner() {
        let candidate =
            ChannelCacheCandidate::new("1".to_owned(), "maya".to_owned(), "Maya".to_owned());
        let previous = types::ChannelStatus {
            is_partner: true,
            ..Default::default()
        };
        let user: twitch_api::helix::users::User = serde_json::from_value(serde_json::json!({
            "id": "1", "login": "maya", "display_name": "Maya", "type": "",
            "broadcaster_type": "affiliate", "description": "", "profile_image_url": "",
            "offline_image_url": "", "view_count": 0, "created_at": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        let mut cache = types::ChannelCache::default();
        upsert_channel_cache_candidate(&mut cache, &candidate, Some(&user), None, Some(&previous));
        assert!(!cache.channels["maya"].is_partner);
    }

    #[test]
    fn channel_cache_without_partner_field_remains_readable() {
        let cache: types::ChannelCache = serde_json::from_value(serde_json::json!({
            "channels": { "maya": { "broadcaster_id": "1", "login": "maya", "display_name": "Maya",
                "profile_image_url": "", "is_live": false, "stream": null } }
        }))
        .unwrap();
        assert!(!cache.channels["maya"].is_partner);
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

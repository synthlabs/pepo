use color_eyre::Report;
use eyre::eyre;
use eyre::WrapErr;
#[cfg(debug_assertions)]
use futures::stream;
use futures::StreamExt;
use futures::TryStreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info, trace, warn};
use twitch_api::types::UserId;
use twitch_api::{
    eventsub::{self, Event, EventsubWebsocketData, ReconnectPayload, SessionData, WelcomePayload},
    HelixClient,
};
use twitch_oauth2::UserToken;

use crate::{
    logging,
    token::{AuthFailure, RecoveryError, TokenIdentity, TokenManager},
    types::EventSubSettings,
};

type SharedMap<V> = Arc<Mutex<HashMap<String, Mutex<HashSet<V>>>>>;
type DesiredChannels = Arc<Mutex<HashMap<String, UserId>>>;
type EventSubSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
pub type EventSubSettingsReader = Arc<dyn Fn() -> EventSubSettings + Send + Sync>;

#[derive(Debug)]
pub struct EventNotification {
    pub ts: twitch_api::types::Timestamp,
    pub event: Event,
}

#[derive(Debug)]
pub enum EventSubMessage {
    Notification(EventNotification),
    AuthFailed(AuthFailure),
}

pub struct EventSubRuntime {
    pub events: Receiver<EventSubMessage>,
    pub handles: Vec<tauri::async_runtime::JoinHandle<()>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EventSubSubscription {
    pub channel_name: String,
    pub id: twitch_api::types::EventSubId,
    pub sub_type: eventsub::EventType,
}

// Implement hashing using ALL fields
impl Hash for EventSubSubscription {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.channel_name.hash(state);
        self.id.hash(state);
        self.sub_type.to_string().hash(state); // Convert enum to string
    }
}

#[derive(Clone)]
pub struct EventSubManager {
    session_id: Arc<Mutex<Option<String>>>,
    subscriptions: SharedMap<EventSubSubscription>,
    desired_channels: DesiredChannels,
    user_update_subscription_id: Arc<Mutex<Option<twitch_api::types::EventSubId>>>,
    auth_session: Arc<Mutex<Option<TokenIdentity>>>,
    revoked_rooms: Arc<Mutex<HashMap<String, String>>>,
    uncertain_rooms: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    Joined,
    Pending,
}

pub fn subscription_error_status<C: twitch_api::HttpClient>(error: &Report) -> Option<u16> {
    use twitch_api::helix::{
        ClientRequestError, HelixRequestDeleteError, HelixRequestGetError, HelixRequestPostError,
    };
    match error.downcast_ref::<ClientRequestError<C::Error>>() {
        Some(ClientRequestError::HelixRequestPostError(HelixRequestPostError::Error {
            status,
            ..
        })) => Some(status.as_u16()),
        Some(ClientRequestError::HelixRequestPostError(
            HelixRequestPostError::InvalidResponse { status, .. },
        )) => Some(status.as_u16()),
        Some(ClientRequestError::HelixRequestDeleteError(HelixRequestDeleteError::Error {
            status,
            ..
        })) => Some(status.as_u16()),
        Some(ClientRequestError::HelixRequestDeleteError(
            HelixRequestDeleteError::InvalidResponse { status, .. },
        )) => Some(status.as_u16()),
        Some(ClientRequestError::HelixRequestGetError(HelixRequestGetError::Error {
            status,
            ..
        })) => Some(status.as_u16()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionMode {
    Fresh,
    ReconnectHandoff,
}

#[derive(Debug, PartialEq, Eq)]
enum SocketAction {
    Continue,
    Connected,
    Reconnect(String),
    FreshReconnect,
    AuthFailed(AuthFailure),
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
enum RevocationAction {
    Account(TokenIdentity),
    Room,
    Reconnect,
}

#[derive(Debug)]
enum SocketRead {
    Message(tungstenite::Message),
    Closed,
    IdleTimeout,
    ReceiveError(tungstenite::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventSubFailure {
    AuthFailed,
    DuplicateSubscription,
    InvalidReconnect,
    StaleSession,
    StaleSubscription,
    Recoverable,
}

/// Connect to the websocket and return the stream
async fn connect(connect_url: String) -> Result<EventSubSocket, Report> {
    info!(url = connect_url, "connecting to twitch");
    let config = tungstenite::protocol::WebSocketConfig::default();
    let (socket, _) =
        tokio_tungstenite::connect_async_with_config(connect_url, Some(config), false)
            .await
            .wrap_err("Can't connect")?;

    Ok(socket)
}

fn twitch_eventsub_url() -> String {
    twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL
        .as_str()
        .to_owned()
}

async fn next_socket_read(socket: &mut EventSubSocket, settings: EventSubSettings) -> SocketRead {
    match tokio::time::timeout(settings.socket_idle_timeout(), socket.next()).await {
        Ok(Some(Ok(msg))) => SocketRead::Message(msg),
        Ok(Some(Err(err))) => SocketRead::ReceiveError(err),
        Ok(None) => SocketRead::Closed,
        Err(_) => SocketRead::IdleTimeout,
    }
}

fn classify_error_text(error: &str) -> EventSubFailure {
    let lower = error.to_lowercase();

    if lower.contains("auth_expired")
        || lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("invalid oauth token")
    {
        EventSubFailure::AuthFailed
    } else if lower.contains("subscription already exists")
        || (lower.contains("409") && lower.contains("conflict"))
    {
        EventSubFailure::DuplicateSubscription
    } else if lower.contains("invalid reconnect") || lower.contains("code=4007") {
        EventSubFailure::InvalidReconnect
    } else if lower.contains("websocket transport session does not exist")
        || lower.contains("has already disconnected")
        || lower.contains("code=4004")
    {
        EventSubFailure::StaleSession
    } else if lower.contains("404") && lower.contains("not found") {
        EventSubFailure::StaleSubscription
    } else {
        EventSubFailure::Recoverable
    }
}

fn account_revoked(status: &str, condition_user: Option<&str>, active_user: &str) -> bool {
    (status == "authorization_revoked" || status == "user_removed")
        && condition_user == Some(active_user)
}

fn retry_delay(attempt: u32, settings: EventSubSettings) -> Duration {
    let secs = retry_delay_secs(attempt, settings);
    let jitter_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.subsec_millis() % 1_000) as u64)
        .unwrap_or(0);

    Duration::from_secs(secs) + Duration::from_millis(jitter_ms)
}

fn retry_delay_secs(attempt: u32, settings: EventSubSettings) -> u64 {
    settings.retry_delay_secs(attempt)
}

fn warn_or_warn_repeated(
    settings: EventSubSettings,
    key: String,
    message: String,
    throttle: Duration,
) {
    if settings.repeated_log_throttle_enabled {
        logging::warn_repeated(key, message, throttle);
    } else {
        warn!("{message}");
    }
}

fn error_or_error_repeated(
    settings: EventSubSettings,
    key: String,
    message: String,
    throttle: Duration,
) {
    if settings.repeated_log_throttle_enabled {
        logging::error_repeated(key, message, throttle);
    } else {
        error!("{message}");
    }
}

impl EventSubManager {
    pub fn new() -> EventSubManager {
        EventSubManager {
            session_id: Arc::new(Mutex::new(None)),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            desired_channels: Arc::new(Mutex::new(HashMap::new())),
            user_update_subscription_id: Arc::new(Mutex::new(None)),
            auth_session: Arc::new(Mutex::new(None)),
            revoked_rooms: Arc::new(Mutex::new(HashMap::new())),
            uncertain_rooms: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn session_id(&self) -> Option<String> {
        self.session_id.lock().unwrap().clone()
    }

    fn set_session_id(&self, session_id: String) {
        *self.session_id.lock().unwrap() = Some(session_id);
    }

    fn clear_active_session(&self) {
        *self.session_id.lock().unwrap() = None;
        self.clear_active_subscriptions();
    }

    fn clear_active_subscriptions(&self) {
        self.subscriptions.lock().unwrap().clear();
        self.uncertain_rooms.lock().unwrap().clear();
        *self.user_update_subscription_id.lock().unwrap() = None;
    }

    fn add_desired_channel(&self, channel_name: String, chat_id: UserId) {
        self.desired_channels
            .lock()
            .unwrap()
            .insert(channel_name, chat_id);
    }

    fn remove_desired_channel(&self, channel_name: &str) {
        self.desired_channels.lock().unwrap().remove(channel_name);
    }

    pub(crate) fn desired_channels_snapshot(&self) -> Vec<(String, UserId)> {
        self.desired_channels
            .lock()
            .unwrap()
            .iter()
            .map(|(name, id)| (name.clone(), id.clone()))
            .collect()
    }

    fn add_subscription(&self, channel_name: String, sub: EventSubSubscription) {
        let mut guard = self.subscriptions.lock().unwrap();

        let mut subs = guard.entry(channel_name).or_default().lock().unwrap();
        subs.insert(sub);
    }

    fn remove_subscriptions(&self, channel_name: &str) {
        let mut guard = self.subscriptions.lock().unwrap();

        guard.remove(channel_name);
    }

    pub fn is_connected(&self) -> bool {
        self.session_id().is_some()
    }
    pub fn channel_complete(&self, login: &str) -> bool {
        let subs = self.get_subscriptions(login);
        subs.iter()
            .any(|s| s.sub_type == eventsub::EventType::ChannelChatMessage)
            && subs
                .iter()
                .any(|s| s.sub_type == eventsub::EventType::ChannelChatNotification)
    }
    pub fn joined_channels(&self) -> Vec<String> {
        self.desired_channels_snapshot()
            .into_iter()
            .filter_map(|(login, _)| self.channel_complete(&login).then_some(login))
            .collect()
    }
    pub fn take_room_failures(&self) -> HashMap<String, String> {
        std::mem::take(&mut *self.revoked_rooms.lock().unwrap())
    }

    fn process_revocation(
        &self,
        subscription: &serde_json::Value,
    ) -> Result<RevocationAction, Report> {
        let session = self
            .auth_session
            .lock()
            .unwrap()
            .clone()
            .ok_or(RecoveryError::SessionChanged)?;
        let status = subscription["status"].as_str().unwrap_or("unknown");
        let condition = &subscription["condition"];
        if account_revoked(status, condition["user_id"].as_str(), &session.user_id) {
            return Ok(RevocationAction::Account(session));
        }

        let broadcaster = condition["broadcaster_user_id"].as_str();
        let login = self
            .desired_channels_snapshot()
            .into_iter()
            .find(|(_, id)| Some(id.as_str()) == broadcaster)
            .map(|(login, _)| login);
        if let Some(login) = login {
            self.revoked_rooms
                .lock()
                .unwrap()
                .insert(login.clone(), status.to_owned());
            warn!(channel = login, status, "channel subscription revoked");
            Ok(RevocationAction::Room)
        } else {
            warn!(
                status,
                "non-chat EventSub subscription revoked; reconnecting"
            );
            Ok(RevocationAction::Reconnect)
        }
    }

    pub fn channels_with_subscriptions(&self) -> HashSet<String> {
        let mut channels: HashSet<_> = self
            .subscriptions
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(login, subs)| (!subs.lock().unwrap().is_empty()).then(|| login.clone()))
            .collect();
        channels.extend(self.uncertain_rooms.lock().unwrap().iter().cloned());
        channels
    }
    fn has_subscription(&self, login: &str) -> bool {
        !self.get_subscriptions(login).is_empty()
    }

    fn get_subscriptions(&self, channel_name: &str) -> Vec<EventSubSubscription> {
        let guard = self.subscriptions.lock().unwrap();
        guard
            .get(channel_name)
            .map(|subs| subs.lock().unwrap().iter().cloned().collect())
            .unwrap_or_default()
    }

    pub async fn join_chat<C: twitch_api::HttpClient>(
        self,
        chat_id: UserId,
        chat_name: String,
        client: &HelixClient<'_, C>,
        token: UserToken,
    ) -> Result<JoinOutcome, Report> {
        self.add_desired_channel(chat_name.clone(), chat_id.clone());

        let Some(session_id) = self.session_id() else {
            debug!(
                "EventSubManager - queued channel until websocket is connected: chat={}",
                chat_name
            );
            return Ok(JoinOutcome::Pending);
        };

        if self.channel_complete(&chat_name) {
            debug!(
                "EventSubManager - channel already subbed to: chat={}",
                chat_name,
            );
            return Ok(JoinOutcome::Joined);
        }

        match self
            .create_channel_subscriptions(chat_id, chat_name.clone(), &session_id, client, &token)
            .await
        {
            Ok(_) => Ok(JoinOutcome::Joined),
            Err(err) => {
                let err_msg = format!("{:?}", err);
                if classify_error_text(&err_msg) == EventSubFailure::StaleSession {
                    warn!(
                        "EventSubManager - session is stale while joining {}; queued for reconnect",
                        chat_name
                    );
                    self.clear_active_session();
                    return Ok(JoinOutcome::Pending);
                }
                if classify_error_text(&err_msg) != EventSubFailure::AuthFailed
                    && subscription_error_status::<C>(&err)
                        .is_none_or(|status| !(400..500).contains(&status))
                {
                    self.uncertain_rooms.lock().unwrap().insert(chat_name);
                }
                Err(err)
            }
        }
    }

    async fn create_channel_subscriptions<C: twitch_api::HttpClient>(
        &self,
        chat_id: UserId,
        chat_name: String,
        session_id: &str,
        client: &HelixClient<'_, C>,
        token: &UserToken,
    ) -> Result<(), Report> {
        let transport = eventsub::Transport::websocket(session_id);
        debug!(
            "EventSubManager - creating ChannelChatMessageV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let user_id = token.clone().user_id;
        let message =
            eventsub::channel::chat::ChannelChatMessageV1::new(chat_id.clone(), user_id.clone());
        if !self
            .get_subscriptions(&chat_name)
            .iter()
            .any(|s| s.sub_type == eventsub::EventType::ChannelChatMessage)
        {
            match client
                .create_eventsub_subscription(message.clone(), transport.clone(), token)
                .await
            {
                Ok(resp) => {
                    if self.session_id().as_deref() != Some(session_id) {
                        return Err(eyre!("websocket transport session does not exist"));
                    }
                    self.add_subscription(
                        chat_name.clone(),
                        EventSubSubscription {
                            channel_name: chat_name.clone(),
                            id: resp.id.clone(),
                            sub_type: resp.type_.clone(),
                        },
                    );
                }
                Err(err) => {
                    if classify_error_text(&err.to_string())
                        == EventSubFailure::DuplicateSubscription
                    {
                        self.recover_channel_subscriptions(
                            &chat_name, &chat_id, session_id, client, token,
                        )
                        .await?;
                    } else {
                        self.handle_create_subscription_error(err)?;
                    }
                }
            }
        }

        if !self
            .get_subscriptions(&chat_name)
            .iter()
            .any(|s| s.sub_type == eventsub::EventType::ChannelChatMessage)
        {
            return Err(eyre!("chat message subscription receipt is missing"));
        }
        debug!(
            "EventSubManager - creating ChannelChatNotificationV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let condition = eventsub::channel::chat::ChannelChatNotificationV1::new(
            chat_id.clone(),
            user_id.clone(),
        );
        if !self
            .get_subscriptions(&chat_name)
            .iter()
            .any(|s| s.sub_type == eventsub::EventType::ChannelChatNotification)
        {
            match client
                .create_eventsub_subscription(condition.clone(), transport.clone(), &token.clone())
                .await
            {
                Ok(resp) => {
                    if self.session_id().as_deref() != Some(session_id) {
                        return Err(eyre!("websocket transport session does not exist"));
                    }
                    self.add_subscription(
                        chat_name.clone(),
                        EventSubSubscription {
                            channel_name: chat_name.clone(),
                            id: resp.id.clone(),
                            sub_type: resp.type_.clone(),
                        },
                    );
                }
                Err(err) => {
                    if classify_error_text(&err.to_string())
                        == EventSubFailure::DuplicateSubscription
                    {
                        self.recover_channel_subscriptions(
                            &chat_name, &chat_id, session_id, client, token,
                        )
                        .await?;
                    } else {
                        self.handle_create_subscription_error(err)?;
                    }
                }
            }
        }

        if !self.channel_complete(&chat_name) {
            return Err(eyre!("channel subscription receipts are incomplete"));
        }
        Ok(())
    }

    async fn recover_channel_subscriptions<C: twitch_api::HttpClient>(
        &self,
        login: &str,
        broadcaster: &UserId,
        session: &str,
        client: &HelixClient<'_, C>,
        token: &UserToken,
    ) -> Result<(), Report> {
        let pages: Vec<_> = client
            .get_eventsub_subscriptions(None, None, None, token)
            .try_collect()
            .await?;
        if self.session_id().as_deref() != Some(session) {
            return Err(eyre!("websocket transport session does not exist"));
        }
        for page in pages {
            for sub in page.subscriptions {
                let transport = serde_json::to_value(&sub.transport)?;
                if sub.status == eventsub::Status::Enabled
                    && sub.condition["broadcaster_user_id"].as_str() == Some(broadcaster.as_str())
                    && sub.condition["user_id"].as_str() == Some(token.user_id.as_str())
                    && transport["session_id"].as_str() == Some(session)
                    && matches!(
                        sub.type_,
                        eventsub::EventType::ChannelChatMessage
                            | eventsub::EventType::ChannelChatNotification
                    )
                {
                    self.add_subscription(
                        login.to_owned(),
                        EventSubSubscription {
                            channel_name: login.to_owned(),
                            id: sub.id,
                            sub_type: sub.type_,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn handle_create_subscription_error<E>(&self, err: E) -> Result<(), Report>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let err_str = err.to_string();
        match classify_error_text(&err_str) {
            EventSubFailure::DuplicateSubscription => {
                warn!("EventSub subscription already exists; keeping socket alive: {err_str}");
                Ok(())
            }
            EventSubFailure::AuthFailed => Err(eyre!("AUTH_EXPIRED: {}", err_str)),
            _ => Err(eyre!(err)),
        }
    }

    pub async fn leave_chat<C: twitch_api::HttpClient>(
        self,
        chat_name: String,
        client: &HelixClient<'_, C>,
        token: UserToken,
    ) -> Result<(), Report> {
        let Some(session_id) = self.session_id() else {
            self.remove_subscriptions(&chat_name);
            self.uncertain_rooms.lock().unwrap().remove(&chat_name);
            self.remove_desired_channel(&chat_name);
            return Ok(());
        };

        let uncertain = self.uncertain_rooms.lock().unwrap().contains(&chat_name);
        if uncertain {
            let chat_id = self
                .desired_channels
                .lock()
                .unwrap()
                .get(&chat_name)
                .cloned()
                .ok_or_else(|| eyre!("missing channel identity for uncertain subscription"))?;
            self.recover_channel_subscriptions(&chat_name, &chat_id, &session_id, client, &token)
                .await?;
            self.uncertain_rooms.lock().unwrap().remove(&chat_name);
        }

        if !self.has_subscription(&chat_name) {
            debug!("EventSubManager - sub doesn't exist: chat={}", chat_name);
            self.remove_desired_channel(&chat_name);
            return Ok(());
        }

        debug!("EventSubManager - deleting subs: chat={}", chat_name);

        let subs = self.get_subscriptions(&chat_name);

        for s in subs.clone() {
            match client
                .delete_eventsub_subscription(s.id.clone(), &token.clone())
                .await
            {
                Ok(_) => {}
                Err(err) => {
                    let err_str = err.to_string();
                    match classify_error_text(&err_str) {
                        EventSubFailure::StaleSubscription => {
                            warn!("EventSub subscription was already gone: {err_str}");
                        }
                        EventSubFailure::StaleSession => {
                            warn!("EventSub session went stale while leaving {chat_name}");
                            self.clear_active_session();
                            self.remove_desired_channel(&chat_name);
                            return Ok(());
                        }
                        EventSubFailure::AuthFailed => {
                            return Err(eyre!("AUTH_EXPIRED: {}", err_str));
                        }
                        _ => return Err(eyre!(err)),
                    }
                }
            }
            if let Some(subscriptions) = self.subscriptions.lock().unwrap().get(&chat_name) {
                subscriptions.lock().unwrap().remove(&s);
            }
        }
        self.remove_subscriptions(&chat_name);
        self.remove_desired_channel(&chat_name);

        Ok(())
    }

    pub fn start(
        self,
        client: HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        settings_reader: EventSubSettingsReader,
    ) -> Result<EventSubRuntime, Report> {
        let (std_tx, std_rx) = std::sync::mpsc::sync_channel::<EventSubMessage>(32);
        let mut handles = Vec::new();

        #[cfg(debug_assertions)]
        {
            let token_manager = token_manager.clone();
            let client = client.clone();
            let settings_reader = settings_reader.clone();
            let cost_watcher_handle = tauri::async_runtime::spawn(async move {
                loop {
                    let settings = settings_reader();
                    if !settings.debug_cost_watcher_enabled {
                        sleep(settings.debug_cost_watcher_interval()).await;
                        continue;
                    }

                    let Some(current_token) = token_manager.active_twitch_token().await else {
                        sleep(settings.debug_cost_watcher_interval()).await;
                        continue;
                    };
                    let chatters: Vec<twitch_api::eventsub::EventSubSubscription> = match client
                        .get_eventsub_subscriptions(None, None, None, &current_token)
                        .map_ok(|sub| {
                            stream::iter(
                                sub.subscriptions
                                    .into_iter()
                                    .map(Ok::<_, twitch_api::helix::ClientRequestError<_>>),
                            )
                        })
                        .try_flatten()
                        .try_collect()
                        .await
                    {
                        Ok(resp) => resp,
                        Err(err) => {
                            error!(
                                "failed to get eventsub subscriptions: err={}",
                                err.to_string()
                            );
                            Default::default()
                        }
                    };

                    for sub in chatters.iter() {
                        debug!(
                            "EventSubSubscription: id={}, cost={}, condition={:?}, status={:?}",
                            sub.id, sub.cost, sub.condition, sub.status
                        )
                    }
                    debug!(
                        "EventSubManger::cost_watcher - tick={}s",
                        settings.debug_cost_watcher_interval_secs
                    );
                    sleep(settings.debug_cost_watcher_interval()).await;
                }
            });
            handles.push(cost_watcher_handle);
        }

        let websocket_handle = tauri::async_runtime::spawn(async move {
            let mut retry_attempt = 0;
            let settings_reader = settings_reader.clone();
            let Some(session) = token_manager.active_snapshot().await else {
                return;
            };
            *self.auth_session.lock().unwrap() = Some(session.identity.clone());

            loop {
                match token_manager
                    .ready_token_for_session(&session.identity)
                    .await
                {
                    Ok(_) => {}
                    Err(RecoveryError::SessionChanged) => return,
                    Err(RecoveryError::ReauthenticationRequired(failure)) => {
                        let _ = std_tx.send(EventSubMessage::AuthFailed(failure));
                        return;
                    }
                    Err(error) => {
                        let delay = retry_delay(retry_attempt, settings_reader());
                        retry_attempt = retry_attempt.saturating_add(1);
                        warn!(
                            operation = "eventsub_preflight",
                            "{error}; retrying in {delay:?}"
                        );
                        sleep(delay).await;
                        continue;
                    }
                }
                debug!("connecting to websocket mode=fresh");

                let mut s = match connect(twitch_eventsub_url()).await {
                    Ok(s) => s,
                    Err(e) => {
                        let delay = retry_delay(retry_attempt, settings_reader());
                        retry_attempt = retry_attempt.saturating_add(1);
                        error!("eventsub connect failed, retrying in {:?}: {:?}", delay, e);
                        sleep(delay).await;
                        continue;
                    }
                };
                loop {
                    let settings = settings_reader();
                    match self
                        .clone()
                        .process_socket_read(
                            next_socket_read(&mut s, settings).await,
                            std_tx.clone(),
                            &client,
                            token_manager.clone(),
                            ConnectionMode::Fresh,
                            settings,
                        )
                        .await
                    {
                        SocketAction::Continue => {}
                        SocketAction::Connected => retry_attempt = 0,
                        SocketAction::Stop => return,
                        SocketAction::Reconnect(reconnect_url) => {
                            match self
                                .clone()
                                .complete_reconnect_handoff(
                                    reconnect_url,
                                    &mut s,
                                    std_tx.clone(),
                                    &client,
                                    token_manager.clone(),
                                    settings_reader.clone(),
                                )
                                .await
                            {
                                Ok(new_socket) => {
                                    s = new_socket;
                                    retry_attempt = 0;
                                }
                                Err(err) => {
                                    let err_msg = format!("{:?}", err);
                                    error!("eventsub reconnect handoff failed - {}", err_msg);
                                    match self.action_for_report(&err) {
                                        SocketAction::AuthFailed(failure) => {
                                            let _ =
                                                std_tx.send(EventSubMessage::AuthFailed(failure));
                                            return;
                                        }
                                        SocketAction::Stop => return,
                                        _ => {}
                                    }
                                    self.clear_active_session();
                                    break;
                                }
                            }
                        }
                        SocketAction::FreshReconnect => {
                            self.clear_active_session();
                            break;
                        }
                        SocketAction::AuthFailed(reason) => {
                            warn!("auth failure detected, stopping EventSub reconnect loop");
                            let _ = std_tx.send(EventSubMessage::AuthFailed(reason));
                            return;
                        }
                    }
                }

                let delay = retry_delay(retry_attempt, settings_reader());
                retry_attempt = retry_attempt.saturating_add(1);
                debug!("EventSubManger::run - retrying in {:?}", delay);
                sleep(delay).await;
            }
        });
        handles.push(websocket_handle);

        Ok(EventSubRuntime {
            events: std_rx,
            handles,
        })
    }

    async fn process_socket_read(
        self,
        read: SocketRead,
        ts: SyncSender<EventSubMessage>,
        client: &HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        mode: ConnectionMode,
        eventsub_settings: EventSubSettings,
    ) -> SocketAction {
        match read {
            SocketRead::Message(msg) => {
                trace!("message received: {:?}", msg);
                match self
                    .clone()
                    .process_message(msg, ts, client, token_manager, mode, eventsub_settings)
                    .await
                {
                    Ok(action) => action,
                    Err(e) => {
                        let err_msg = format!("{:?}", e);
                        error!("process_message - {}", err_msg);
                        self.action_for_report(&e)
                    }
                }
            }
            SocketRead::Closed => {
                warn!("eventsub stream closed, reconnecting");
                SocketAction::FreshReconnect
            }
            SocketRead::IdleTimeout => {
                warn!(
                    idle_timeout_secs = eventsub_settings.socket_idle_timeout_secs,
                    "eventsub idle timeout (missed keepalives), reconnecting"
                );
                SocketAction::FreshReconnect
            }
            SocketRead::ReceiveError(err) => {
                let err_msg = format!("{:?}", err);
                match err {
                    tungstenite::Error::Protocol(
                        tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                    ) => error!("eventsub connection reset, reconnecting"),
                    _ => error!("eventsub receive error, reconnecting: {:?}", err),
                }
                self.action_for_failure(classify_error_text(&err_msg), err_msg)
            }
        }
    }

    fn action_for_failure(&self, failure: EventSubFailure, err_msg: String) -> SocketAction {
        match failure {
            EventSubFailure::AuthFailed => SocketAction::FreshReconnect,
            EventSubFailure::DuplicateSubscription => {
                warn!("ignoring duplicate EventSub subscription error: {err_msg}");
                SocketAction::Continue
            }
            EventSubFailure::InvalidReconnect | EventSubFailure::StaleSession => {
                warn!("EventSub session is stale; falling back to a fresh websocket: {err_msg}");
                self.clear_active_session();
                SocketAction::FreshReconnect
            }
            EventSubFailure::StaleSubscription => {
                warn!("ignoring stale EventSub subscription error: {err_msg}");
                SocketAction::Continue
            }
            EventSubFailure::Recoverable => SocketAction::FreshReconnect,
        }
    }

    fn action_for_report(&self, error: &Report) -> SocketAction {
        match error.downcast_ref::<RecoveryError>() {
            Some(RecoveryError::ReauthenticationRequired(failure)) => {
                SocketAction::AuthFailed(failure.clone())
            }
            Some(RecoveryError::SessionChanged) => SocketAction::Stop,
            Some(RecoveryError::RetryLater(_)) => SocketAction::FreshReconnect,
            None => {
                let message = format!("{error:?}");
                self.action_for_failure(classify_error_text(&message), message)
            }
        }
    }

    async fn complete_reconnect_handoff(
        self,
        reconnect_url: String,
        old_socket: &mut EventSubSocket,
        ts: SyncSender<EventSubMessage>,
        client: &HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        settings_reader: EventSubSettingsReader,
    ) -> Result<EventSubSocket, Report> {
        debug!("connecting to websocket mode=reconnect_handoff");
        let mut new_socket = connect(reconnect_url).await?;

        loop {
            tokio::select! {
                old_read = next_socket_read(old_socket, settings_reader()) => {
                    let settings = settings_reader();
                    self
                        .clone()
                        .process_old_socket_during_handoff(
                            old_read,
                            ts.clone(),
                            client,
                            token_manager.clone(),
                            settings,
                        )
                        .await?;
                }
                new_read = next_socket_read(&mut new_socket, settings_reader()) => {
                    let settings = settings_reader();
                    match self.clone().process_socket_read(
                        new_read,
                        ts.clone(),
                        client,
                        token_manager.clone(),
                        ConnectionMode::ReconnectHandoff,
                        settings,
                    ).await {
                        SocketAction::Connected => return Ok(new_socket),
                        SocketAction::Continue => {}
                        SocketAction::Reconnect(url) => {
                            return Err(eyre!("nested reconnect before welcome: {}", url));
                        }
                        SocketAction::FreshReconnect => {
                            return Err(eyre!("reconnect handoff socket failed before welcome"));
                        }
                        SocketAction::AuthFailed(failure) => {
                            return Err(eyre!(RecoveryError::ReauthenticationRequired(failure)));
                        }
                        SocketAction::Stop => return Err(eyre!(RecoveryError::SessionChanged)),
                    }
                }
            }
        }
    }

    async fn process_old_socket_during_handoff(
        self,
        read: SocketRead,
        ts: SyncSender<EventSubMessage>,
        client: &HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        eventsub_settings: EventSubSettings,
    ) -> Result<(), Report> {
        match read {
            SocketRead::Message(msg) => {
                match self
                    .process_message(
                        msg,
                        ts,
                        client,
                        token_manager,
                        ConnectionMode::ReconnectHandoff,
                        eventsub_settings,
                    )
                    .await
                {
                    Ok(SocketAction::Continue | SocketAction::Connected) => Ok(()),
                    Ok(SocketAction::Reconnect(url)) => {
                        warn!("ignoring nested EventSub reconnect while handoff is active: {url}");
                        Ok(())
                    }
                    Ok(SocketAction::FreshReconnect) => {
                        warn!("old EventSub socket asked for a fresh reconnect during handoff");
                        Ok(())
                    }
                    Ok(SocketAction::AuthFailed(failure)) => {
                        Err(eyre!(RecoveryError::ReauthenticationRequired(failure)))
                    }
                    Ok(SocketAction::Stop) => Err(eyre!(RecoveryError::SessionChanged)),
                    Err(err) => {
                        if err.downcast_ref::<RecoveryError>().is_some() {
                            return Err(err);
                        }
                        let err_msg = format!("{:?}", err);
                        if classify_error_text(&err_msg) == EventSubFailure::AuthFailed {
                            return Err(err);
                        }
                        warn!("old EventSub socket ended during reconnect handoff: {err_msg}");
                        Ok(())
                    }
                }
            }
            SocketRead::Closed => {
                warn!("old EventSub socket closed during reconnect handoff");
                Ok(())
            }
            SocketRead::IdleTimeout => {
                warn!("old EventSub socket went idle during reconnect handoff");
                Ok(())
            }
            SocketRead::ReceiveError(err) => {
                warn!(
                    "old EventSub socket errored during reconnect handoff: {:?}",
                    err
                );
                Ok(())
            }
        }
    }

    /// Process a message from the websocket
    async fn process_message(
        self,
        msg: tungstenite::Message,
        ts: SyncSender<EventSubMessage>,
        client: &HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        mode: ConnectionMode,
        eventsub_settings: EventSubSettings,
    ) -> Result<SocketAction, Report> {
        match msg {
            tungstenite::Message::Text(s) => {
                trace!("process_message: {:?}", s);
                // Parse the message into a [twitch_api::eventsub::EventsubWebsocketData]
                let parsed = match Event::parse_websocket(&s) {
                    Ok(p) => p,
                    Err(e) => {
                        warn_or_warn_repeated(
                            eventsub_settings,
                            format!("eventsub_unparseable:{e}"),
                            format!("process_message - skipping unparseable message: {e}"),
                            eventsub_settings.unparseable_warning_throttle(),
                        );
                        return Ok(SocketAction::Continue);
                    }
                };
                match parsed {
                    EventsubWebsocketData::Welcome {
                        payload: WelcomePayload { session },
                        ..
                    } => {
                        let Some(snapshot) = token_manager.active_snapshot().await else {
                            return Err(eyre!(RecoveryError::SessionChanged));
                        };
                        let result = self
                            .process_welcome_message(
                                session,
                                client,
                                snapshot.token,
                                mode,
                                eventsub_settings,
                            )
                            .await;
                        if let Err(error) = result {
                            if classify_error_text(&format!("{error:?}"))
                                == EventSubFailure::AuthFailed
                            {
                                token_manager
                                    .recover_rejected_token(&snapshot.identity)
                                    .await?;
                                return Ok(SocketAction::FreshReconnect);
                            }
                            return Err(error);
                        }

                        Ok(SocketAction::Connected)
                    }
                    EventsubWebsocketData::Reconnect {
                        payload: ReconnectPayload { session },
                        ..
                    } => {
                        let Some(url) = session.reconnect_url else {
                            return Err(eyre!("eventsub reconnect missing reconnect_url"));
                        };
                        Ok(SocketAction::Reconnect(url.to_string()))
                    }
                    EventsubWebsocketData::Notification { metadata, payload } => {
                        ts.send(EventSubMessage::Notification(EventNotification {
                            ts: metadata.message_timestamp.into_owned(),
                            event: payload,
                        }))?;
                        Ok(SocketAction::Continue)
                    }
                    EventsubWebsocketData::Revocation { .. } => {
                        let raw: serde_json::Value = serde_json::from_str(&s)?;
                        match self.process_revocation(&raw["payload"]["subscription"])? {
                            RevocationAction::Account(session) => {
                                Err(eyre!(token_manager.revoked_authorization(&session).await))
                            }
                            RevocationAction::Room => Ok(SocketAction::Continue),
                            RevocationAction::Reconnect => Ok(SocketAction::FreshReconnect),
                        }
                    }
                    EventsubWebsocketData::Keepalive {
                        metadata: _,
                        payload: _,
                    } => Ok(SocketAction::Continue),
                    _ => Ok(SocketAction::Continue),
                }
            }
            tungstenite::Message::Close(frame) => {
                let reason = frame
                    .as_ref()
                    .map(|f| format!("code={}, reason={}", f.code, f.reason))
                    .unwrap_or_else(|| "no frame data".to_string());
                warn!("websocket closed: {}", reason);
                Err(eyre!("connection closed: {}", reason))
            }
            _ => Ok(SocketAction::Continue),
        }
    }

    async fn process_welcome_message(
        self,
        data: SessionData<'_>,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
        mode: ConnectionMode,
        eventsub_settings: EventSubSettings,
    ) -> Result<(), Report> {
        let session_id = data.id.to_string();
        debug!("welcome message - {} mode={:?}", session_id, mode);

        self.set_session_id(session_id.clone());

        if mode == ConnectionMode::ReconnectHandoff {
            debug!("EventSub reconnect handoff complete; subscriptions carried by Twitch");
            return Ok(());
        }

        self.clear_active_subscriptions();
        if let Err(err) = self
            .create_user_update_subscription(&session_id, client, &token)
            .await
        {
            let err_msg = format!("{:?}", err);
            if classify_error_text(&err_msg) == EventSubFailure::AuthFailed {
                return Err(err);
            }
            error_or_error_repeated(
                eventsub_settings,
                format!("eventsub_user_update_subscription:{err_msg}"),
                format!("failed to create user update EventSub subscription: {err_msg}"),
                eventsub_settings.subscription_error_throttle(),
            );
        }

        Ok(())
    }

    async fn create_user_update_subscription(
        &self,
        session_id: &str,
        client: &HelixClient<'static, reqwest::Client>,
        token: &UserToken,
    ) -> Result<(), Report> {
        debug!("subbing to user={} updates", token.login.clone());
        let transport = eventsub::Transport::websocket(session_id);
        let resp = match client
            .create_eventsub_subscription(
                eventsub::user::UserUpdateV1::new(token.user_id.clone()),
                transport.clone(),
                token,
            )
            .await
        {
            Ok(resp) => resp,
            Err(err) => return self.handle_create_subscription_error(err),
        };

        *self.user_update_subscription_id.lock().unwrap() = Some(resp.id.clone());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_recovery_access_401_reconnects_without_clearing_credentials() {
        let manager = EventSubManager::new();
        manager.add_desired_channel("maya".to_owned(), UserId::from_static("235835559"));
        let error =
            eyre!("AUTH_EXPIRED: helix returned error 401 - Unauthorized: Invalid OAuth token");
        assert_eq!(
            manager.action_for_report(&error),
            SocketAction::FreshReconnect
        );
        manager.clear_active_session();
        assert_eq!(manager.desired_channels_snapshot().len(), 1);
        assert_eq!(
            manager.action_for_report(&eyre!(RecoveryError::SessionChanged)),
            SocketAction::Stop
        );
    }

    #[test]
    fn auth_recovery_terminal_failure_preserves_credential_identity() {
        let failure = AuthFailure {
            identity: TokenIdentity {
                user_id: "1".to_owned(),
                session: 2,
                generation: 3,
            },
            issue: crate::token::AuthIssue::permanent("refresh_token_rejected"),
        };
        let error = eyre!(RecoveryError::ReauthenticationRequired(failure.clone()));
        assert_eq!(
            EventSubManager::new().action_for_report(&error),
            SocketAction::AuthFailed(failure)
        );
    }

    #[test]
    fn classifies_reconnect_and_subscription_errors() {
        assert_eq!(
            classify_error_text("connection closed: code=4007, reason=invalid reconnect attempt"),
            EventSubFailure::InvalidReconnect
        );
        assert_eq!(
            classify_error_text("helix returned error 409 - Conflict: subscription already exists"),
            EventSubFailure::DuplicateSubscription
        );
        assert_eq!(
            classify_error_text(
                "websocket transport session does not exist or has already disconnected"
            ),
            EventSubFailure::StaleSession
        );
        assert_eq!(
            classify_error_text("helix returned error 404 - Not Found: not found"),
            EventSubFailure::StaleSubscription
        );
        assert_eq!(
            classify_error_text("AUTH_EXPIRED: subscription revoked"),
            EventSubFailure::AuthFailed
        );
    }

    #[test]
    fn retry_delay_caps_at_sixty_seconds() {
        let settings = EventSubSettings::default();
        assert_eq!(retry_delay_secs(0, settings), 5);
        assert_eq!(retry_delay_secs(1, settings), 10);
        assert_eq!(retry_delay_secs(2, settings), 20);
        assert_eq!(retry_delay_secs(3, settings), 40);
        assert_eq!(retry_delay_secs(4, settings), 60);
        assert_eq!(retry_delay_secs(30, settings), 60);
    }

    #[test]
    fn retry_delay_uses_configured_base_and_cap() {
        let settings = EventSubSettings {
            retry_base_secs: 2,
            retry_max_secs: 12,
            ..Default::default()
        };

        assert_eq!(retry_delay_secs(0, settings), 2);
        assert_eq!(retry_delay_secs(1, settings), 4);
        assert_eq!(retry_delay_secs(2, settings), 8);
        assert_eq!(retry_delay_secs(3, settings), 12);
    }

    #[test]
    fn desired_channels_survive_active_session_clear() {
        let manager = EventSubManager::new();
        manager.add_desired_channel("maya".to_owned(), UserId::from_static("235835559"));
        manager.set_session_id("session-1".to_owned());
        manager.add_subscription(
            "maya".to_owned(),
            EventSubSubscription {
                channel_name: "maya".to_owned(),
                id: twitch_api::types::EventSubId::from_static("sub-1"),
                sub_type: eventsub::EventType::ChannelChatMessage,
            },
        );

        assert!(manager.has_subscription("maya"));
        manager.clear_active_session();

        assert!(!manager.has_subscription("maya"));
        assert_eq!(manager.session_id(), None);
        assert_eq!(manager.desired_channels_snapshot().len(), 1);
    }
}

#[cfg(test)]
mod subscription_tests {
    use super::*;
    use httpmock::{Method::POST, MockServer};
    use serde_json::json;

    #[derive(Clone)]
    struct MockClient {
        base: String,
        client: reqwest::Client,
    }
    impl twitch_api::client::Client for MockClient {
        type Error = reqwest::Error;
        fn req(
            &self,
            request: twitch_api::client::Request,
        ) -> twitch_api::client::BoxedFuture<'_, Result<twitch_api::client::Response, Self::Error>>
        {
            Box::pin(async move {
                let (parts, body) = request.into_parts();
                let url = format!("{}{}", self.base, parts.uri.path_and_query().unwrap());
                let received = self
                    .client
                    .request(parts.method, url)
                    .headers(parts.headers)
                    .body(body)
                    .send()
                    .await?;
                let status = received.status();
                let headers = received.headers().clone();
                let mut response = twitch_api::client::Response::new(received.bytes().await?);
                *response.status_mut() = status;
                *response.headers_mut() = headers;
                Ok(response)
            })
        }
    }
    fn token() -> UserToken {
        UserToken::from_existing_unchecked(
            "token",
            None,
            "client",
            None,
            "tester".into(),
            "user".into(),
            None,
            None,
        )
    }
    fn response(kind: &str, id: &str) -> serde_json::Value {
        json!({"data": [{
            "id": id, "status": "enabled", "type": kind, "version": "1", "cost": 0,
            "condition": {"broadcaster_user_id":"123", "user_id":"user"},
            "created_at": "2026-10-25T00:00:00Z",
            "transport": {"method":"websocket", "session_id":"session", "connected_at":"2026-10-25T00:00:00Z"}
        }], "total":1, "total_cost":0, "max_total_cost":10})
    }
    #[tokio::test]
    async fn joining_without_a_session_is_pending_without_making_http_requests() {
        let server = MockServer::start();
        let post = server.mock(|when, then| {
            when.method(POST).path("/helix/eventsub/subscriptions");
            then.status(500);
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        assert_eq!(
            manager
                .clone()
                .join_chat("123".into(), "room".into(), &client, token())
                .await
                .unwrap(),
            JoinOutcome::Pending
        );
        assert!(!manager.channel_complete("room"));
        assert_eq!(manager.desired_channels_snapshot().len(), 1);
        post.assert_hits(0);
    }

    #[tokio::test]
    async fn stale_session_join_is_pending_instead_of_joined() {
        let server = MockServer::start();
        let post = server.mock(|when, then| {
            when.method(POST).path("/helix/eventsub/subscriptions");
            then.status(400).json_body(json!({"error":"Bad Request","status":400,"message":"websocket transport session does not exist"}));
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        manager.set_session_id("session".into());
        assert_eq!(
            manager
                .clone()
                .join_chat("123".into(), "room".into(), &client, token())
                .await
                .unwrap(),
            JoinOutcome::Pending
        );
        assert!(!manager.is_connected());
        assert!(!manager.channel_complete("room"));
        post.assert_hits(1);
    }

    #[tokio::test]
    async fn denied_room_can_recover_after_partial_candidate_cleanup() {
        use httpmock::Method::DELETE;
        let server = MockServer::start();
        let message = server.mock(|when, then| {
            when.method(POST)
                .path("/helix/eventsub/subscriptions")
                .json_body_partial(r#"{"type":"channel.chat.message"}"#);
            then.status(202)
                .json_body(response("channel.chat.message", "message"));
        });
        let mut denied = server.mock(|when, then| {
            when.method(POST).path("/helix/eventsub/subscriptions").json_body_partial(r#"{"type":"channel.chat.notification"}"#);
            then.status(403).json_body(json!({"error":"Forbidden","status":403,"message":"subscription missing proper authorization"}));
        });
        let deleted = server.mock(|when, then| {
            when.method(DELETE)
                .path("/helix/eventsub/subscriptions")
                .query_param("id", "message");
            then.status(204);
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        manager.set_session_id("session".into());
        let error = manager
            .clone()
            .join_chat("123".into(), "room".into(), &client, token())
            .await
            .unwrap_err();
        assert_eq!(subscription_error_status::<MockClient>(&error), Some(403));
        assert_eq!(manager.channels_with_subscriptions().len(), 1);
        assert!(!manager.channel_complete("room"));
        manager
            .clone()
            .leave_chat("room".into(), &client, token())
            .await
            .unwrap();
        assert!(manager.channels_with_subscriptions().is_empty());
        assert!(manager.desired_channels_snapshot().is_empty());
        deleted.assert_hits(1);
        denied.delete();
        let notification = server.mock(|when, then| {
            when.method(POST)
                .path("/helix/eventsub/subscriptions")
                .json_body_partial(r#"{"type":"channel.chat.notification"}"#);
            then.status(202)
                .json_body(response("channel.chat.notification", "notification"));
        });
        assert_eq!(
            manager
                .clone()
                .join_chat("123".into(), "room".into(), &client, token())
                .await
                .unwrap(),
            JoinOutcome::Joined
        );
        assert!(manager.channel_complete("room"));
        message.assert_hits(2);
        notification.assert_hits(1);
    }

    #[test]
    fn room_revocations_are_consumed_instead_of_becoming_permanent_blacklists() {
        let manager = EventSubManager::new();
        manager
            .revoked_rooms
            .lock()
            .unwrap()
            .insert("room".into(), "chat_user_banned".into());
        assert_eq!(
            manager.take_room_failures().get("room").map(String::as_str),
            Some("chat_user_banned")
        );
        assert!(manager.take_room_failures().is_empty());
    }

    #[tokio::test]
    async fn uncertain_creates_keep_capacity_reserved_until_receipts_are_reconciled_and_deleted() {
        use httpmock::Method::{DELETE, GET};
        for status in [202, 203] {
            let server = MockServer::start();
            server.mock(|when, then| {
                when.method(POST).path("/helix/eventsub/subscriptions");
                then.status(status).body("lost or invalid create response");
            });
            let mut unavailable = server.mock(|when, then| {
                when.method(GET).path("/helix/eventsub/subscriptions");
                then.status(429).json_body(
                    json!({"error":"Too Many Requests", "status":429, "message":"rate limited"}),
                );
            });
            let client = HelixClient::with_client(MockClient {
                base: server.base_url(),
                client: reqwest::Client::new(),
            });
            let manager = EventSubManager::new();
            manager.set_session_id("session".into());
            assert!(manager
                .clone()
                .join_chat("123".into(), "room".into(), &client, token())
                .await
                .is_err());
            assert!(manager.get_subscriptions("room").is_empty());
            assert!(manager.channels_with_subscriptions().contains("room"));
            let error = manager
                .clone()
                .leave_chat("room".into(), &client, token())
                .await
                .unwrap_err();
            assert_eq!(subscription_error_status::<MockClient>(&error), Some(429));
            assert!(manager.channels_with_subscriptions().contains("room"));
            unavailable.delete();
            let listing = server.mock(|when, then| {
                when.method(GET).path("/helix/eventsub/subscriptions");
                then.status(200)
                    .json_body(response("channel.chat.message", "created-message"));
            });
            let deleted = server.mock(|when, then| {
                when.method(DELETE)
                    .path("/helix/eventsub/subscriptions")
                    .query_param("id", "created-message");
                then.status(204);
            });
            manager
                .clone()
                .leave_chat("room".into(), &client, token())
                .await
                .unwrap();
            assert!(manager.channels_with_subscriptions().is_empty());
            assert!(manager.desired_channels_snapshot().is_empty());
            listing.assert_hits(1);
            deleted.assert_hits(1);
        }
    }

    #[tokio::test]
    async fn partial_subscription_failure_retries_only_missing_type() {
        let server = MockServer::start();
        let message = server.mock(|when, then| {
            when.method(POST)
                .path("/helix/eventsub/subscriptions")
                .json_body_partial(r#"{"type":"channel.chat.message"}"#);
            then.status(202)
                .json_body(response("channel.chat.message", "message"));
        });
        let mut failure = server.mock(|when, then| {
            when.method(POST)
                .path("/helix/eventsub/subscriptions")
                .json_body_partial(r#"{"type":"channel.chat.notification"}"#);
            then.status(429).json_body(
                json!({"error":"Too Many Requests","status":429,"message":"rate limited"}),
            );
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        manager.set_session_id("session".into());
        let error = manager
            .create_channel_subscriptions("123".into(), "room".into(), "session", &client, &token())
            .await
            .unwrap_err();
        assert_eq!(subscription_error_status::<MockClient>(&error), Some(429));
        assert!(!manager.channel_complete("room"));
        assert_eq!(manager.get_subscriptions("room").len(), 1);
        failure.delete();
        let notification = server.mock(|when, then| {
            when.method(POST)
                .path("/helix/eventsub/subscriptions")
                .json_body_partial(r#"{"type":"channel.chat.notification"}"#);
            then.status(202)
                .json_body(response("channel.chat.notification", "notification"));
        });
        manager
            .create_channel_subscriptions("123".into(), "room".into(), "session", &client, &token())
            .await
            .unwrap();
        assert!(manager.channel_complete("room"));
        message.assert_hits(1);
        notification.assert_hits(1);
    }

    #[tokio::test]
    async fn duplicate_receipts_are_recovered_for_current_session_only() {
        use httpmock::Method::GET;
        let server = MockServer::start();
        let duplicate = server.mock(|when, then| {
            when.method(POST).path("/helix/eventsub/subscriptions");
            then.status(409).json_body(
                json!({"error":"Conflict","status":409,"message":"subscription already exists"}),
            );
        });
        let mut listed = response("channel.chat.message", "message");
        let notification = response("channel.chat.notification", "notification");
        let mut wrong_session = listed["data"][0].clone();
        wrong_session["id"] = json!("other-session");
        wrong_session["transport"]["session_id"] = json!("old-session");
        listed["data"]
            .as_array_mut()
            .unwrap()
            .push(notification["data"][0].clone());
        listed["data"].as_array_mut().unwrap().push(wrong_session);
        listed["total"] = json!(3);
        let listing = server.mock(|when, then| {
            when.method(GET).path("/helix/eventsub/subscriptions");
            then.status(200).json_body(listed);
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        manager.set_session_id("session".into());
        manager
            .create_channel_subscriptions("123".into(), "room".into(), "session", &client, &token())
            .await
            .unwrap();
        assert!(manager.channel_complete("room"));
        assert_eq!(manager.get_subscriptions("room").len(), 2);
        duplicate.assert_hits(1);
        listing.assert_hits(1);
    }

    #[tokio::test]
    async fn failed_leave_keeps_cleanup_pending_without_repeating_successful_deletes() {
        use httpmock::Method::DELETE;
        let server = MockServer::start();
        for (kind, id) in [
            ("channel.chat.message", "message"),
            ("channel.chat.notification", "notification"),
        ] {
            server.mock(|when, then| {
                when.method(POST)
                    .path("/helix/eventsub/subscriptions")
                    .json_body_partial(format!(r#"{{"type":"{kind}"}}"#));
                then.status(202).json_body(response(kind, id));
            });
        }
        let deleted = server.mock(|when, then| {
            when.method(DELETE)
                .path("/helix/eventsub/subscriptions")
                .query_param("id", "message");
            then.status(204);
        });
        let mut failed = server.mock(|when, then| {
            when.method(DELETE)
                .path("/helix/eventsub/subscriptions")
                .query_param("id", "notification");
            then.status(500).json_body(
                json!({"error":"Internal Server Error","status":500,"message":"temporary failure"}),
            );
        });
        let client = HelixClient::with_client(MockClient {
            base: server.base_url(),
            client: reqwest::Client::new(),
        });
        let manager = EventSubManager::new();
        manager.set_session_id("session".into());
        manager.add_desired_channel("room".into(), "123".into());
        manager
            .create_channel_subscriptions("123".into(), "room".into(), "session", &client, &token())
            .await
            .unwrap();
        assert!(manager
            .clone()
            .leave_chat("room".into(), &client, token())
            .await
            .is_err());
        assert_eq!(manager.desired_channels_snapshot().len(), 1);
        // HashSet iteration may encounter the failed deletion before the successful one.
        let previously_deleted = deleted.hits();
        failed.delete();
        let retried = server.mock(|when, then| {
            when.method(DELETE)
                .path("/helix/eventsub/subscriptions")
                .query_param("id", "notification");
            then.status(204);
        });
        manager
            .clone()
            .leave_chat("room".into(), &client, token())
            .await
            .unwrap();
        assert!(manager.desired_channels_snapshot().is_empty());
        assert!(manager.get_subscriptions("room").is_empty());
        deleted.assert_hits(if previously_deleted == 0 {
            1
        } else {
            previously_deleted
        });
        retried.assert_hits(1);
    }
    #[test]
    fn room_or_version_revocation_does_not_expire_account() {
        assert!(!account_revoked("version_removed", Some("user"), "user"));
        assert!(!account_revoked(
            "user_removed",
            Some("broadcaster"),
            "user"
        ));
        assert!(!account_revoked("authorization_revoked", None, "user"));
        assert!(account_revoked(
            "authorization_revoked",
            Some("user"),
            "user"
        ));
        assert!(account_revoked("user_removed", Some("user"), "user"));
    }

    fn manager_with_auth_session() -> (EventSubManager, TokenIdentity) {
        let manager = EventSubManager::new();
        let identity = TokenIdentity {
            user_id: "user".into(),
            session: 2,
            generation: 3,
        };
        *manager.auth_session.lock().unwrap() = Some(identity.clone());
        (manager, identity)
    }

    #[test]
    fn account_revocations_use_socket_identity_for_guarded_auth_failure() {
        let (manager, identity) = manager_with_auth_session();
        for status in ["authorization_revoked", "user_removed"] {
            let subscription = serde_json::json!({
                "status": status,
                "condition": {"user_id": "user", "broadcaster_user_id": "123"}
            });
            assert_eq!(
                manager.process_revocation(&subscription).unwrap(),
                RevocationAction::Account(identity.clone())
            );
            assert!(manager.take_room_failures().is_empty());
        }
    }

    #[test]
    fn room_revocations_queue_cleanup_without_discarding_other_rooms() {
        let (manager, _) = manager_with_auth_session();
        manager.add_desired_channel("room".into(), "123".into());
        manager.add_desired_channel("other".into(), "456".into());
        manager.add_subscription(
            "other".into(),
            EventSubSubscription {
                channel_name: "other".into(),
                id: "other-message".into(),
                sub_type: eventsub::EventType::ChannelChatMessage,
            },
        );
        for (status, user) in [
            ("chat_user_banned", "user"),
            ("version_removed", "user"),
            ("user_removed", "broadcaster"),
        ] {
            let subscription = serde_json::json!({
                "status": status,
                "condition": {"user_id": user, "broadcaster_user_id": "123"}
            });
            assert_eq!(
                manager.process_revocation(&subscription).unwrap(),
                RevocationAction::Room
            );
            assert_eq!(
                manager.take_room_failures(),
                HashMap::from([("room".into(), status.into())])
            );
            assert_eq!(manager.desired_channels_snapshot().len(), 2);
            assert_eq!(manager.get_subscriptions("other").len(), 1);
        }
    }

    #[test]
    fn non_room_subscription_revocation_requests_reconnect() {
        let (manager, _) = manager_with_auth_session();
        let subscription = serde_json::json!({
            "status": "version_removed",
            "condition": {"user_id": "user"}
        });
        assert_eq!(
            manager.process_revocation(&subscription).unwrap(),
            RevocationAction::Reconnect
        );
        assert!(manager.take_room_failures().is_empty());
    }

    #[test]
    fn revocation_without_auth_session_stops_stale_socket() {
        let manager = EventSubManager::new();
        let subscription = serde_json::json!({
            "status": "authorization_revoked",
            "condition": {"user_id": "user"}
        });
        let error = manager.process_revocation(&subscription).unwrap_err();
        assert_eq!(
            error.downcast_ref::<RecoveryError>(),
            Some(&RecoveryError::SessionChanged)
        );
    }
}

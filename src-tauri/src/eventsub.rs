use color_eyre::Report;
use eyre::eyre;
use eyre::WrapErr;
use futures::StreamExt;
#[cfg(debug_assertions)]
use futures::{stream, TryStreamExt};
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

use crate::{logging, token::TokenManager, types::EventSubSettings};

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
    AuthFailed(String),
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
    AuthFailed(String),
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

    fn desired_channels_snapshot(&self) -> Vec<(String, UserId)> {
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

    fn has_subscription(&self, channel_name: &str) -> bool {
        let guard = self.subscriptions.lock().unwrap();
        guard
            .get(channel_name)
            .map(|subs| !subs.lock().unwrap().is_empty())
            .unwrap_or(false)
    }

    fn get_subscriptions(&self, channel_name: &str) -> Vec<EventSubSubscription> {
        let guard = self.subscriptions.lock().unwrap();
        guard
            .get(channel_name)
            .map(|subs| subs.lock().unwrap().iter().cloned().collect())
            .unwrap_or_default()
    }

    pub async fn join_chat(
        self,
        chat_id: UserId,
        chat_name: String,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        self.add_desired_channel(chat_name.clone(), chat_id.clone());

        let Some(session_id) = self.session_id() else {
            debug!(
                "EventSubManager - queued channel until websocket is connected: chat={}",
                chat_name
            );
            return Ok(());
        };

        if self.has_subscription(&chat_name) {
            debug!(
                "EventSubManager - channel already subbed to: chat={}",
                chat_name,
            );
            return Ok(());
        }

        match self
            .create_channel_subscriptions(chat_id, chat_name.clone(), &session_id, client, &token)
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                let err_msg = format!("{:?}", err);
                if classify_error_text(&err_msg) == EventSubFailure::StaleSession {
                    warn!(
                        "EventSubManager - session is stale while joining {}; queued for reconnect",
                        chat_name
                    );
                    self.clear_active_session();
                    return Ok(());
                }
                self.remove_desired_channel(&chat_name);
                Err(err)
            }
        }
    }

    async fn create_channel_subscriptions(
        &self,
        chat_id: UserId,
        chat_name: String,
        session_id: &str,
        client: &HelixClient<'static, reqwest::Client>,
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
        match client
            .create_eventsub_subscription(message.clone(), transport.clone(), token)
            .await
        {
            Ok(resp) => {
                self.add_subscription(
                    chat_name.clone(),
                    EventSubSubscription {
                        channel_name: chat_name.clone(),
                        id: resp.id.clone(),
                        sub_type: resp.type_.clone(),
                    },
                );
            }
            Err(err) => self.handle_create_subscription_error(err)?,
        }

        debug!(
            "EventSubManager - creating ChannelChatNotificationV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let condition = eventsub::channel::chat::ChannelChatNotificationV1::new(
            chat_id.clone(),
            user_id.clone(),
        );
        match client
            .create_eventsub_subscription(condition.clone(), transport.clone(), &token.clone())
            .await
        {
            Ok(resp) => {
                self.add_subscription(
                    chat_name.clone(),
                    EventSubSubscription {
                        channel_name: chat_name.clone(),
                        id: resp.id.clone(),
                        sub_type: resp.type_.clone(),
                    },
                );
            }
            Err(err) => self.handle_create_subscription_error(err)?,
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

    pub async fn leave_chat(
        self,
        chat_name: String,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        self.remove_desired_channel(&chat_name);

        if self.session_id().is_none() {
            self.remove_subscriptions(&chat_name);
            return Ok(());
        }

        if !self.has_subscription(&chat_name) {
            debug!("EventSubManager - sub doesn't exist: chat={}", chat_name);
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
                            return Ok(());
                        }
                        EventSubFailure::AuthFailed => {
                            return Err(eyre!("AUTH_EXPIRED: {}", err_str));
                        }
                        _ => return Err(eyre!(err)),
                    }
                }
            }
        }
        self.remove_subscriptions(&chat_name);

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

            loop {
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
                retry_attempt = 0;

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
                        SocketAction::Continue | SocketAction::Connected => {}
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
                                    if classify_error_text(&err_msg) == EventSubFailure::AuthFailed
                                    {
                                        let _ = std_tx.send(EventSubMessage::AuthFailed(
                                            "token expired or revoked".into(),
                                        ));
                                        return;
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
                        self.action_for_failure(classify_error_text(&err_msg), err_msg)
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
            EventSubFailure::AuthFailed => {
                SocketAction::AuthFailed("token expired or revoked".into())
            }
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
                        SocketAction::AuthFailed(reason) => {
                            return Err(eyre!("AUTH_EXPIRED: {}", reason));
                        }
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
                    Ok(SocketAction::AuthFailed(reason)) => Err(eyre!("AUTH_EXPIRED: {}", reason)),
                    Err(err) => {
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
                        let Some(current_token) = token_manager.active_twitch_token().await else {
                            return Err(eyre!("AUTH_EXPIRED: no active token"));
                        };
                        self.process_welcome_message(
                            session,
                            client,
                            current_token,
                            mode,
                            eventsub_settings,
                        )
                        .await?;

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
                    re @ EventsubWebsocketData::Revocation { .. } => {
                        Err(eyre!("AUTH_EXPIRED: subscription revoked: {re:?}"))
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

        self.resubscribe_desired_channels(&session_id, client, &token, eventsub_settings)
            .await?;

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

    async fn resubscribe_desired_channels(
        &self,
        session_id: &str,
        client: &HelixClient<'static, reqwest::Client>,
        token: &UserToken,
        eventsub_settings: EventSubSettings,
    ) -> Result<(), Report> {
        for (channel_name, chat_id) in self.desired_channels_snapshot() {
            if self.has_subscription(&channel_name) {
                continue;
            }

            if let Err(err) = self
                .create_channel_subscriptions(
                    chat_id,
                    channel_name.clone(),
                    session_id,
                    client,
                    token,
                )
                .await
            {
                let err_msg = format!("{:?}", err);
                match classify_error_text(&err_msg) {
                    EventSubFailure::AuthFailed | EventSubFailure::StaleSession => {
                        return Err(err);
                    }
                    _ => error_or_error_repeated(
                        eventsub_settings,
                        format!("eventsub_channel_resubscribe:{channel_name}:{err_msg}"),
                        format!("failed to resubscribe EventSub channel {channel_name}: {err_msg}"),
                        eventsub_settings.subscription_error_throttle(),
                    ),
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

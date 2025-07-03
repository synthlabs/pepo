use color_eyre::Report;
use eyre::eyre;
use eyre::WrapErr;
use futures::future::BoxFuture;
use futures::FutureExt;
use futures::{stream, TryStreamExt};
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info};
use twitch_api::types::UserId;
use twitch_api::{
    eventsub::{self, Event, EventsubWebsocketData, ReconnectPayload, SessionData, WelcomePayload},
    HelixClient,
};
use twitch_oauth2::UserToken;
type SharedMap<V> = Arc<Mutex<HashMap<String, Mutex<HashSet<V>>>>>;

#[derive(Debug)]
pub struct EventNotification {
    pub ts: twitch_api::types::Timestamp,
    pub event: Event,
}

#[derive(Debug)]
pub struct EventSubSubscription {
    pub cost: usize,
    pub condition: serde_json::Value,
    pub created_at: twitch_api::types::Timestamp,
    pub id: twitch_api::types::EventSubId,
    pub status: eventsub::Status,
}

#[derive(Clone)]
pub struct EventSubManager {
    session_id: Arc<Mutex<String>>,
    connect_url: Arc<Mutex<String>>,
    subscriptions: SharedMap<String>,
}

/// Connect to the websocket and return the stream
async fn connect(
    connect_url: String,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Report,
> {
    info!(url = connect_url, "connecting to twitch");
    let config = tungstenite::protocol::WebSocketConfig::default();
    let (socket, _) =
        tokio_tungstenite::connect_async_with_config(connect_url, Some(config), false)
            .await
            .wrap_err("Can't connect")?;

    Ok(socket)
}

impl EventSubManager {
    pub fn new() -> EventSubManager {
        EventSubManager {
            session_id: Arc::new(Mutex::new("".to_owned())),
            connect_url: Arc::new(Mutex::new(
                twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL
                    .as_str()
                    .to_owned(),
            )),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add_subscription(self, channel_name: String, id: String) {
        let mut guard = self.subscriptions.lock().unwrap();

        let mut subs = guard.entry(channel_name).or_default().lock().unwrap();
        subs.insert(id);
    }

    fn remove_subscription(self, channel_name: String, id: String) {
        let mut guard = self.subscriptions.lock().unwrap();

        let mut subs = guard.entry(channel_name).or_default().lock().unwrap();
        subs.remove(&id);
    }

    fn get_subscriptions(self, channel_name: String) -> Vec<String> {
        let mut guard = self.subscriptions.lock().unwrap();

        let mut subs = guard.entry(channel_name).or_default().lock().unwrap();

        subs.iter().map(|s| s.clone()).collect()
    }

    pub async fn join_chat(
        self,
        chat_id: UserId,
        chat_name: String,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        let session_id = {
            let guard = self.session_id.lock().unwrap();
            (*guard).clone()
        };

        if session_id == "" {
            return Err(eyre!("session id not set"));
        }

        let transport = eventsub::Transport::websocket(session_id.clone());
        debug!(
            "EventSubManager - creating ChannelChatMessageV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let user_id = token.clone().user_id;
        let message =
            eventsub::channel::chat::ChannelChatMessageV1::new(chat_id.clone(), user_id.clone());
        let resp = client
            .create_eventsub_subscription(message.clone(), transport.clone(), &token)
            .await?;

        self.clone()
            .add_subscription(chat_name.clone(), resp.id.clone().to_string());

        debug!(
            "EventSubManager - creating ChannelChatNotificationV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let condition = eventsub::channel::chat::ChannelChatNotificationV1::new(
            chat_id.clone(),
            user_id.clone(),
        );
        let resp = client
            .create_eventsub_subscription(condition.clone(), transport.clone(), &token.clone())
            .await?;

        self.add_subscription(chat_name, resp.id.clone().to_string());

        Ok(())
    }

    pub async fn leave_chat(
        self,
        chat_name: String,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        let session_id = {
            let guard = self.session_id.lock().unwrap();
            (*guard).clone()
        };

        if session_id == "" {
            return Err(eyre!("session id not set"));
        }

        let subs = self.get_subscriptions(chat_name);

        for s in subs {
            client
                .delete_eventsub_subscription(s, &token.clone())
                .await?;
        }

        Ok(())
    }

    pub fn start(
        self,
        client: HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<std::sync::mpsc::Receiver<EventNotification>, Report> {
        let connect_url_ref = self.connect_url.clone();

        let (std_tx, std_rx) = std::sync::mpsc::sync_channel::<EventNotification>(32);

        #[cfg(debug_assertions)]
        {
            let token = token.clone();
            let client = client.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let chatters: Vec<twitch_api::eventsub::EventSubSubscription> = client
                        .get_eventsub_subscriptions(None, None, None, &token.clone())
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
                        .unwrap();

                    // pub condition: Value,
                    // pub created_at: Timestamp,
                    // pub id: EventSubId,
                    // pub status: Status,
                    for sub in chatters.iter() {
                        debug!(
                            "EventSubSubscription: id={}, cost={}, condition={:?}, status={:?}",
                            sub.id, sub.cost, sub.condition, sub.status
                        )
                    }
                    debug!("EventSubManger::cost_watcher - tick=10s");
                    sleep(Duration::from_secs(10)).await;
                }
            });
        }

        tauri::async_runtime::spawn(async move {
            loop {
                debug!("connecting to websocket");

                let connect_url: String = {
                    let guard = connect_url_ref.lock().unwrap();
                    (*guard).clone()
                };
                // Establish the stream
                let mut s = connect(connect_url)
                    .await
                    .context("when establishing connection")
                    .unwrap();

                while let Some(msg) = futures::StreamExt::next(&mut s).await {
                    debug!("message received: {:?}", msg);
                    let msg = match msg {
                        Err(tungstenite::Error::Protocol(
                            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                        )) => {
                            error!(
                                "connection was sent an unexpected frame or was reset, reestablishing it"
                            );
                            break;
                        }
                        _ => msg.context("when getting message").unwrap(),
                    };
                    let self_ref = self.clone();
                    match self_ref
                        .process_message(msg, std_tx.clone(), &client, token.clone())
                        .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("process_message - {:?}", e);
                            break;
                        }
                    };
                }

                debug!("EventSubManger::run - tick=10s");
                sleep(Duration::from_secs(10)).await;
            }
        });
        Ok(std_rx)
    }

    /// Process a message from the websocket
    async fn process_message(
        self,
        msg: tungstenite::Message,
        ts: SyncSender<EventNotification>,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        match msg {
            tungstenite::Message::Text(s) => {
                debug!("{:?}", s);
                // Parse the message into a [twitch_api::eventsub::EventsubWebsocketData]
                match Event::parse_websocket(&s)? {
                    EventsubWebsocketData::Welcome {
                        payload: WelcomePayload { session },
                        ..
                    }
                    | EventsubWebsocketData::Reconnect {
                        payload: ReconnectPayload { session },
                        ..
                    } => {
                        self.process_welcome_message(session, client, token.clone())
                            .await?;

                        Ok(())
                    }
                    EventsubWebsocketData::Notification { metadata, payload } => {
                        ts.send(EventNotification {
                            ts: metadata.message_timestamp.into_owned(),
                            event: payload,
                        })?;
                        Ok(())
                    }
                    re @ EventsubWebsocketData::Revocation { .. } => {
                        Err(eyre!("got revocation event: {re:?}"))
                    }
                    EventsubWebsocketData::Keepalive {
                        metadata: _,
                        payload: _,
                    } => Ok(()),
                    _ => Ok(()),
                }
            }
            tungstenite::Message::Close(_) => Err(eyre!("connection closed")),
            _ => Ok(()),
        }
    }

    async fn process_welcome_message(
        self,
        data: SessionData<'_>,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        let session_id = data.id.to_string();
        debug!("welcome message - {}", session_id);

        *self.session_id.lock().unwrap() = session_id.clone();

        if let Some(url) = data.reconnect_url {
            *self.connect_url.lock().unwrap() = url.to_string();
        }

        debug!("subbing to user={} updates", token.login.clone());
        let transport = eventsub::Transport::websocket(session_id.clone());
        let _resp = client
            .create_eventsub_subscription(
                eventsub::user::UserUpdateV1::new(token.user_id.clone()),
                transport.clone(),
                &token,
            )
            .await?;

        Ok(())
    }
}

use color_eyre::Report;
use eyre::eyre;
use eyre::WrapErr;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info};
use twitch_api::types::UserId;
use twitch_api::{
    eventsub::{self, Event, EventsubWebsocketData, ReconnectPayload, SessionData, WelcomePayload},
    HelixClient,
};
use twitch_oauth2::UserToken;

fn cow_to_static(cow: Cow<'_, str>) -> &'static str {
    let s: String = cow.into_owned();
    Box::leak(s.into_boxed_str())
}

#[derive(Clone, Copy)]
pub struct EventSubManager {
    session_id: &'static str,
    connect_url: &'static str,
}

impl EventSubManager {
    pub fn create() -> EventSubManager {
        EventSubManager {
            session_id: "",
            connect_url: twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.as_str().clone(),
        }
    }

    /// Connect to the websocket and return the stream
    async fn connect(
        &mut self,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Report,
    > {
        info!(url = self.connect_url, "connecting to twitch");
        let config = tungstenite::protocol::WebSocketConfig::default();
        let (socket, _) =
            tokio_tungstenite::connect_async_with_config(self.connect_url, Some(config), false)
                .await
                .wrap_err("Can't connect")?;

        Ok(socket)
    }

    pub async fn join_chat(
        chat_id: UserId,
        session_id: &'static str,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        let transport = eventsub::Transport::websocket(session_id);
        debug!(
            "EventSubManager - creating ChannelChatMessageV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        let user_id = token.clone().user_id;
        let message =
            eventsub::channel::chat::ChannelChatMessageV1::new(chat_id.clone(), user_id.clone());
        client
            .create_eventsub_subscription(message, transport.clone(), &token)
            .await?;

        debug!(
            "EventSubManager - creating ChannelChatNotificationV1: user_id={}, session_id={}",
            chat_id, session_id
        );
        client
            .create_eventsub_subscription(
                eventsub::channel::chat::ChannelChatNotificationV1::new(
                    chat_id.clone(),
                    user_id.clone(),
                ),
                transport.clone(),
                &token.clone(),
            )
            .await?;

        Ok(())
    }

    pub async fn leave_chat(channel_name: String, session_id: String) {
        debug!(
            "EventSubManager - leave: channel_name={}, session_id={}",
            channel_name, session_id
        );
    }

    pub async fn run<Fut>(
        mut self,
        mut event_fn: impl FnMut(Event, twitch_api::types::Timestamp) -> Fut,
        client: HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report>
    where
        Fut: std::future::Future<Output = Result<(), Report>>,
    {
        loop {
            debug!("connecting to websocket");
            // Establish the stream
            let mut s = self
                .connect()
                .await
                .context("when establishing connection")?;

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
                    _ => msg.context("when getting message")?,
                };
                match self
                    .process_message(msg, &mut event_fn, &client, token.clone())
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!("process_message - {:?}", e);
                        break;
                    }
                };
            }

            debug!("EventSubManger - tick=10s");
            sleep(Duration::from_secs(10)).await;
        }
    }

    /// Process a message from the websocket
    async fn process_message<Fut>(
        &mut self,
        msg: tungstenite::Message,
        event_fn: &mut impl FnMut(Event, twitch_api::types::Timestamp) -> Fut,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report>
    where
        Fut: std::future::Future<Output = Result<(), Report>>,
    {
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
                        // let transport = eventsub::Transport::websocket(
                        //     self.session_id.clone().unwrap_or_default(),
                        // );
                        // let message =
                        //     eventsub::stream::StreamOnlineV1::broadcaster_user_id("207813352");
                        // let token = self.token.lock().await;
                        // self.client
                        //     .create_eventsub_subscription(message, transport.clone(), &*token)
                        //     .await?;
                        Ok(())
                    }
                    EventsubWebsocketData::Notification { metadata, payload } => {
                        event_fn(payload, metadata.message_timestamp.into_owned()).await?;
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
        &mut self,
        data: SessionData<'_>,
        client: &HelixClient<'static, reqwest::Client>,
        token: UserToken,
    ) -> Result<(), Report> {
        self.session_id = cow_to_static(data.id);
        debug!("welcome message - {}", self.session_id);
        if let Some(url) = data.reconnect_url {
            self.connect_url = cow_to_static(url);
        }

        let user_id = token.clone().user_id;
        Self::join_chat(user_id, self.session_id, client, token.clone()).await?;

        Ok(())
    }
}

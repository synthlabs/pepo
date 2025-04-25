use color_eyre::Report;
use eyre::WrapErr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info, warn};
use twitch_api::{
    eventsub::{Event, EventsubWebsocketData, ReconnectPayload, SessionData, WelcomePayload},
    HelixClient,
};
use twitch_oauth2::UserToken;

pub struct EventSubManager {
    pub session_id: Option<String>,
    pub _token: Arc<Mutex<UserToken>>,
    pub _client: HelixClient<'static, reqwest::Client>,
    connect_url: twitch_oauth2::url::Url,
    _chats: Arc<Mutex<Vec<twitch_api::types::UserId>>>,
}

impl EventSubManager {
    pub fn create(
        session_id: Option<String>,
        token: UserToken,
        client: HelixClient<'static, reqwest::Client>,
    ) -> EventSubManager {
        EventSubManager {
            session_id,
            _token: Arc::new(tokio::sync::Mutex::new(token)),
            _client: client.clone(),
            connect_url: twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.clone(),
            _chats: Arc::new(Mutex::new(vec![])),
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
        info!(url = self.connect_url.as_str(), "connecting to twitch");
        let config = tungstenite::protocol::WebSocketConfig::default();
        let (socket, _) =
            tokio_tungstenite::connect_async_with_config(&self.connect_url, Some(config), false)
                .await
                .wrap_err("Can't connect")?;

        Ok(socket)
    }

    pub async fn run<Fut>(
        mut self,
        mut event_fn: impl FnMut(Event, twitch_api::types::Timestamp) -> Fut,
    ) -> Result<(), Report>
    where
        Fut: std::future::Future<Output = Result<(), Report>>,
    {
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
                    s = self
                        .connect()
                        .await
                        .context("when reestablishing connection")?;
                    continue;
                }
                _ => msg.context("when getting message")?,
            };
            self.process_message(msg, &mut event_fn).await?
        }
        Ok(())
    }

    /// Process a message from the websocket
    async fn process_message<Fut>(
        &mut self,
        msg: tungstenite::Message,
        event_fn: &mut impl FnMut(Event, twitch_api::types::Timestamp) -> Fut,
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
                        self.process_welcome_message(session).await?;
                        Ok(())
                    }
                    EventsubWebsocketData::Notification { metadata, payload } => {
                        event_fn(payload, metadata.message_timestamp.into_owned()).await?;
                        Ok(())
                    }
                    re @ EventsubWebsocketData::Revocation { .. } => {
                        eyre::bail!("got revocation event: {re:?}")
                    }
                    EventsubWebsocketData::Keepalive {
                        metadata: _,
                        payload: _,
                    } => Ok(()),
                    _ => Ok(()),
                }
            }
            tungstenite::Message::Close(_) => eyre::bail!("connection closed"),
            _ => Ok(()),
        }
    }

    async fn process_welcome_message(&mut self, data: SessionData<'_>) -> Result<(), Report> {
        debug!("welcome message");
        self.session_id = Some(data.id.to_string());
        if let Some(url) = data.reconnect_url {
            self.connect_url = url.parse()?;
        }
        Ok(())
    }
}

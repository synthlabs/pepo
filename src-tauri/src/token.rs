use lazy_static::lazy_static;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error};
use twitch_api::HelixClient;
use twitch_oauth2::{Scope, TwitchToken, UserToken};

const CLIENT_ID: &str = "uyf8apz7jdx3ujc3pboj58vim8c8a6";

lazy_static! {
    static ref default_scopes: Vec<Scope> = vec![
        Scope::UserReadChat,
        Scope::UserWriteChat,
        Scope::UserReadFollows,
        Scope::UserReadEmotes,
        Scope::UserReadBlockedUsers,
        Scope::UserReadSubscriptions,
    ];
}

#[derive(Clone)]
pub struct TokenManager {
    user_token: Arc<Mutex<Option<UserToken>>>,
    client: HelixClient<'static, reqwest::Client>,
    builder: Arc<Mutex<twitch_oauth2::DeviceUserTokenBuilder>>,
}

impl TokenManager {
    pub fn from_existing(token: UserToken, client: HelixClient<'static, reqwest::Client>) -> Self {
        let builder = twitch_oauth2::tokens::DeviceUserTokenBuilder::new(
            CLIENT_ID.to_string(),
            default_scopes.clone(),
        );

        TokenManager {
            user_token: Arc::new(Mutex::new(Some(token))),
            client,
            builder: Arc::new(Mutex::new(builder)),
        }
    }

    pub fn new(client: HelixClient<'static, reqwest::Client>) -> Self {
        let builder = twitch_oauth2::tokens::DeviceUserTokenBuilder::new(
            CLIENT_ID.to_string(),
            default_scopes.clone(),
        );
        TokenManager {
            user_token: Arc::new(Mutex::new(None)),
            client,
            builder: Arc::new(Mutex::new(builder)),
        }
    }

    pub async fn start_device_code_flow(&self) -> twitch_oauth2::id::DeviceCodeResponse {
        debug!("starting device flow");

        let mut build_guard = self.builder.lock().await;

        let result = build_guard.start(&self.client).await.unwrap();
        return result.clone();
    }

    pub async fn finish_device_code_flow(&self) -> twitch_oauth2::UserToken {
        debug!("finishing device flow");

        let mut build_guard = self.builder.lock().await;

        let token = build_guard
            .wait_for_code(&self.client, tokio::time::sleep)
            .await
            .unwrap();

        *self.user_token.lock().await = Some(token.clone());

        return token;
    }

    pub async fn is_token_valid(&self) -> bool {
        let token = self.user_token.lock().await;
        if let Some(token) = token.as_ref() {
            match token.validate_token(&self.client.clone()).await {
                Ok(_) => return true,
                Err(err) => {
                    error!("token validation failed: {}", err.to_string());
                    return false;
                }
            }
        }
        debug!("token isn't set");
        false
    }

    pub async fn get_token(&self) -> twitch_oauth2::UserToken {
        self.user_token
            .lock()
            .await
            .clone()
            .expect("token to be set")
    }

}

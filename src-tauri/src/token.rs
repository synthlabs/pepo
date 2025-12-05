use core::time;
use lazy_static::lazy_static;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use twitch_api::HelixClient;
use twitch_oauth2::{Scope, TwitchToken, UserToken};

fn default_refresh_callback(token: UserToken) {
    debug!("token refreshed: {:#?}", token);
}

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
    pub on_refresh: Arc<Box<dyn Fn(UserToken) + Send + Sync>>,
    user_token: Option<Arc<Mutex<UserToken>>>,
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
            on_refresh: Arc::new(Box::new(default_refresh_callback)),
            user_token: Some(Arc::new(Mutex::new(token))),
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
            user_token: None,
            on_refresh: Arc::new(Box::new(default_refresh_callback)),
            client,
            builder: Arc::new(Mutex::new(builder)),
        }
    }

    pub async fn start_device_code_flow(self) -> twitch_oauth2::id::DeviceCodeResponse {
        debug!("starting device flow");

        let mut build_guard = self.builder.lock().await;

        let result = build_guard.start(&self.client).await.unwrap();
        return result.clone();
    }

    pub async fn finish_device_code_flow(mut self) -> twitch_oauth2::UserToken {
        debug!("finishing device flow");

        let mut build_guard = self.builder.lock().await;

        let token = build_guard
            .wait_for_code(&self.client, tokio::time::sleep)
            .await
            .unwrap();

        self.user_token = Some(Arc::new(Mutex::new(token.clone())));

        return token;
    }

    pub async fn is_token_valid(self) -> bool {
        if let Some(token_guard) = self.user_token {
            let token = token_guard.lock().await;
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

    pub async fn get_token(self) -> twitch_oauth2::UserToken {
        let token_guard = self.user_token.expect("token to be set");
        let token = token_guard.lock().await;
        token.clone()
    }

    pub fn manage(self) {
        tauri::async_runtime::spawn(async move {
            let mut last_validation_tick = Instant::now();
            loop {
                if let Some(ref user_token) = self.user_token {
                    let mut user_token_guard = user_token.lock().await;
                    debug!(
                        "last_validation_tick: since={:?}",
                        last_validation_tick.elapsed()
                    );
                    if last_validation_tick.elapsed() > Duration::from_secs(300) {
                        info!("validating token");
                        let res = user_token_guard
                            .validate_token(&self.client.clone())
                            .await
                            .expect("failed to validate token");

                        debug!("validate: token={:?}", res);
                        last_validation_tick = Instant::now();
                    }

                    info!("token: expires_in={:?}", user_token_guard.expires_in());
                    if user_token_guard.expires_in() < std::time::Duration::from_secs(600) {
                        info!("refreshing token");
                        user_token_guard
                            .refresh_token(&self.client.clone())
                            .await
                            .expect("failed to refresh token");
                        (self.on_refresh)(user_token_guard.clone())
                    }
                }
                debug!("sleeping");
                tokio::time::sleep(time::Duration::from_secs(30)).await;
            }
        });
    }
}

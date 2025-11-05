use core::time;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tauri::async_runtime::block_on;
use tokio::sync::Mutex;
use tracing::{debug, info};
use twitch_api::HelixClient;
use twitch_oauth2::{Scope, TwitchToken, UserToken};

fn default_refresh_callback(token: UserToken) {
    debug!("token refreshed: {:#?}", token);
}

const CLIENT_ID: &str = "uyf8apz7jdx3ujc3pboj58vim8c8a6";

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
            vec![
                Scope::UserReadChat,
                Scope::UserWriteChat,
                Scope::UserReadFollows,
                Scope::UserReadEmotes,
                Scope::UserReadBlockedUsers,
                Scope::UserReadSubscriptions,
            ],
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
            vec![
                Scope::UserReadChat,
                Scope::UserWriteChat,
                Scope::UserReadFollows,
                Scope::UserReadEmotes,
                Scope::UserReadBlockedUsers,
                Scope::UserReadSubscriptions,
            ],
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

    // TODO: fix the this weird async/std bs
    pub fn manage(self) {
        std::thread::spawn(move || {
            let mut last_validation_tick = Instant::now();
            loop {
                if let Some(ref user_token) = self.user_token {
                    let mut user_token_guard = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(async { user_token.lock().await })
                    });
                    debug!(
                        "last_validation_tick: since={:?}",
                        last_validation_tick.elapsed()
                    );
                    if last_validation_tick.elapsed() > Duration::from_secs(300) {
                        info!("validating token");
                        let res = block_on(user_token_guard.validate_token(&self.client.clone()))
                            .expect("failed to validate token");

                        debug!("validate: token={:?}", res);
                        last_validation_tick = Instant::now();
                    }

                    info!("token: expires_in={:?}", user_token_guard.expires_in());
                    if user_token_guard.expires_in() < std::time::Duration::from_secs(600) {
                        info!("refreshing token");
                        block_on(user_token_guard.refresh_token(&self.client.clone()))
                            .expect("failed to refresh token");
                        (self.on_refresh)(user_token_guard.clone())
                    }
                }
                debug!("sleeping");
                std::thread::sleep(time::Duration::from_secs(30));
            }
        });
    }
}

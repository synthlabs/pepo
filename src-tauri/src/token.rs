use core::time;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tauri::async_runtime::block_on;
use tauri_plugin_opener::OpenerExt;
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
    user_token: Arc<Mutex<UserToken>>,
    client: HelixClient<'static, reqwest::Client>,
    _app_handle: Arc<tauri::AppHandle>,
}

impl TokenManager {
    pub fn from_existing(
        token: UserToken,
        client: HelixClient<'static, reqwest::Client>,
        app_handle: tauri::AppHandle,
    ) -> Self {
        TokenManager {
            user_token: Arc::new(Mutex::new(token)),
            on_refresh: Arc::new(Box::new(default_refresh_callback)),
            client,
            _app_handle: Arc::new(app_handle),
        }
    }

    pub async fn new(
        client: HelixClient<'static, reqwest::Client>,
        app_handle: tauri::AppHandle,
    ) -> Self {
        let token = Self::get_new_token(client.clone(), app_handle.clone()).await;
        Self::from_existing(token, client, app_handle)
    }

    async fn get_new_token(
        client: HelixClient<'static, reqwest::Client>,
        app_handle: tauri::AppHandle,
    ) -> twitch_oauth2::UserToken {
        // First we need to get a token, preferably you'd also store this information somewhere safe to reuse when restarting the application.
        // For now we'll just get a new token every time the application starts.
        // One way to store the token is to store the access_token and refresh_token in a file and load it when the application starts with
        // `twitch_oauth2::UserToken::from_existing`
        let mut builder = twitch_oauth2::tokens::DeviceUserTokenBuilder::new(
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

        debug!("made token builder");

        let code: &twitch_oauth2::id::DeviceCodeResponse = builder.start(&client).await.unwrap();

        // onDeviceCode

        info!("login {}", code.verification_uri);
        app_handle
            .opener()
            .open_url(code.verification_uri.clone(), None::<&str>)
            .unwrap();

        // onUrlOpened

        let token = builder
            .wait_for_code(&client, tokio::time::sleep)
            .await
            .unwrap();

        // onUserToken

        return token
    }

    pub fn manage(self) {
        std::thread::spawn(move || {
            let mut last_validation_tick = Instant::now();
            loop {
                if let Ok(mut user_token_guard) = self.user_token.lock() {
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

    pub fn twitch_token(self) -> UserToken {
        let user_token_guard = self.user_token.lock().unwrap();
        user_token_guard.clone()
    }
}

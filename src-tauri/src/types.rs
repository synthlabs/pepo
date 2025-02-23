use serde::{Deserialize, Serialize};
use specta::Type;
use twitch_api::{client::CompatError, HelixClient};
use twitch_oauth2::TwitchToken;

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct UserToken {
    /// The access token used to authenticate requests with
    pub access_token: String,
    pub client_id: String,
    /// Username of user associated with this token
    pub login: String,
    /// User ID of the user associated with this token
    pub user_id: String,
    /// The refresh token used to extend the life of this user token
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

impl UserToken {
    pub fn from_twitch_token(token: twitch_oauth2::UserToken) -> UserToken {
        UserToken {
            access_token: token.access_token.secret().to_string(),
            client_id: token.client_id().to_string(),
            login: token.login.to_string(),
            user_id: token.user_id.to_string(),
            refresh_token: Some(
                token
                    .refresh_token
                    .clone()
                    .unwrap_or("".into())
                    .secret()
                    .to_string(),
            ),
            expires_in: token.expires_in().as_secs(),
        }
    }

    pub async fn to_twitch_token(
        self,
        client: HelixClient<'static, reqwest::Client>,
    ) -> std::result::Result<
        twitch_oauth2::UserToken,
        twitch_oauth2::tokens::errors::ValidationError<CompatError<reqwest::Error>>,
    > {
        twitch_oauth2::UserToken::from_existing(
            &client,
            twitch_oauth2::AccessToken::from(self.access_token),
            twitch_oauth2::RefreshToken::from(self.refresh_token.unwrap_or("".to_owned())),
            None,
        )
        .await
    }
}

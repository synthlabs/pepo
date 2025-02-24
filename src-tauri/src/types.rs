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

impl From<twitch_oauth2::UserToken> for UserToken {
    fn from(item: twitch_oauth2::UserToken) -> Self {
        let item = item.clone();
        UserToken {
            access_token: item.access_token.secret().to_string(),
            client_id: item.client_id().to_string(),
            login: item.login.to_string(),
            user_id: item.user_id.to_string(),
            expires_in: item.expires_in().as_secs(),
            refresh_token: Some(
                item.refresh_token
                    .clone()
                    .unwrap_or("".into())
                    .secret()
                    .to_string(),
            ),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct Stream {
    /// ID of the game being played on the stream.
    pub game_id: String,
    /// Name of the game being played.
    pub game_name: String,
    /// Stream ID.
    pub id: String,
    /// Stream language.
    pub language: String,
    /// Indicates if the broadcaster has specified their channel contains mature content that may be inappropriate for younger audiences.
    pub is_mature: bool,
    /// UTC timestamp.
    pub started_at: String,
    pub tags: Vec<String>,
    /// Thumbnail URL of the stream. All image URLs have variable width and height. You can replace {width} and {height} with any values to get that size image
    pub thumbnail_url: String,
    /// Stream title.
    pub title: String,
    /// ID of the user who is streaming.
    pub user_id: String,
    /// Display name corresponding to user_id.
    pub user_name: String,
    /// Login of the user who is streaming.
    pub user_login: String,
    /// Number of viewers watching the stream at the time of the query.
    pub viewer_count: usize,
}

impl From<twitch_api::helix::streams::Stream> for Stream {
    fn from(item: twitch_api::helix::streams::Stream) -> Self {
        Stream {
            game_id: item.game_id.to_string(),
            game_name: item.game_name,
            id: item.id.to_string(),
            language: item.language,
            is_mature: item.is_mature,
            started_at: item.started_at.to_string(),
            tags: item.tags,
            thumbnail_url: item.thumbnail_url,
            title: item.title,
            user_id: item.user_id.to_string(),
            user_name: item.user_name.to_string(),
            user_login: item.user_login.to_string(),
            viewer_count: item.viewer_count,
        }
    }
}

// {
//     "id": "141981764",
//     "login": "twitchdev",
//     "display_name": "TwitchDev",
//     "type": "",
//     "broadcaster_type": "partner",
//     "description": "Supporting third-party developers building Twitch integrations from chatbots to game integrations.",
//     "profile_image_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/8a6381c7-d0c0-4576-b179-38bd5ce1d6af-profile_image-300x300.png",
//     "offline_image_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/3f13ab61-ec78-4fe6-8481-8682cb3b0ac2-channel_offline_image-1920x1080.png",
//     "view_count": 5980557,
//     "email": "not-real@email.com",
//     "created_at": "2016-12-14T20:32:28Z"
//   }

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct Broadcaster {
    /// An ID that uniquely identifies the broadcaster that this user is following.
    pub id: String,
    /// The broadcaster’s login name.
    pub login: String,
    /// The broadcaster’s display name.
    pub display_name: String,
    pub profile_image_url: String,
    pub offline_image_url: String,
    pub description: String,
    pub created_at: String,
}

impl From<twitch_api::helix::channels::FollowedBroadcaster> for Broadcaster {
    fn from(item: twitch_api::helix::channels::FollowedBroadcaster) -> Self {
        Broadcaster {
            id: item.broadcaster_id.to_string(),
            login: item.broadcaster_login.to_string(),
            display_name: item.broadcaster_name.to_string(),
            profile_image_url: "".to_string(),
            offline_image_url: "".to_string(),
            description: "".to_string(),
            created_at: "".to_string(),
        }
    }
}

impl From<twitch_api::helix::users::User> for Broadcaster {
    fn from(item: twitch_api::helix::users::User) -> Self {
        Broadcaster {
            id: item.id.to_string(),
            login: item.login.to_string(),
            display_name: item.display_name.to_string(),
            profile_image_url: item.profile_image_url.unwrap_or_default(),
            offline_image_url: item.offline_image_url.unwrap_or_default(),
            description: item.description.unwrap_or_default(),
            created_at: item.created_at.to_string(),
        }
    }
}

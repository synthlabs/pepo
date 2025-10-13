use serde::{Deserialize, Serialize};
use specta::Type;
use twitch_api::{client::CompatError, HelixClient};
use twitch_oauth2::TwitchToken;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AuthState {
    pub phase: AuthPhase,
    pub device_code: String,
    pub token: Option<UserToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AuthPhase {
    Unauthorized,
    WaitingForDeviceCode,
    WaitingForAuth,
    FailedAuth,
    Authorized,
}

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

#[derive(Clone, Deserialize, Serialize, Type)]
pub struct ChannelInfo {
    /// Twitch User ID of this channel owner
    pub broadcaster_id: String,
    /// Twitch User login of this channel owner
    pub broadcaster_login: String,
    /// Twitch user display name of this channel owner
    pub broadcaster_name: String,
    /// Current game ID being played on the channel
    pub game_id: String,
    /// Name of the game being played on the channel
    pub game_name: String,
    /// Language of the channel
    pub broadcaster_language: String,
    /// Title of the stream
    pub title: String,
    /// Description of the stream
    #[serde(default)]
    pub description: String,
    /// Stream delay in seconds
    ///
    /// # Notes
    ///
    /// This value may not be accurate, it'll only be accurate when the token belongs to the broadcaster and they are partnered.
    #[serde(default)]
    pub delay: i64,
    /// The tags applied to the channel.
    pub tags: Vec<String>,
    /// Boolean flag indicating if the channel has branded content.
    pub is_branded_content: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct Badge {
    /// An ID that identifies this set of chat badges. For example, Bits or Subscriber.
    pub set_id: String,
    /// An ID that identifies this version of the badge. The ID can be any value.
    /// For example, for Bits, the ID is the Bits tier level, but for World of Warcraft, it could be Alliance or Horde.
    pub id: String,
    /// Contains metadata related to the chat badges in the badges tag.
    /// Currently, this tag contains metadata only for subscriber badges, to indicate the number of months the user has been a subscriber.
    pub info: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ChannelMessageType {
    /// An Unknown Message Type
    Unknown,
    /// A regular text message
    Text,
    /// A highlighted message with channel points
    ChannelPointsHighlighted,
    /// A message sent with channel points during sub-only mode
    ChannelPointsSubOnly,
    /// A first message from a user
    UserIntro,
    /// A gigantified emote
    PowerUpsGigantifiedEmote,
    /// A message sent with effects
    PowerUpsMessageEffect,
}

impl From<twitch_api::eventsub::channel::chat::message::MessageType> for ChannelMessageType {
    fn from(item: twitch_api::eventsub::channel::chat::message::MessageType) -> Self {
        match item {
            twitch_api::eventsub::channel::chat::message::MessageType::Text => {
                ChannelMessageType::Text
            }
            twitch_api::eventsub::channel::chat::message::MessageType::ChannelPointsHighlighted => {
                ChannelMessageType::ChannelPointsHighlighted
            }
            twitch_api::eventsub::channel::chat::message::MessageType::ChannelPointsSubOnly => {
                ChannelMessageType::ChannelPointsSubOnly
            }
            twitch_api::eventsub::channel::chat::message::MessageType::UserIntro => {
                ChannelMessageType::UserIntro
            }
            twitch_api::eventsub::channel::chat::message::MessageType::PowerUpsGigantifiedEmote => {
                ChannelMessageType::PowerUpsGigantifiedEmote
            }
            twitch_api::eventsub::channel::chat::message::MessageType::PowerUpsMessageEffect => {
                ChannelMessageType::PowerUpsMessageEffect
            }
            _ => ChannelMessageType::Unknown,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct ChannelMessage {
    pub ts: String,
    pub payload: String,
    /// The user ID of the user that sent the message.
    pub chatter_user_id: String,
    /// The user name of the user that sent the message.
    pub chatter_user_name: String,
    /// A UUID that identifies the message.
    pub message_id: String,
    pub text: String,
    // pub fragments: Vec<Fragment>,
    /// The type of message.
    pub message_type: ChannelMessageType,
    /// List of chat badges.
    pub badges: Vec<Badge>,
    /// Metadata if this message is a cheer.
    // pub cheer: Option<Cheer>,
    /// The color of the user's name in the chat room.
    /// This is a hexadecimal RGB color code in the form, `#<RGB>`.
    /// This may be empty if it is never set.
    pub color: String,
    // Metadata if this message is a reply.
    // pub reply: Option<Reply>,
    /// A stable message int that can be used by the UI
    pub index: usize,
}

impl ChannelMessage {
    pub fn new(
        value: twitch_api::eventsub::channel::ChannelChatMessageV1Payload,
        ts: String,
    ) -> Self {
        let raw_msg = serde_json::to_string(&value).unwrap();
        ChannelMessage {
            ts: ts,
            payload: raw_msg,
            chatter_user_id: value.chatter_user_id.to_string(),
            chatter_user_name: value.chatter_user_name.to_string(),
            message_id: value.message_id.to_string(),
            text: value.message.text,
            message_type: value.message_type.into(),
            color: value.color.to_string(),
            index: 0,
            badges: value
                .badges
                .iter()
                .map(|v| Badge {
                    set_id: v.set_id.to_string(),
                    id: v.id.to_string(),
                    info: v.info.clone(),
                })
                .collect(),
        }
    }
}

impl From<twitch_api::helix::channels::ChannelInformation> for ChannelInfo {
    fn from(value: twitch_api::helix::channels::ChannelInformation) -> Self {
        ChannelInfo {
            broadcaster_id: value.broadcaster_id.into(),
            broadcaster_login: value.broadcaster_login.into(),
            broadcaster_name: value.broadcaster_name.into(),
            game_id: value.game_id.into(),
            game_name: value.game_name.into(),
            broadcaster_language: value.broadcaster_language.clone(),
            title: value.title.clone(),
            description: value.description.clone(),
            delay: value.delay,
            tags: value.tags.clone(),
            is_branded_content: value.is_branded_content,
        }
    }
}

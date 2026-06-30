use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use twitch_api::{client::CompatError, HelixClient};
use twitch_oauth2::TwitchToken;

use crate::{
    badgemanager::{Badge, BadgeManager},
    emote::{cache::EmoteCacheTrait, Emote},
    emotemanager::EmoteManager,
    message,
};

pub const APP_SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
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

impl Default for AuthPhase {
    fn default() -> Self {
        AuthPhase::Unauthorized
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub appearance: AppearanceSettings,
    pub layout: LayoutSettings,
    pub chat: ChatSettings,
    pub emotes: EmoteSettings,
    pub channel_cache: ChannelCacheSettings,
    pub auth: AuthSettings,
    pub eventsub: EventSubSettings,
    pub providers: ProviderSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 0,
            appearance: AppearanceSettings::default(),
            layout: LayoutSettings::default(),
            chat: ChatSettings::default(),
            emotes: EmoteSettings::default(),
            channel_cache: ChannelCacheSettings::default(),
            auth: AuthSettings::default(),
            eventsub: EventSubSettings::default(),
            providers: ProviderSettings::default(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.schema_version = APP_SETTINGS_SCHEMA_VERSION;
        self.chat = self.chat.normalized();
        self.emotes = self.emotes.normalized();
        self.channel_cache = self.channel_cache.normalized();
        self.auth = self.auth.normalized();
        self.eventsub = self.eventsub.normalized();
        self.providers = self.providers.normalized();
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct ChannelCacheSettings {
    pub recurring_poll_enabled: bool,
    pub poll_interval_secs: u64,
    pub error_log_throttle_enabled: bool,
    pub error_log_throttle_secs: u64,
    pub user_lookup_chunk_size: usize,
}

impl Default for ChannelCacheSettings {
    fn default() -> Self {
        Self {
            recurring_poll_enabled: true,
            poll_interval_secs: 60,
            error_log_throttle_enabled: true,
            error_log_throttle_secs: 300,
            user_lookup_chunk_size: 100,
        }
    }
}

impl ChannelCacheSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.poll_interval_secs =
            u64_or_default(self.poll_interval_secs, defaults.poll_interval_secs);
        self.error_log_throttle_secs = u64_or_default(
            self.error_log_throttle_secs,
            defaults.error_log_throttle_secs,
        );
        self.user_lookup_chunk_size =
            usize_or_default(self.user_lookup_chunk_size, defaults.user_lookup_chunk_size);
        self
    }

    pub fn poll_interval(self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    pub fn error_log_throttle(self) -> Duration {
        Duration::from_secs(self.error_log_throttle_secs)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct AuthSettings {
    pub login_activation_delay_ms: u64,
    pub refresh_supervisor_tick_secs: u64,
    pub validation_interval_secs: u64,
    pub refresh_if_remaining_lt_secs: u64,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            login_activation_delay_ms: 500,
            refresh_supervisor_tick_secs: 15,
            validation_interval_secs: 300,
            refresh_if_remaining_lt_secs: 600,
        }
    }
}

impl AuthSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.login_activation_delay_ms = u64_or_default(
            self.login_activation_delay_ms,
            defaults.login_activation_delay_ms,
        );
        self.refresh_supervisor_tick_secs = u64_or_default(
            self.refresh_supervisor_tick_secs,
            defaults.refresh_supervisor_tick_secs,
        );
        self.validation_interval_secs = u64_or_default(
            self.validation_interval_secs,
            defaults.validation_interval_secs,
        );
        self.refresh_if_remaining_lt_secs = u64_or_default(
            self.refresh_if_remaining_lt_secs,
            defaults.refresh_if_remaining_lt_secs,
        );
        self
    }

    pub fn login_activation_delay(self) -> Duration {
        Duration::from_millis(self.login_activation_delay_ms)
    }

    pub fn refresh_supervisor_tick(self) -> Duration {
        Duration::from_secs(self.refresh_supervisor_tick_secs)
    }

    pub fn validation_interval(self) -> Duration {
        Duration::from_secs(self.validation_interval_secs)
    }

    pub fn refresh_threshold(self) -> Duration {
        Duration::from_secs(self.refresh_if_remaining_lt_secs)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct EventSubSettings {
    pub socket_idle_timeout_secs: u64,
    pub retry_base_secs: u64,
    pub retry_max_secs: u64,
    pub debug_cost_watcher_enabled: bool,
    pub debug_cost_watcher_interval_secs: u64,
    pub repeated_log_throttle_enabled: bool,
    pub unparseable_warning_throttle_secs: u64,
    pub subscription_error_throttle_secs: u64,
}

impl Default for EventSubSettings {
    fn default() -> Self {
        Self {
            socket_idle_timeout_secs: 30,
            retry_base_secs: 5,
            retry_max_secs: 60,
            debug_cost_watcher_enabled: true,
            debug_cost_watcher_interval_secs: 30,
            repeated_log_throttle_enabled: true,
            unparseable_warning_throttle_secs: 60,
            subscription_error_throttle_secs: 300,
        }
    }
}

impl EventSubSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.socket_idle_timeout_secs = u64_or_default(
            self.socket_idle_timeout_secs,
            defaults.socket_idle_timeout_secs,
        );
        self.retry_base_secs = u64_or_default(self.retry_base_secs, defaults.retry_base_secs);
        self.retry_max_secs = u64_or_default(self.retry_max_secs, defaults.retry_max_secs);
        self.debug_cost_watcher_interval_secs = u64_or_default(
            self.debug_cost_watcher_interval_secs,
            defaults.debug_cost_watcher_interval_secs,
        );
        self.unparseable_warning_throttle_secs = u64_or_default(
            self.unparseable_warning_throttle_secs,
            defaults.unparseable_warning_throttle_secs,
        );
        self.subscription_error_throttle_secs = u64_or_default(
            self.subscription_error_throttle_secs,
            defaults.subscription_error_throttle_secs,
        );
        self
    }

    pub fn socket_idle_timeout(self) -> Duration {
        Duration::from_secs(self.socket_idle_timeout_secs)
    }

    pub fn retry_delay_secs(self, attempt: u32) -> u64 {
        let exponent = attempt.min(4);
        self.retry_base_secs
            .saturating_mul(2u64.pow(exponent))
            .min(self.retry_max_secs)
    }

    pub fn debug_cost_watcher_interval(self) -> Duration {
        Duration::from_secs(self.debug_cost_watcher_interval_secs)
    }

    pub fn unparseable_warning_throttle(self) -> Duration {
        Duration::from_secs(self.unparseable_warning_throttle_secs)
    }

    pub fn subscription_error_throttle(self) -> Duration {
        Duration::from_secs(self.subscription_error_throttle_secs)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct ProviderSettings {
    pub http_connect_timeout_secs: u64,
    pub http_request_timeout_secs: u64,
    pub metadata_retention_enabled: bool,
    pub metadata_retention_secs: u64,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            http_connect_timeout_secs: 5,
            http_request_timeout_secs: 15,
            metadata_retention_enabled: true,
            metadata_retention_secs: 30 * 24 * 60 * 60,
        }
    }
}

impl ProviderSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.http_connect_timeout_secs = u64_or_default(
            self.http_connect_timeout_secs,
            defaults.http_connect_timeout_secs,
        );
        self.http_request_timeout_secs = u64_or_default(
            self.http_request_timeout_secs,
            defaults.http_request_timeout_secs,
        );
        self.metadata_retention_secs = u64_or_default(
            self.metadata_retention_secs,
            defaults.metadata_retention_secs,
        );
        self
    }

    pub fn http_connect_timeout(self) -> Duration {
        Duration::from_secs(self.http_connect_timeout_secs)
    }

    pub fn http_request_timeout(self) -> Duration {
        Duration::from_secs(self.http_request_timeout_secs)
    }
}

fn u64_or_default(value: u64, fallback: u64) -> u64 {
    if value == 0 {
        fallback
    } else {
        value
    }
}

fn usize_or_default(value: usize, fallback: usize) -> usize {
    if value == 0 {
        fallback
    } else {
        value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: AppearanceTheme,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: AppearanceTheme::Dark,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceTheme {
    System,
    Light,
    Dark,
}

impl Default for AppearanceTheme {
    fn default() -> Self {
        AppearanceTheme::Dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct LayoutSettings {
    pub sidebar_open: bool,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self { sidebar_open: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct ChatSettings {
    pub message_limit: usize,
    pub autoscroll_threshold_px: u32,
    pub show_timestamps: bool,
    pub timestamp_locale: String,
    pub timestamp_style: TimestampStyle,
    pub show_badges: bool,
    pub show_emotes: bool,
    pub alternate_backgrounds: bool,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            message_limit: 500,
            autoscroll_threshold_px: 32,
            show_timestamps: true,
            timestamp_locale: "en".to_string(),
            timestamp_style: TimestampStyle::Short,
            show_badges: true,
            show_emotes: true,
            alternate_backgrounds: true,
        }
    }
}

impl ChatSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        if self.message_limit == 0 {
            self.message_limit = defaults.message_limit;
        }
        if self.autoscroll_threshold_px == 0 {
            self.autoscroll_threshold_px = defaults.autoscroll_threshold_px;
        }
        if self.timestamp_locale.trim().is_empty() {
            self.timestamp_locale = defaults.timestamp_locale;
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TimestampStyle {
    Short,
    Medium,
    Long,
    Full,
}

impl Default for TimestampStyle {
    fn default() -> Self {
        TimestampStyle::Short
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct EmoteSettings {
    pub providers: Vec<EmoteProviderPreference>,
    pub autocomplete_enabled: bool,
    pub autocomplete_min_chars: usize,
    pub search_debounce_ms: u64,
    pub autocomplete_result_limit: usize,
    pub picker_result_limit: usize,
    pub picker_columns: usize,
    pub picker_max_height_px: u32,
    pub inline_emote_px: u32,
    pub inline_badge_px: u32,
}

impl Default for EmoteSettings {
    fn default() -> Self {
        Self {
            providers: EmoteProviderId::default_preferences(),
            autocomplete_enabled: true,
            autocomplete_min_chars: 2,
            search_debounce_ms: 75,
            autocomplete_result_limit: 25,
            picker_result_limit: 50,
            picker_columns: 8,
            picker_max_height_px: 192,
            inline_emote_px: 28,
            inline_badge_px: 20,
        }
    }
}

impl EmoteSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        let mut providers = Vec::new();

        for preference in self.providers {
            if !providers
                .iter()
                .any(|p: &EmoteProviderPreference| p.id == preference.id)
            {
                providers.push(preference);
            }
        }

        for preference in EmoteProviderId::default_preferences() {
            if !providers.iter().any(|p| p.id == preference.id) {
                providers.push(preference);
            }
        }

        self.providers = providers;

        if self.autocomplete_min_chars == 0 {
            self.autocomplete_min_chars = defaults.autocomplete_min_chars;
        }
        if self.search_debounce_ms == 0 {
            self.search_debounce_ms = defaults.search_debounce_ms;
        }
        if self.autocomplete_result_limit == 0 {
            self.autocomplete_result_limit = defaults.autocomplete_result_limit;
        }
        if self.picker_result_limit == 0 {
            self.picker_result_limit = defaults.picker_result_limit;
        }
        if self.picker_columns == 0 {
            self.picker_columns = defaults.picker_columns;
        }
        if self.picker_max_height_px == 0 {
            self.picker_max_height_px = defaults.picker_max_height_px;
        }
        if self.inline_emote_px == 0 {
            self.inline_emote_px = defaults.inline_emote_px;
        }
        if self.inline_badge_px == 0 {
            self.inline_badge_px = defaults.inline_badge_px;
        }

        self
    }

    pub fn provider_enabled(&self, id: EmoteProviderId) -> bool {
        self.providers
            .iter()
            .find(|preference| preference.id == id)
            .map(|preference| preference.enabled)
            .unwrap_or(true)
    }

    pub fn enabled_provider_ids_ordered(&self) -> Vec<EmoteProviderId> {
        self.providers
            .iter()
            .filter(|preference| preference.enabled)
            .map(|preference| preference.id)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct EmoteProviderPreference {
    pub id: EmoteProviderId,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EmoteProviderId {
    Twitch,
    Bttv,
    Ffz,
    Seventv,
}

impl EmoteProviderId {
    pub fn default_preferences() -> Vec<EmoteProviderPreference> {
        vec![
            EmoteProviderPreference {
                id: EmoteProviderId::Twitch,
                enabled: true,
            },
            EmoteProviderPreference {
                id: EmoteProviderId::Bttv,
                enabled: true,
            },
            EmoteProviderPreference {
                id: EmoteProviderId::Ffz,
                enabled: true,
            },
            EmoteProviderPreference {
                id: EmoteProviderId::Seventv,
                enabled: true,
            },
        ]
    }

    pub fn provider_name(self) -> &'static str {
        match self {
            EmoteProviderId::Twitch => "TwitchProvider",
            EmoteProviderId::Bttv => "BttvProvider",
            EmoteProviderId::Ffz => "FfzProvider",
            EmoteProviderId::Seventv => "SeventvProvider",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Type)]
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
    #[serde(default)]
    pub profile_image_url: String,
}

impl core::fmt::Debug for UserToken {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            UserToken {
                access_token: _,
                client_id,
                login,
                user_id,
                refresh_token: _,
                expires_in,
                profile_image_url,
            } => f
                .debug_struct("UserToken")
                .field("access_token", &"********")
                .field("client_id", &client_id)
                .field("login", &login)
                .field("user_id", &user_id)
                .field("refresh_token", &"********")
                .field("expires_in", &expires_in)
                .field("profile_image_url", &profile_image_url)
                .finish(),
        }
    }
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
            profile_image_url: String::new(),
        }
    }

    pub async fn to_twitch_token(
        self,
        client: HelixClient<'static, reqwest::Client>,
    ) -> std::result::Result<
        twitch_oauth2::UserToken,
        twitch_oauth2::tokens::errors::RetrieveTokenError<CompatError<reqwest::Error>>,
    > {
        let access_token = twitch_oauth2::AccessToken::from(self.access_token);
        match self.refresh_token.filter(|token| !token.is_empty()) {
            Some(refresh_token) => {
                twitch_oauth2::UserToken::from_existing_or_refresh_token(
                    &client,
                    access_token,
                    twitch_oauth2::RefreshToken::from(refresh_token),
                    twitch_oauth2::ClientId::from(self.client_id),
                    None,
                )
                .await
            }
            None => twitch_oauth2::UserToken::from_existing(&client, access_token, None, None)
                .await
                .map_err(Into::into),
        }
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
            profile_image_url: String::new(),
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

#[derive(Clone, Debug, Serialize, Deserialize, Type, Default)]
pub struct ChannelCache {
    pub channels: HashMap<String, ChannelStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, Default)]
pub struct ChannelStatus {
    pub broadcaster_id: String,
    pub login: String,
    pub display_name: String,
    pub profile_image_url: String,
    pub is_live: bool,
    pub stream: Option<Stream>,
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
    /// Profile image URL of the broadcaster
    #[serde(default)]
    pub profile_image_url: String,
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
            profile_image_url: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct BadgeRef {
    /// An ID that identifies this set of chat badges. For example, Bits or Subscriber.
    pub set_id: String,
    /// An ID that identifies this version of the badge. The ID can be any value.
    /// For example, for Bits, the ID is the Bits tier level, but for World of Warcraft, it could be Alliance or Horde.
    pub id: String,
    /// Contains metadata related to the chat badges in the badges tag.
    /// Currently, this tag contains metadata only for subscriber badges, to indicate the number of months the user has been a subscriber.
    pub info: String,
    /// The info for displaying the badge
    pub badge: Badge,
}

static INDEX_COUNTER: AtomicU64 = AtomicU64::new(0);

macro_rules! next_index {
    () => {
        INDEX_COUNTER.fetch_add(1, Ordering::Relaxed)
    };
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
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

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type, PartialEq, Eq)]
pub struct ChannelMessageTranslation {
    pub source_language: String,
    pub target_language: String,
    pub translated_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type, PartialEq, Eq)]
pub struct ChannelMessageTranslationUpdate {
    pub message_id: String,
    pub translation: ChannelMessageTranslation,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct ChannelMessage {
    pub ts: String,
    // pub payload: String,
    /// The broadcaster user ID.
    pub broadcaster_user_id: String,
    /// The broadcaster display name.
    pub broadcaster_user_name: String,
    /// The broadcaster login.
    pub broadcaster_user_login: String,
    /// The user ID of the user that sent the message.
    pub chatter_user_id: String,
    /// The user name of the user that sent the message.
    pub chatter_user_name: String,
    /// A UUID that identifies the message.
    pub message_id: String,
    pub text: String,
    /// The parsed fragments of the text field for rendering
    pub fragments: Vec<message::Fragment>,
    /// The type of message.
    pub message_type: ChannelMessageType,
    /// List of chat badges.
    pub badges: Vec<BadgeRef>,
    /// Metadata if this message is a cheer.
    // pub cheer: Option<Cheer>,
    /// The color of the user's name in the chat room.
    /// This is a hexadecimal RGB color code in the form, `#<RGB>`.
    /// This may be empty if it is never set.
    pub color: String,
    // Metadata if this message is a reply.
    // pub reply: Option<Reply>,
    /// Translated message text when available.
    pub translation: Option<ChannelMessageTranslation>,
    /// A stable message int that can be used by the UI
    pub index: u64,
}

impl ChannelMessage {
    pub fn new(
        value: twitch_api::eventsub::channel::ChannelChatMessageV1Payload,
        ts: String,
        bm: BadgeManager,
        em: EmoteManager,
        emote_settings: EmoteSettings,
        app_handle: tauri::AppHandle,
    ) -> Self {
        let bm_ref = bm.clone();
        let emote_settings = emote_settings.normalized();
        let broadcaster_id = value.broadcaster_user_id.to_string();
        let broadcaster_login = value.broadcaster_user_login.to_string();
        let message_id = value.message_id.to_string();
        let message_text = value.message.text.clone();
        let emote_cache = em.get_emote_cache(broadcaster_id.clone(), &emote_settings);

        if emote_settings.provider_enabled(EmoteProviderId::Twitch) {
            let _: Vec<_> = value
                .message
                .fragments
                .iter()
                .map(|f| {
                    if let twitch_api::eventsub::channel::chat::Fragment::Emote { text, emote } = f
                    {
                        if !emote_cache.has_emote(text.to_string()) {
                            let scope = em
                                .resolve_user_name(&emote.owner_id.to_string())
                                .unwrap_or_else(|| "Channel".to_string());
                            em.insert_twitch_fragment_emote(
                                broadcaster_id.clone(),
                                text.to_string(),
                                Emote::from_emote_fragment(text.to_string(), emote, scope),
                                &emote_settings,
                            );
                        }
                    }
                })
                .collect();
        }

        let fragments = message::Parser::parse(message_text.clone(), &emote_cache);
        let translation_text = translation_input_from_fragments(&fragments);
        crate::internal::detect_language(
            app_handle,
            &broadcaster_login,
            &message_id,
            translation_text.as_deref().unwrap_or(""),
        );

        ChannelMessage {
            ts: ts,
            broadcaster_user_id: value.broadcaster_user_id.to_string(),
            broadcaster_user_name: value.broadcaster_user_name.to_string(),
            broadcaster_user_login: broadcaster_login,
            chatter_user_id: value.chatter_user_id.to_string(),
            chatter_user_name: value.chatter_user_name.to_string(),
            message_id,
            text: message_text.clone(),
            message_type: value.message_type.into(),
            color: value.color.to_string(),
            translation: None,
            index: next_index!(),
            badges: value
                .badges
                .iter()
                .map(|v| {
                    let bm_ref = bm_ref.clone();
                    let badge = match tauri::async_runtime::block_on(
                        bm_ref.get(v.set_id.to_string(), value.broadcaster_user_id.to_string()),
                    ) {
                        Some(b_set) => b_set.version(v.id.to_string()),
                        None => None,
                    };

                    badge_ref_or_fallback(
                        v.set_id.to_string(),
                        v.id.to_string(),
                        v.info.clone(),
                        badge,
                    )
                })
                .collect(),
            fragments,
        }
    }
}

fn badge_ref_or_fallback(
    set_id: String,
    id: String,
    info: String,
    badge: Option<Badge>,
) -> BadgeRef {
    BadgeRef {
        set_id,
        id,
        info,
        badge: badge.unwrap_or_default(),
    }
}

fn translation_input_from_fragments(fragments: &[message::Fragment]) -> Option<String> {
    let text = fragments
        .iter()
        .filter_map(|fragment| match fragment {
            message::Fragment::Text(fragment) => Some(fragment.text.as_str()),
            message::Fragment::Emote(_) | message::Fragment::Cheer(_) => None,
        })
        .collect::<String>();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_input_uses_text_fragments_only() {
        let input = translation_input_from_fragments(&[
            message::Fragment::Text(message::TextFragment {
                index: 0,
                text: "ты видел ".to_owned(),
            }),
            message::Fragment::Emote(message::EmoteFragment {
                index: 1,
                emote: Emote {
                    name: "LUL".to_owned(),
                    ..Default::default()
                },
            }),
            message::Fragment::Text(message::TextFragment {
                index: 2,
                text: " как он нюхал".to_owned(),
            }),
        ]);

        assert_eq!(input.as_deref(), Some("ты видел как он нюхал"));
    }

    #[test]
    fn translation_input_skips_emote_only_fragments() {
        let input =
            translation_input_from_fragments(&[message::Fragment::Emote(message::EmoteFragment {
                index: 0,
                emote: Emote {
                    name: "LUL".to_owned(),
                    ..Default::default()
                },
            })]);

        assert_eq!(input, None);
    }

    #[test]
    fn badge_ref_uses_empty_badge_when_metadata_is_pending() {
        let badge_ref = badge_ref_or_fallback(
            "subscriber".to_owned(),
            "12".to_owned(),
            "12".to_owned(),
            None,
        );

        assert_eq!(badge_ref.set_id, "subscriber");
        assert_eq!(badge_ref.id, "12");
        assert_eq!(badge_ref.info, "12");
        assert!(badge_ref.badge.set_id.is_empty());
        assert!(badge_ref.badge.id.is_empty());
        assert!(badge_ref.badge.image_url_4x.is_empty());
    }

    #[test]
    fn badge_ref_preserves_loaded_badge_metadata() {
        let badge_ref = badge_ref_or_fallback(
            "subscriber".to_owned(),
            "12".to_owned(),
            "12".to_owned(),
            Some(Badge {
                set_id: "subscriber".to_owned(),
                id: "12".to_owned(),
                image_url_4x: "https://example.com/sub.png".to_owned(),
                title: "Subscriber".to_owned(),
                ..Default::default()
            }),
        );

        assert_eq!(badge_ref.set_id, "subscriber");
        assert_eq!(badge_ref.id, "12");
        assert_eq!(badge_ref.badge.set_id, "subscriber");
        assert_eq!(badge_ref.badge.id, "12");
        assert_eq!(badge_ref.badge.image_url_4x, "https://example.com/sub.png");
        assert_eq!(badge_ref.badge.title, "Subscriber");
    }

    #[test]
    fn default_app_settings_preserve_current_behavior() {
        let settings = AppSettings::default().normalized();

        assert_eq!(settings.schema_version, APP_SETTINGS_SCHEMA_VERSION);
        assert!(matches!(settings.appearance.theme, AppearanceTheme::Dark));
        assert!(settings.layout.sidebar_open);
        assert_eq!(settings.chat.message_limit, 500);
        assert_eq!(settings.chat.autoscroll_threshold_px, 32);
        assert_eq!(settings.emotes.autocomplete_min_chars, 2);
        assert_eq!(settings.emotes.autocomplete_result_limit, 25);
        assert_eq!(settings.emotes.picker_result_limit, 50);
        assert_eq!(
            settings.emotes.enabled_provider_ids_ordered(),
            vec![
                EmoteProviderId::Twitch,
                EmoteProviderId::Bttv,
                EmoteProviderId::Ffz,
                EmoteProviderId::Seventv
            ]
        );
        assert!(settings.channel_cache.recurring_poll_enabled);
        assert_eq!(settings.channel_cache.poll_interval_secs, 60);
        assert!(settings.channel_cache.error_log_throttle_enabled);
        assert_eq!(settings.channel_cache.error_log_throttle_secs, 300);
        assert_eq!(settings.channel_cache.user_lookup_chunk_size, 100);
        assert_eq!(settings.auth.login_activation_delay_ms, 500);
        assert_eq!(settings.auth.refresh_supervisor_tick_secs, 15);
        assert_eq!(settings.auth.validation_interval_secs, 300);
        assert_eq!(settings.auth.refresh_if_remaining_lt_secs, 600);
        assert_eq!(settings.eventsub.socket_idle_timeout_secs, 30);
        assert_eq!(settings.eventsub.retry_base_secs, 5);
        assert_eq!(settings.eventsub.retry_max_secs, 60);
        assert!(settings.eventsub.debug_cost_watcher_enabled);
        assert_eq!(settings.eventsub.debug_cost_watcher_interval_secs, 30);
        assert!(settings.eventsub.repeated_log_throttle_enabled);
        assert_eq!(settings.eventsub.unparseable_warning_throttle_secs, 60);
        assert_eq!(settings.eventsub.subscription_error_throttle_secs, 300);
        assert_eq!(settings.providers.http_connect_timeout_secs, 5);
        assert_eq!(settings.providers.http_request_timeout_secs, 15);
        assert!(settings.providers.metadata_retention_enabled);
        assert_eq!(
            settings.providers.metadata_retention_secs,
            30 * 24 * 60 * 60
        );
    }

    #[test]
    fn emote_provider_normalization_dedupes_and_appends_missing_defaults() {
        let settings = EmoteSettings {
            providers: vec![
                EmoteProviderPreference {
                    id: EmoteProviderId::Bttv,
                    enabled: false,
                },
                EmoteProviderPreference {
                    id: EmoteProviderId::Twitch,
                    enabled: true,
                },
                EmoteProviderPreference {
                    id: EmoteProviderId::Bttv,
                    enabled: true,
                },
            ],
            ..Default::default()
        }
        .normalized();

        assert_eq!(
            settings.providers,
            vec![
                EmoteProviderPreference {
                    id: EmoteProviderId::Bttv,
                    enabled: false,
                },
                EmoteProviderPreference {
                    id: EmoteProviderId::Twitch,
                    enabled: true,
                },
                EmoteProviderPreference {
                    id: EmoteProviderId::Ffz,
                    enabled: true,
                },
                EmoteProviderPreference {
                    id: EmoteProviderId::Seventv,
                    enabled: true,
                },
            ]
        );
        assert_eq!(
            settings.enabled_provider_ids_ordered(),
            vec![
                EmoteProviderId::Twitch,
                EmoteProviderId::Ffz,
                EmoteProviderId::Seventv
            ]
        );
    }

    #[test]
    fn all_disabled_providers_stay_disabled() {
        let settings = EmoteSettings {
            providers: EmoteProviderId::default_preferences()
                .into_iter()
                .map(|mut preference| {
                    preference.enabled = false;
                    preference
                })
                .collect(),
            ..Default::default()
        }
        .normalized();

        assert!(settings.enabled_provider_ids_ordered().is_empty());
        assert!(!settings.provider_enabled(EmoteProviderId::Twitch));
    }

    #[test]
    fn app_settings_deserialize_missing_fields_from_defaults() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        let settings = settings.normalized();

        assert_eq!(settings.chat.message_limit, 500);
        assert!(settings.chat.show_timestamps);
        assert_eq!(settings.emotes.providers.len(), 4);
        assert_eq!(settings.channel_cache.poll_interval_secs, 60);
        assert_eq!(settings.auth.refresh_if_remaining_lt_secs, 600);
        assert_eq!(settings.eventsub.retry_max_secs, 60);
        assert_eq!(
            settings.providers.metadata_retention_secs,
            30 * 24 * 60 * 60
        );
    }

    #[test]
    fn app_settings_zero_numbers_fallback_without_changing_boolean_toggles() {
        let settings = AppSettings {
            channel_cache: ChannelCacheSettings {
                recurring_poll_enabled: false,
                poll_interval_secs: 0,
                error_log_throttle_enabled: false,
                error_log_throttle_secs: 0,
                user_lookup_chunk_size: 0,
            },
            auth: AuthSettings {
                login_activation_delay_ms: 0,
                refresh_supervisor_tick_secs: 0,
                validation_interval_secs: 0,
                refresh_if_remaining_lt_secs: 0,
            },
            eventsub: EventSubSettings {
                socket_idle_timeout_secs: 0,
                retry_base_secs: 0,
                retry_max_secs: 0,
                debug_cost_watcher_enabled: false,
                debug_cost_watcher_interval_secs: 0,
                repeated_log_throttle_enabled: false,
                unparseable_warning_throttle_secs: 0,
                subscription_error_throttle_secs: 0,
            },
            providers: ProviderSettings {
                http_connect_timeout_secs: 0,
                http_request_timeout_secs: 0,
                metadata_retention_enabled: false,
                metadata_retention_secs: 0,
            },
            ..Default::default()
        }
        .normalized();

        assert!(!settings.channel_cache.recurring_poll_enabled);
        assert_eq!(settings.channel_cache.poll_interval_secs, 60);
        assert!(!settings.channel_cache.error_log_throttle_enabled);
        assert_eq!(settings.channel_cache.error_log_throttle_secs, 300);
        assert_eq!(settings.channel_cache.user_lookup_chunk_size, 100);
        assert_eq!(settings.auth.login_activation_delay_ms, 500);
        assert_eq!(settings.auth.refresh_supervisor_tick_secs, 15);
        assert_eq!(settings.auth.validation_interval_secs, 300);
        assert_eq!(settings.auth.refresh_if_remaining_lt_secs, 600);
        assert_eq!(settings.eventsub.socket_idle_timeout_secs, 30);
        assert_eq!(settings.eventsub.retry_base_secs, 5);
        assert_eq!(settings.eventsub.retry_max_secs, 60);
        assert!(!settings.eventsub.debug_cost_watcher_enabled);
        assert_eq!(settings.eventsub.debug_cost_watcher_interval_secs, 30);
        assert!(!settings.eventsub.repeated_log_throttle_enabled);
        assert_eq!(settings.eventsub.unparseable_warning_throttle_secs, 60);
        assert_eq!(settings.eventsub.subscription_error_throttle_secs, 300);
        assert_eq!(settings.providers.http_connect_timeout_secs, 5);
        assert_eq!(settings.providers.http_request_timeout_secs, 15);
        assert!(!settings.providers.metadata_retention_enabled);
        assert_eq!(
            settings.providers.metadata_retention_secs,
            30 * 24 * 60 * 60
        );
    }
}

use serde::{Deserialize, Serialize};

pub mod cache;
pub mod providers;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default, PartialEq)]
pub struct Emote {
    /// ID of the emote.
    pub id: String,
    /// Name of the emote a viewer types into Twitch chat for the image to appear.
    pub name: String,
    pub tier: String,
    /// If the emote_type is "subscriptions", this indicates the subscriber tier at which the emote is unlocked. Set to an empty string otherwise.
    // FIXME: Enumify?
    /// The type of emote.
    ///
    /// The most common values for custom channel emotes are
    ///
    /// `subscriptions`: Indicates a custom subscriber emote.
    ///
    /// `bitstier`: Indicates a custom Bits tier emote.
    ///
    /// `follower`: Indicates a custom follower emote.
    pub emote_type: String,
    /// ID of the emote set the emote belongs to.
    pub emote_set_id: String,
    /// The formats that the emote is available in.
    pub format: Vec<String>,
    /// The sizes that the emote is available in.
    pub scale: Vec<String>,
    /// The background themes that the emote is available in.
    pub theme_mode: Vec<String>,
    /// Fully constructed URL for the emote image.
    pub url: String,
    /// The provider this emote comes from (e.g. "Twitch", "BTTV").
    pub provider: String,
    /// The scope of the emote (e.g. "Global", "Channel").
    pub scope: String,
}

impl Emote {
    pub fn from_emote_fragment(
        name: String,
        value: &twitch_api::eventsub::channel::chat::Emote,
        scope: String,
    ) -> Self {
        let id = value.id.to_string();
        let format: Vec<String> = value.format.iter().map(|v| v.to_string()).collect();
        let fmt = format.last().cloned().unwrap_or_else(|| "static".to_string());
        let url = format!(
            "https://static-cdn.jtvnw.net/emoticons/v2/{}/{}/dark/3.0",
            id, fmt
        );
        Emote {
            id,
            name: name.clone(),
            emote_set_id: value.emote_set_id.to_string(),
            format,
            theme_mode: vec!["light".to_string(), "dark".to_string()],
            scale: vec!["1.0".to_string(), "2.0".to_string(), "3.0".to_string()],
            url,
            provider: "Twitch".to_string(),
            scope,
            ..Default::default()
        }
    }
}

impl From<&twitch_api::helix::chat::GlobalEmote> for Emote {
    fn from(value: &twitch_api::helix::chat::GlobalEmote) -> Self {
        let id = value.id.to_string();
        let format: Vec<String> = value.format.iter().map(|v| v.to_string()).collect();
        let scale: Vec<String> = value.scale.iter().map(|v| v.to_string()).collect();
        let theme_mode: Vec<String> = value.theme_mode.iter().map(|v| v.to_string()).collect();
        let fmt = format.last().cloned().unwrap_or_else(|| "static".to_string());
        let tm = theme_mode.last().cloned().unwrap_or_else(|| "dark".to_string());
        let sc = scale.last().cloned().unwrap_or_else(|| "1.0".to_string());
        let url = format!(
            "https://static-cdn.jtvnw.net/emoticons/v2/{}/{}/{}/{}",
            id, fmt, tm, sc
        );
        Emote {
            id,
            name: value.name.clone(),
            emote_set_id: providers::GLOBAL_SCOPE_KEY.to_owned(),
            format,
            scale,
            theme_mode,
            url,
            provider: "Twitch".to_string(),
            scope: "Global".to_string(),
            ..Default::default()
        }
    }
}

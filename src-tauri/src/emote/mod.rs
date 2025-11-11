use serde::{Deserialize, Serialize};

pub mod cache;
pub mod providers;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
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
}

impl From<&twitch_api::helix::chat::GlobalEmote> for Emote {
    fn from(value: &twitch_api::helix::chat::GlobalEmote) -> Self {
        Emote {
            id: value.id.to_string(),
            name: value.name.clone(),
            tier: "".to_string(),
            emote_type: "".to_string(),
            emote_set_id: providers::GLOBAL_SCOPE_KEY.to_owned(),
            format: value.format.iter().map(|v| v.to_string()).collect(),
            scale: value.scale.iter().map(|v| v.to_string()).collect(),
            theme_mode: value.theme_mode.iter().map(|v| v.to_string()).collect(),
        }
    }
}

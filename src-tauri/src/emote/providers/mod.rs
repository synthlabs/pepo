use crate::emote::{cache::EmoteCacheTrait, Emote};
use crate::types::{EmoteProviderId, ProviderSettings};

pub mod bttv;
pub mod ffz;
pub mod http;
pub mod seventv;
pub mod twitch;

pub const GLOBAL_SCOPE_KEY: &str = "_global";

pub trait EmoteProvider<T: EmoteCacheTrait>: Send + Sync {
    fn get_id(&self) -> EmoteProviderId;
    fn get_name(&self) -> String {
        self.get_id().provider_name().to_string()
    }
    fn hydrate_cache(&self, _scope_key: &str, _provider_settings: &ProviderSettings) -> bool {
        false
    }
    fn load_global_emotes(&self, client: &reqwest::Client, provider_settings: &ProviderSettings);
    fn load_channel_emotes(
        &self,
        broadcaster_id: String,
        client: &reqwest::Client,
        provider_settings: &ProviderSettings,
    );
    fn load_user_emotes(&self) {}
    fn insert_emote(&self, _scope: String, _name: String, _emote: Emote) {}
    fn get_emote_cache(&self, scope: String) -> T;
}

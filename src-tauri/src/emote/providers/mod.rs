use crate::emote::{cache::EmoteCacheTrait, Emote};
use crate::types::EmoteProviderId;

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
    fn load_global_emotes(&self, client: &reqwest::Client);
    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client);
    fn load_user_emotes(&self) {}
    fn insert_emote(&self, _scope: String, _name: String, _emote: Emote) {}
    fn get_emote_cache(&self, scope: String) -> T;
}

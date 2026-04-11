use crate::emote::cache::EmoteCacheTrait;

pub mod bttv;
pub mod twitch;

pub const GLOBAL_SCOPE_KEY: &str = "_global";

pub trait EmoteProvider<T: EmoteCacheTrait> {
    fn get_name(&self) -> String;
    fn load_global_emotes(&self, client: &reqwest::Client);
    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client);
    fn get_emote_cache(&self, scope: String) -> T;
}

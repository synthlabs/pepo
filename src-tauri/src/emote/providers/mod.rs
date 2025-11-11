use crate::emote::cache::EmoteCacheTrait;

pub mod twitch;

pub const GLOBAL_SCOPE_KEY: &str = "_global";

pub trait EmoteProvider<T: EmoteCacheTrait> {
    fn get_name(&self) -> String;
    fn load_global_emotes(
        &self,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    );
    fn load_channel_emotes(
        &self,
        broadcaster_id: String,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    );
    fn get_emote_cache(&self, scope: String) -> T;
    // fn get_emote(&self, scope: String, name: String) -> Option<Emote>;
}

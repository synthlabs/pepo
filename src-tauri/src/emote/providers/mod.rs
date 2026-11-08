use crate::emote::{cache::EmoteCacheTrait, Emote};
use crate::types::{EmoteProviderId, ProviderSettings};

pub mod bttv;
pub mod ffz;
pub mod http;
pub mod seventv;
pub mod twitch;

#[cfg(test)]
mod tests;

pub const GLOBAL_SCOPE_KEY: &str = "_global";

pub trait EmoteProvider<T: EmoteCacheTrait>: Send + Sync {
    fn refresh_global<'a>(
        &'a self,
        client: &'a reqwest::Client,
        settings: &'a ProviderSettings,
    ) -> futures::future::BoxFuture<'a, Result<(), crate::network::Failure>> {
        Box::pin(async move {
            self.load_global_emotes(client, settings);
            Ok(())
        })
    }
    fn refresh_channel<'a>(
        &'a self,
        scope: String,
        client: &'a reqwest::Client,
        settings: &'a ProviderSettings,
    ) -> futures::future::BoxFuture<'a, Result<(), crate::network::Failure>> {
        Box::pin(async move {
            self.load_channel_emotes(scope, client, settings);
            Ok(())
        })
    }
    fn refresh_user<'a>(
        &'a self,
        _settings: &'a ProviderSettings,
    ) -> futures::future::BoxFuture<'a, Result<(), crate::network::Failure>> {
        Box::pin(async move {
            self.load_user_emotes();
            Ok(())
        })
    }
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

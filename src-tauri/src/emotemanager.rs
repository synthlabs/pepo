use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCacheTrait, MultiCache},
    providers::{
        bttv::BttvProvider, ffz::FfzProvider, seventv::SeventvProvider, twitch::TwitchProvider,
        EmoteProvider,
    },
    Emote,
};
use crate::types::{EmoteProviderId, EmoteSettings};
use crate::SharedTwitchToken;

// TODO: switch to a RWLock instead of Mutex
type SharedVec<V> = Arc<Mutex<Vec<V>>>;
type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    pub providers: SharedVec<Box<dyn EmoteProvider<MultiCache> + Send>>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: SharedTwitchToken,
    name_cache: SharedMap<String>,
}

impl EmoteManager {
    pub fn empty(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: SharedTwitchToken,
    ) -> EmoteManager {
        EmoteManager {
            providers: Arc::new(Mutex::new(Vec::new())),
            client,
            token,
            name_cache: Default::default(),
        }
    }

    pub fn load_global(&self, emote_settings: &EmoteSettings) {
        let http_client = reqwest::Client::new();

        let providers: Vec<Box<dyn EmoteProvider<MultiCache> + Send>> = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered()
            .into_iter()
            .map(|id| self.provider(id))
            .collect();

        for p in &providers {
            p.load_global_emotes(&http_client);
        }

        let mut store = self.providers.lock().unwrap();
        *store = providers;
    }

    pub async fn load_channel(self, broadcaster_id: String, emote_settings: &EmoteSettings) {
        let http_client = reqwest::Client::new();
        let providers = self.providers.lock().unwrap();
        let provider_ids = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered();

        let _: Vec<_> = providers
            .iter()
            .filter(|p| provider_ids.contains(&p.get_id()))
            .map(|p| p.load_channel_emotes(broadcaster_id.clone(), &http_client))
            .collect();
    }

    pub fn load_user_emotes(&self) {
        let providers = self.providers.lock().unwrap();
        for p in providers.iter() {
            p.load_user_emotes();
        }
    }

    pub fn get_emote_cache(&self, scope: String, emote_settings: &EmoteSettings) -> MultiCache {
        let providers = self.providers.lock().unwrap();
        let provider_ids = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered();
        let caches: Vec<_> = provider_ids
            .iter()
            .flat_map(|id| providers.iter().find(|p| p.get_id() == *id))
            .flat_map(|p| p.get_emote_cache(scope.clone()).into_caches())
            .collect();

        let mc = MultiCache::new(caches);

        tracing::trace!(scope = scope, name = mc.name(), "get emote cache");
        mc
    }

    pub fn insert_twitch_fragment_emote(
        &self,
        scope: String,
        name: String,
        emote: Emote,
        emote_settings: &EmoteSettings,
    ) {
        let emote_settings = emote_settings.clone().normalized();
        if !emote_settings.provider_enabled(EmoteProviderId::Twitch) {
            return;
        }

        let providers = self.providers.lock().unwrap();
        if let Some(provider) = providers
            .iter()
            .find(|p| p.get_id() == EmoteProviderId::Twitch)
        {
            let cache = provider.get_emote_cache(scope);
            if !cache.has_emote(name.clone()) {
                cache.set_emote(name, emote);
            }
        }
    }

    fn provider(&self, id: EmoteProviderId) -> Box<dyn EmoteProvider<MultiCache> + Send> {
        match id {
            EmoteProviderId::Twitch => {
                Box::new(TwitchProvider::new(self.client.clone(), self.token.clone()))
            }
            EmoteProviderId::Bttv => Box::new(BttvProvider::new()),
            EmoteProviderId::Ffz => Box::new(FfzProvider::new()),
            EmoteProviderId::Seventv => Box::new(SeventvProvider::new()),
        }
    }

    pub fn resolve_user_name(&self, user_id: &str) -> Option<String> {
        // Check cache first
        {
            let cache = self.name_cache.lock().unwrap();
            if let Some(name) = cache.get(user_id) {
                return Some(name.clone());
            }
        }

        // Resolve via Helix API
        let result = tauri::async_runtime::block_on(async {
            let token = self.token.lock().await.clone();
            self.client.get_user_from_id(user_id, &token).await
        });

        match result {
            Ok(Some(user)) => {
                let name = user.display_name.to_string();
                debug!(user_id, name, "resolved user name for emote owner");
                let mut cache = self.name_cache.lock().unwrap();
                cache.insert(user_id.to_string(), name.clone());
                Some(name)
            }
            Ok(None) => {
                debug!(user_id, "no user found for emote owner id");
                None
            }
            Err(err) => {
                error!(user_id, "failed to resolve emote owner name: {}", err);
                None
            }
        }
    }
}

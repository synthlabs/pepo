use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCacheTrait, MultiCache},
    persist::{SharedEmoteMetadataStore, TauriEmoteMetadataStore},
    providers::{
        bttv::BttvProvider, ffz::FfzProvider, http::provider_client, seventv::SeventvProvider,
        twitch::TwitchProvider, EmoteProvider,
    },
    Emote,
};
use crate::types::{EmoteProviderId, EmoteSettings};
use crate::SharedTwitchToken;

type ProviderRef = Arc<dyn EmoteProvider<MultiCache>>;
// TODO: switch to a RWLock instead of Mutex
type SharedProviders = Arc<Mutex<Vec<ProviderRef>>>;
type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    pub providers: SharedProviders,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: SharedTwitchToken,
    persistence: SharedEmoteMetadataStore,
    name_cache: SharedMap<String>,
}

impl EmoteManager {
    pub fn empty(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: SharedTwitchToken,
        app_handle: tauri::AppHandle,
    ) -> EmoteManager {
        EmoteManager {
            providers: Arc::new(Mutex::new(Vec::new())),
            client,
            token,
            persistence: Arc::new(TauriEmoteMetadataStore::new(app_handle)),
            name_cache: Default::default(),
        }
    }

    pub fn load_global(&self, emote_settings: &EmoteSettings) {
        let http_client = provider_client();

        let providers: Vec<ProviderRef> = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered()
            .into_iter()
            .map(|id| self.provider(id))
            .collect();

        {
            let mut store = self.providers.lock().unwrap();
            *store = providers.clone();
        }

        for p in &providers {
            debug!(provider = %p.get_name(), "loading global emotes");
            p.load_global_emotes(&http_client);
        }
    }

    pub async fn load_channel(self, broadcaster_id: String, emote_settings: &EmoteSettings) {
        let http_client = provider_client();
        let provider_ids = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered();
        let providers = {
            let providers = self.providers.lock().unwrap();
            providers
                .iter()
                .filter(|p| provider_ids.contains(&p.get_id()))
                .cloned()
                .collect::<Vec<_>>()
        };

        debug!(
            broadcaster_id,
            provider_count = providers.len(),
            "loading channel emotes"
        );

        let _: Vec<_> = providers
            .iter()
            .map(|p| {
                debug!(
                    broadcaster_id,
                    provider = %p.get_name(),
                    "loading channel emotes for provider"
                );
                p.load_channel_emotes(broadcaster_id.clone(), &http_client)
            })
            .collect();
    }

    pub fn load_user_emotes(&self) {
        let providers = self.providers.lock().unwrap().clone();
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
            provider.insert_emote(scope, name, emote);
        }
    }

    fn provider(&self, id: EmoteProviderId) -> ProviderRef {
        match id {
            EmoteProviderId::Twitch => {
                Arc::new(TwitchProvider::new(self.client.clone(), self.token.clone()))
            }
            EmoteProviderId::Bttv => Arc::new(BttvProvider::new(self.persistence.clone())),
            EmoteProviderId::Ffz => Arc::new(FfzProvider::new(self.persistence.clone())),
            EmoteProviderId::Seventv => Arc::new(SeventvProvider::new(self.persistence.clone())),
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

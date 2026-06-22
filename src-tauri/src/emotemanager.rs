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
        twitch::TwitchProvider, EmoteProvider, GLOBAL_SCOPE_KEY,
    },
    Emote,
};
use crate::token::TokenManager;
use crate::types::{EmoteProviderId, EmoteSettings};

type ProviderRef = Arc<dyn EmoteProvider<MultiCache>>;
// TODO: switch to a RWLock instead of Mutex
type SharedProviders = Arc<Mutex<Vec<ProviderRef>>>;
type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    pub providers: SharedProviders,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token_manager: Option<TokenManager>,
    persistence: SharedEmoteMetadataStore,
    name_cache: SharedMap<String>,
}

impl EmoteManager {
    pub fn empty(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        app_handle: tauri::AppHandle,
    ) -> EmoteManager {
        EmoteManager {
            providers: Arc::new(Mutex::new(Vec::new())),
            client,
            token_manager: Some(token_manager),
            persistence: Arc::new(TauriEmoteMetadataStore::new(app_handle)),
            name_cache: Default::default(),
        }
    }

    #[cfg(test)]
    fn with_persistence_for_test(persistence: SharedEmoteMetadataStore) -> EmoteManager {
        EmoteManager {
            providers: Arc::new(Mutex::new(Vec::new())),
            client: twitch_api::HelixClient::with_client(reqwest::Client::new()),
            token_manager: None,
            persistence,
            name_cache: Default::default(),
        }
    }

    pub fn load_global(&self, emote_settings: &EmoteSettings) {
        let http_client = provider_client();

        let providers = self.ensure_providers(emote_settings);

        for p in &providers {
            debug!(provider = %p.get_name(), "loading global emotes");
            p.load_global_emotes(&http_client);
        }
    }

    pub async fn load_channel(self, broadcaster_id: String, emote_settings: &EmoteSettings) {
        let http_client = provider_client();
        let providers = self.ensure_providers(emote_settings);

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

    pub fn preload(&self, scope: &str, emote_settings: &EmoteSettings) {
        let providers = self.ensure_providers(emote_settings);
        for p in &providers {
            let hydrated_global = p.hydrate_cache(GLOBAL_SCOPE_KEY);
            let hydrated_channel =
                scope != GLOBAL_SCOPE_KEY && p.hydrate_cache(scope);
            if hydrated_global || hydrated_channel {
                debug!(
                    provider = %p.get_name(),
                    scope,
                    hydrated_global,
                    hydrated_channel,
                    "preloaded persisted emote metadata"
                );
            }
        }
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
            EmoteProviderId::Twitch => Arc::new(TwitchProvider::new(
                self.client.clone(),
                self.token_manager
                    .as_ref()
                    .expect("twitch provider requires token manager")
                    .clone(),
            )),
            EmoteProviderId::Bttv => Arc::new(BttvProvider::new(self.persistence.clone())),
            EmoteProviderId::Ffz => Arc::new(FfzProvider::new(self.persistence.clone())),
            EmoteProviderId::Seventv => Arc::new(SeventvProvider::new(self.persistence.clone())),
        }
    }

    fn ensure_providers(&self, emote_settings: &EmoteSettings) -> Vec<ProviderRef> {
        let provider_ids = emote_settings
            .clone()
            .normalized()
            .enabled_provider_ids_ordered();
        let mut store = self.providers.lock().unwrap();
        let providers = provider_ids
            .into_iter()
            .map(|id| {
                store
                    .iter()
                    .find(|provider| provider.get_id() == id)
                    .cloned()
                    .unwrap_or_else(|| self.provider(id))
            })
            .collect::<Vec<_>>();
        *store = providers.clone();
        providers
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
            let Some(token_manager) = &self.token_manager else {
                return Err("no active token".to_owned());
            };
            let Some(token) = token_manager.active_twitch_token().await else {
                return Err("no active token".to_owned());
            };
            self.client
                .get_user_from_id(user_id, &token)
                .await
                .map_err(|err| err.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emote::{
        cache::EmoteCacheTrait,
        persist::MemoryEmoteMetadataStore,
        providers::GLOBAL_SCOPE_KEY,
    };

    const NOW: u64 = 1_800_000_000;

    fn emote(name: &str) -> Emote {
        Emote {
            id: format!("id-{name}"),
            name: name.to_string(),
            provider: "BTTV".to_string(),
            scope: "Channel".to_string(),
            ..Default::default()
        }
    }

    fn bttv_only_settings() -> EmoteSettings {
        EmoteSettings {
            providers: vec![
                crate::types::EmoteProviderPreference {
                    id: EmoteProviderId::Bttv,
                    enabled: true,
                },
                crate::types::EmoteProviderPreference {
                    id: EmoteProviderId::Twitch,
                    enabled: false,
                },
                crate::types::EmoteProviderPreference {
                    id: EmoteProviderId::Ffz,
                    enabled: false,
                },
                crate::types::EmoteProviderPreference {
                    id: EmoteProviderId::Seventv,
                    enabled: false,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn ensure_providers_preserves_preloaded_channel_cache() {
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Bttv,
            "1234",
            vec![emote("CachedBTTV")],
            NOW,
        );
        let manager = EmoteManager::with_persistence_for_test(persistence);
        let settings = bttv_only_settings();

        manager.preload("1234", &settings);
        manager.ensure_providers(&settings);

        let cache = manager.get_emote_cache("1234".to_string(), &settings);
        assert!(cache.has_emote("CachedBTTV".to_string()));
    }

    #[test]
    fn preload_reads_global_and_channel_scopes() {
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Bttv,
            GLOBAL_SCOPE_KEY,
            vec![emote("CachedGlobalBTTV")],
            NOW,
        );
        persistence.insert(
            EmoteProviderId::Bttv,
            "1234",
            vec![emote("CachedChannelBTTV")],
            NOW,
        );
        let manager = EmoteManager::with_persistence_for_test(persistence);
        let settings = bttv_only_settings();

        manager.preload("1234", &settings);

        let cache = manager.get_emote_cache("1234".to_string(), &settings);
        assert!(cache.has_emote("CachedGlobalBTTV".to_string()));
        assert!(cache.has_emote("CachedChannelBTTV".to_string()));
    }
}

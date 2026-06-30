use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Deserialize;
use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    persist::SharedEmoteMetadataStore,
    providers::{http::fetch_json, EmoteProvider, GLOBAL_SCOPE_KEY},
    Emote,
};
use crate::types::{EmoteProviderId, ProviderSettings};

const BTTV_API_BASE: &str = "https://api.betterttv.net/3/cached";
const BTTV_CDN_BASE: &str = "https://cdn.betterttv.net/emote";

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BttvEmote {
    id: String,
    code: String,
    image_type: String,
    animated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BttvChannelResponse {
    channel_emotes: Vec<BttvEmote>,
    shared_emotes: Vec<BttvEmote>,
}

fn bttv_to_emote(bttv: &BttvEmote, scope: &str) -> Emote {
    let format = if bttv.animated {
        "animated".to_string()
    } else {
        "static".to_string()
    };
    Emote {
        id: bttv.id.clone(),
        name: bttv.code.clone(),
        url: format!("{}/{}/3x", BTTV_CDN_BASE, bttv.id),
        format: vec![format],
        scale: vec!["1x".to_string(), "2x".to_string(), "3x".to_string()],
        provider: "BTTV".to_string(),
        scope: scope.to_string(),
        ..Default::default()
    }
}

#[derive(Clone)]
pub struct BttvProvider {
    cache: SharedMap<EmoteCache>,
    persistence: SharedEmoteMetadataStore,
    api_base: String,
}

impl BttvProvider {
    pub(crate) fn new(persistence: SharedEmoteMetadataStore) -> Self {
        Self::with_api_base(persistence, BTTV_API_BASE.to_string())
    }

    fn with_api_base(persistence: SharedEmoteMetadataStore, api_base: String) -> Self {
        BttvProvider {
            cache: Default::default(),
            persistence,
            api_base,
        }
    }

    fn hydrate_persisted_cache(
        &self,
        scope_key: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<EmoteCache> {
        if let Some(cache) = self.cache.lock().unwrap().get(scope_key).cloned() {
            return Some(cache);
        }

        let hydrated = self.persistence.load_cache(
            self.get_id(),
            scope_key,
            &self.get_name(),
            provider_settings,
        )?;
        debug!(
            provider = %self.get_name(),
            scope_key,
            count = hydrated.count(),
            age_seconds = hydrated.age_seconds(),
            "hydrated persisted emote metadata"
        );

        let cache = hydrated.cache;
        self.cache
            .lock()
            .unwrap()
            .insert(scope_key.to_string(), cache.clone());
        Some(cache)
    }

    fn store_fresh_cache(
        &self,
        scope_key: String,
        cache: EmoteCache,
        provider_settings: &ProviderSettings,
    ) {
        self.persistence
            .save_cache(self.get_id(), &scope_key, &cache, provider_settings);
        self.cache.lock().unwrap().insert(scope_key, cache);
    }

    fn log_fallback(&self, scope_key: &str, cache: Option<EmoteCache>) {
        if let Some(cache) = cache {
            debug!(
                provider = %self.get_name(),
                scope_key,
                count = cache.len(),
                "using persisted emote metadata fallback"
            );
        }
    }
}

impl EmoteProvider<MultiCache> for BttvProvider {
    fn get_id(&self) -> EmoteProviderId {
        EmoteProviderId::Bttv
    }

    fn hydrate_cache(&self, scope_key: &str, provider_settings: &ProviderSettings) -> bool {
        self.hydrate_persisted_cache(scope_key, provider_settings)
            .is_some()
    }

    fn load_global_emotes(&self, client: &reqwest::Client, provider_settings: &ProviderSettings) {
        let fallback = self.hydrate_persisted_cache(GLOBAL_SCOPE_KEY, provider_settings);
        let url = format!("{}/emotes/global", self.api_base);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<Vec<BttvEmote>>(client, "BTTV", "global", &url).await
            })
        }) {
            Ok(emotes) => {
                let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
                debug!(count = emotes.len(), "loaded bttv global emotes");
                for bttv in &emotes {
                    cache.set_emote(bttv.code.clone(), bttv_to_emote(bttv, "Global"));
                }
                self.store_fresh_cache(GLOBAL_SCOPE_KEY.to_owned(), cache, provider_settings);
            }
            Err(err) => {
                error!("failed to load bttv global emotes: err={}", err);
                self.log_fallback(GLOBAL_SCOPE_KEY, fallback);
            }
        }
    }

    fn load_channel_emotes(
        &self,
        broadcaster_id: String,
        client: &reqwest::Client,
        provider_settings: &ProviderSettings,
    ) {
        let fallback = self.hydrate_persisted_cache(&broadcaster_id, provider_settings);
        let url = format!("{}/users/twitch/{}", self.api_base, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<BttvChannelResponse>(
                    client,
                    "BTTV",
                    format!("channel:{broadcaster_id}"),
                    &url,
                )
                .await
            })
        }) {
            Ok(resp) => {
                let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
                let total = resp.channel_emotes.len() + resp.shared_emotes.len();
                debug!(broadcaster_id, count = total, "loaded bttv channel emotes");
                for bttv in resp.channel_emotes.iter().chain(resp.shared_emotes.iter()) {
                    cache.set_emote(bttv.code.clone(), bttv_to_emote(bttv, "Channel"));
                }
                self.store_fresh_cache(broadcaster_id, cache, provider_settings);
            }
            Err(err) => {
                error!(
                    broadcaster_id,
                    "failed to load bttv channel emotes: err={}", err
                );
                self.log_fallback(&broadcaster_id, fallback);
            }
        }
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        tracing::trace!(scope, "bttv get_emote_cache");
        let mut caches = Vec::new();
        if let Some(channel_cache) = store.get(&scope) {
            caches.push(channel_cache.clone());
        }
        if let Some(global_cache) = store.get(GLOBAL_SCOPE_KEY) {
            caches.push(global_cache.clone());
        }
        MultiCache::new(caches)
    }
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;

    use super::*;
    use crate::emote::{
        cache::EmoteCacheTrait,
        persist::MemoryEmoteMetadataStore,
        providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
    };

    const NOW: u64 = 1_800_000_000;

    fn emote(name: &str) -> Emote {
        Emote {
            id: format!("id-{name}"),
            name: name.to_string(),
            provider: "BTTV".to_string(),
            scope: "Global".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_global_load_uses_persisted_cache() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/emotes/global");
            then.status(503).body("down");
        });
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Bttv,
            GLOBAL_SCOPE_KEY,
            vec![emote("CachedBTTV")],
            NOW,
        );
        let provider = BttvProvider::with_api_base(persistence, server.base_url());

        provider.load_global_emotes(&reqwest::Client::new(), &ProviderSettings::default());

        let cache = provider.get_emote_cache(GLOBAL_SCOPE_KEY.to_string());
        assert!(cache.has_emote("CachedBTTV".to_string()));
        mock.assert();
    }
}

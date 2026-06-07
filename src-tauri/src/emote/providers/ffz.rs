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
use crate::types::EmoteProviderId;

const FFZ_API_BASE: &str = "https://api.frankerfacez.com/v1";
const FFZ_CDN_BASE: &str = "https://cdn.frankerfacez.com/emote";

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Deserialize)]
struct FfzAnimated {}

#[derive(Deserialize)]
struct FfzEmote {
    id: u64,
    name: String,
    animated: Option<FfzAnimated>,
}

#[derive(Deserialize)]
struct FfzEmoteSet {
    #[serde(rename = "emoticons")]
    emotes: Vec<FfzEmote>,
}

#[derive(Deserialize)]
struct FfzGlobalResponse {
    #[allow(dead_code)]
    default_sets: Vec<u64>,
    sets: HashMap<String, FfzEmoteSet>,
}

#[derive(Deserialize)]
struct FfzRoomResponse {
    sets: HashMap<String, FfzEmoteSet>,
}

fn ffz_to_emote(ffz: &FfzEmote, scope: &str) -> Emote {
    let is_animated = ffz.animated.is_some();
    let format = if is_animated {
        "animated".to_string()
    } else {
        "static".to_string()
    };
    let url = if is_animated {
        format!("{}/{}/animated/4", FFZ_CDN_BASE, ffz.id)
    } else {
        format!("{}/{}/4", FFZ_CDN_BASE, ffz.id)
    };
    Emote {
        id: ffz.id.to_string(),
        name: ffz.name.clone(),
        url,
        format: vec![format],
        scale: vec!["1".to_string(), "2".to_string(), "4".to_string()],
        provider: "FFZ".to_string(),
        scope: scope.to_string(),
        ..Default::default()
    }
}

#[derive(Clone)]
pub struct FfzProvider {
    cache: SharedMap<EmoteCache>,
    persistence: SharedEmoteMetadataStore,
    api_base: String,
}

impl FfzProvider {
    pub(crate) fn new(persistence: SharedEmoteMetadataStore) -> Self {
        Self::with_api_base(persistence, FFZ_API_BASE.to_string())
    }

    fn with_api_base(persistence: SharedEmoteMetadataStore, api_base: String) -> Self {
        FfzProvider {
            cache: Default::default(),
            persistence,
            api_base,
        }
    }

    fn hydrate_cache(&self, scope_key: &str) -> Option<EmoteCache> {
        if let Some(cache) = self.cache.lock().unwrap().get(scope_key).cloned() {
            return Some(cache);
        }

        let hydrated = self
            .persistence
            .load_cache(self.get_id(), scope_key, &self.get_name())?;
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

    fn store_fresh_cache(&self, scope_key: String, cache: EmoteCache) {
        self.persistence
            .save_cache(self.get_id(), &scope_key, &cache);
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

impl EmoteProvider<MultiCache> for FfzProvider {
    fn get_id(&self) -> EmoteProviderId {
        EmoteProviderId::Ffz
    }

    fn load_global_emotes(&self, client: &reqwest::Client) {
        let fallback = self.hydrate_cache(GLOBAL_SCOPE_KEY);
        let url = format!("{}/set/global", self.api_base);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<FfzGlobalResponse>(client, "FFZ", "global", &url).await
            })
        }) {
            Ok(resp) => {
                let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
                let mut count = 0;
                for set in resp.sets.values() {
                    for emote in &set.emotes {
                        cache.set_emote(emote.name.clone(), ffz_to_emote(emote, "Global"));
                        count += 1;
                    }
                }
                debug!(count, "loaded ffz global emotes");
                self.store_fresh_cache(GLOBAL_SCOPE_KEY.to_owned(), cache);
            }
            Err(err) => {
                error!("failed to load ffz global emotes: err={}", err);
                self.log_fallback(GLOBAL_SCOPE_KEY, fallback);
            }
        }
    }

    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client) {
        let fallback = self.hydrate_cache(&broadcaster_id);
        let url = format!("{}/room/id/{}", self.api_base, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<FfzRoomResponse>(
                    client,
                    "FFZ",
                    format!("channel:{broadcaster_id}"),
                    &url,
                )
                .await
            })
        }) {
            Ok(resp) => {
                let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
                let mut count = 0;
                for set in resp.sets.values() {
                    for emote in &set.emotes {
                        cache.set_emote(emote.name.clone(), ffz_to_emote(emote, "Channel"));
                        count += 1;
                    }
                }
                debug!(broadcaster_id, count, "loaded ffz channel emotes");
                self.store_fresh_cache(broadcaster_id, cache);
            }
            Err(err) => {
                error!(
                    broadcaster_id,
                    "failed to load ffz channel emotes: err={}", err
                );
                self.log_fallback(&broadcaster_id, fallback);
            }
        }
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        tracing::trace!(scope, "ffz get_emote_cache");
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
            provider: "FFZ".to_string(),
            scope: "Global".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_global_load_uses_persisted_cache() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/set/global");
            then.status(503).body("down");
        });
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Ffz,
            GLOBAL_SCOPE_KEY,
            vec![emote("CachedFFZ")],
            NOW,
        );
        let provider = FfzProvider::with_api_base(persistence, server.base_url());

        provider.load_global_emotes(&reqwest::Client::new());

        let cache = provider.get_emote_cache(GLOBAL_SCOPE_KEY.to_string());
        assert!(cache.has_emote("CachedFFZ".to_string()));
        mock.assert();
    }
}

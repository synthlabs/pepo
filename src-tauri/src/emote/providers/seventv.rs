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

const SEVENTV_API_BASE: &str = "https://7tv.io/v3";

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Deserialize)]
struct SeventvHost {
    url: String,
}

#[derive(Deserialize)]
struct SeventvEmoteData {
    animated: bool,
    host: SeventvHost,
}

#[derive(Deserialize)]
struct SeventvEmote {
    id: String,
    name: String,
    data: SeventvEmoteData,
}

#[derive(Deserialize)]
struct SeventvGlobalResponse {
    emotes: Vec<SeventvEmote>,
}

#[derive(Deserialize)]
struct SeventvChannelEmoteSet {
    emotes: Option<Vec<SeventvEmote>>,
}

#[derive(Deserialize)]
struct SeventvChannelResponse {
    emote_set: SeventvChannelEmoteSet,
}

fn seventv_to_emote(emote: &SeventvEmote, scope: &str) -> Emote {
    let format = if emote.data.animated {
        "animated".to_string()
    } else {
        "static".to_string()
    };
    Emote {
        id: emote.id.clone(),
        name: emote.name.clone(),
        url: format!("https:{}/4x.webp", emote.data.host.url),
        format: vec![format],
        scale: vec![
            "1x".to_string(),
            "2x".to_string(),
            "3x".to_string(),
            "4x".to_string(),
        ],
        provider: "7TV".to_string(),
        scope: scope.to_string(),
        ..Default::default()
    }
}

#[derive(Clone)]
pub struct SeventvProvider {
    cache: SharedMap<EmoteCache>,
    persistence: SharedEmoteMetadataStore,
    api_base: String,
}

impl SeventvProvider {
    pub(crate) fn new(persistence: SharedEmoteMetadataStore) -> Self {
        Self::with_api_base(persistence, SEVENTV_API_BASE.to_string())
    }

    fn with_api_base(persistence: SharedEmoteMetadataStore, api_base: String) -> Self {
        SeventvProvider {
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

impl EmoteProvider<MultiCache> for SeventvProvider {
    fn get_id(&self) -> EmoteProviderId {
        EmoteProviderId::Seventv
    }

    fn load_global_emotes(&self, client: &reqwest::Client) {
        let fallback = self.hydrate_cache(GLOBAL_SCOPE_KEY);
        let url = format!("{}/emote-sets/global", self.api_base);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<SeventvGlobalResponse>(client, "7TV", "global", &url).await
            })
        }) {
            Ok(resp) => {
                let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
                debug!(count = resp.emotes.len(), "loaded seventv global emotes");
                for emote in &resp.emotes {
                    cache.set_emote(emote.name.clone(), seventv_to_emote(emote, "Global"));
                }
                self.store_fresh_cache(GLOBAL_SCOPE_KEY.to_owned(), cache);
            }
            Err(err) => {
                error!("failed to load seventv global emotes: err={}", err);
                self.log_fallback(GLOBAL_SCOPE_KEY, fallback);
            }
        }
    }

    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client) {
        let fallback = self.hydrate_cache(&broadcaster_id);
        let url = format!("{}/users/twitch/{}", self.api_base, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                fetch_json::<SeventvChannelResponse>(
                    client,
                    "7TV",
                    format!("channel:{broadcaster_id}"),
                    &url,
                )
                .await
            })
        }) {
            Ok(resp) => {
                let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
                let emotes = resp.emote_set.emotes.unwrap_or_default();
                debug!(
                    broadcaster_id,
                    count = emotes.len(),
                    "loaded seventv channel emotes"
                );
                for emote in &emotes {
                    cache.set_emote(emote.name.clone(), seventv_to_emote(emote, "Channel"));
                }
                self.store_fresh_cache(broadcaster_id, cache);
            }
            Err(err) => {
                error!(
                    broadcaster_id,
                    "failed to load seventv channel emotes: err={}", err
                );
                self.log_fallback(&broadcaster_id, fallback);
            }
        }
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        tracing::trace!(scope, "seventv get_emote_cache");
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
        persist::{EmoteMetadataStore, MemoryEmoteMetadataStore},
        providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
    };

    const NOW: u64 = 1_800_000_000;

    fn emote(name: &str, scope: &str) -> Emote {
        Emote {
            id: format!("id-{name}"),
            name: name.to_string(),
            provider: "7TV".to_string(),
            scope: scope.to_string(),
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_global_load_uses_persisted_cache() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/emote-sets/global");
            then.status(503).body("down");
        });
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Seventv,
            GLOBAL_SCOPE_KEY,
            vec![emote("Cached7TV", "Global")],
            NOW,
        );
        let provider = SeventvProvider::with_api_base(persistence, server.base_url());

        provider.load_global_emotes(&reqwest::Client::new());

        let cache = provider.get_emote_cache(GLOBAL_SCOPE_KEY.to_string());
        assert!(cache.has_emote("Cached7TV".to_string()));
        mock.assert();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_channel_load_uses_persisted_cache() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/users/twitch/1234");
            then.status(503).body("down");
        });
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Seventv,
            "1234",
            vec![emote("CachedChannel7TV", "Channel")],
            NOW,
        );
        let provider = SeventvProvider::with_api_base(persistence, server.base_url());

        provider.load_channel_emotes("1234".to_string(), &reqwest::Client::new());

        let cache = provider.get_emote_cache("1234".to_string());
        assert!(cache.has_emote("CachedChannel7TV".to_string()));
        mock.assert();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn successful_global_load_overwrites_persisted_cache() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/emote-sets/global");
            then.status(200).json_body_obj(&serde_json::json!({
                "emotes": [
                    {
                        "id": "fresh-id",
                        "name": "Fresh7TV",
                        "data": {
                            "animated": false,
                            "host": {
                                "url": "//cdn.7tv.app/emote/fresh-id"
                            }
                        }
                    }
                ]
            }));
        });
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        persistence.insert(
            EmoteProviderId::Seventv,
            GLOBAL_SCOPE_KEY,
            vec![emote("Stale7TV", "Global")],
            NOW,
        );
        let provider = SeventvProvider::with_api_base(persistence.clone(), server.base_url());

        provider.load_global_emotes(&reqwest::Client::new());

        let cache = provider.get_emote_cache(GLOBAL_SCOPE_KEY.to_string());
        assert!(cache.has_emote("Fresh7TV".to_string()));
        assert!(!cache.has_emote("Stale7TV".to_string()));

        let persisted = persistence
            .load_cache(
                EmoteProviderId::Seventv,
                GLOBAL_SCOPE_KEY,
                &provider.get_name(),
            )
            .unwrap();
        assert!(persisted.cache.has_emote("Fresh7TV".to_string()));
        assert!(!persisted.cache.has_emote("Stale7TV".to_string()));
        mock.assert();
    }
}

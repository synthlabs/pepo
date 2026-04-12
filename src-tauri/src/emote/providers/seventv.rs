use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Deserialize;
use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
    Emote,
};

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
}

impl SeventvProvider {
    pub fn new() -> Self {
        SeventvProvider {
            cache: Default::default(),
        }
    }
}

impl EmoteProvider<MultiCache> for SeventvProvider {
    fn get_name(&self) -> String {
        "SeventvProvider".to_string()
    }

    fn load_global_emotes(&self, client: &reqwest::Client) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
        let url = format!("{}/emote-sets/global", SEVENTV_API_BASE);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .send()
                    .await?
                    .json::<SeventvGlobalResponse>()
                    .await
            })
        }) {
            Ok(resp) => {
                debug!(count = resp.emotes.len(), "loaded seventv global emotes");
                for emote in &resp.emotes {
                    cache.set_emote(emote.name.clone(), seventv_to_emote(emote, "Global"));
                }
            }
            Err(err) => error!("failed to load seventv global emotes: err={}", err),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(GLOBAL_SCOPE_KEY.to_owned(), cache);
    }

    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client) {
        let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
        let url = format!("{}/users/twitch/{}", SEVENTV_API_BASE, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .send()
                    .await?
                    .json::<SeventvChannelResponse>()
                    .await
            })
        }) {
            Ok(resp) => {
                let emotes = resp.emote_set.emotes.unwrap_or_default();
                debug!(
                    broadcaster_id,
                    count = emotes.len(),
                    "loaded seventv channel emotes"
                );
                for emote in &emotes {
                    cache.set_emote(emote.name.clone(), seventv_to_emote(emote, "Channel"));
                }
            }
            Err(err) => error!(
                broadcaster_id,
                "failed to load seventv channel emotes: err={}", err
            ),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(broadcaster_id, cache);
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        debug!(scope, "seventv get_emote_cache");
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

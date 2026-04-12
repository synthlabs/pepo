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
}

impl FfzProvider {
    pub fn new() -> Self {
        FfzProvider {
            cache: Default::default(),
        }
    }
}

impl EmoteProvider<MultiCache> for FfzProvider {
    fn get_name(&self) -> String {
        "FfzProvider".to_string()
    }

    fn load_global_emotes(&self, client: &reqwest::Client) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
        let url = format!("{}/set/global", FFZ_API_BASE);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .send()
                    .await?
                    .json::<FfzGlobalResponse>()
                    .await
            })
        }) {
            Ok(resp) => {
                let mut count = 0;
                for set in resp.sets.values() {
                    for emote in &set.emotes {
                        cache.set_emote(emote.name.clone(), ffz_to_emote(emote, "Global"));
                        count += 1;
                    }
                }
                debug!(count, "loaded ffz global emotes");
            }
            Err(err) => error!("failed to load ffz global emotes: err={}", err),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(GLOBAL_SCOPE_KEY.to_owned(), cache);
    }

    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client) {
        let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
        let url = format!("{}/room/id/{}", FFZ_API_BASE, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .send()
                    .await?
                    .json::<FfzRoomResponse>()
                    .await
            })
        }) {
            Ok(resp) => {
                let mut count = 0;
                for set in resp.sets.values() {
                    for emote in &set.emotes {
                        cache.set_emote(emote.name.clone(), ffz_to_emote(emote, "Channel"));
                        count += 1;
                    }
                }
                debug!(broadcaster_id, count, "loaded ffz channel emotes");
            }
            Err(err) => error!(
                broadcaster_id,
                "failed to load ffz channel emotes: err={}", err
            ),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(broadcaster_id, cache);
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        debug!(scope, "ffz get_emote_cache");
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

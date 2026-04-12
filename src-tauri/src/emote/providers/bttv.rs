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
}

impl BttvProvider {
    pub fn new() -> Self {
        BttvProvider {
            cache: Default::default(),
        }
    }
}

impl EmoteProvider<MultiCache> for BttvProvider {
    fn get_name(&self) -> String {
        "BttvProvider".to_string()
    }

    fn load_global_emotes(&self, client: &reqwest::Client) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());
        let url = format!("{}/emotes/global", BTTV_API_BASE);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { client.get(&url).send().await?.json::<Vec<BttvEmote>>().await })
        }) {
            Ok(emotes) => {
                debug!(count = emotes.len(), "loaded bttv global emotes");
                for bttv in &emotes {
                    cache.set_emote(bttv.code.clone(), bttv_to_emote(bttv, "Global"));
                }
            }
            Err(err) => error!("failed to load bttv global emotes: err={}", err),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(GLOBAL_SCOPE_KEY.to_owned(), cache);
    }

    fn load_channel_emotes(&self, broadcaster_id: String, client: &reqwest::Client) {
        let cache = EmoteCache::new(broadcaster_id.clone(), self.get_name());
        let url = format!("{}/users/twitch/{}", BTTV_API_BASE, broadcaster_id);

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .send()
                    .await?
                    .json::<BttvChannelResponse>()
                    .await
            })
        }) {
            Ok(resp) => {
                let total = resp.channel_emotes.len() + resp.shared_emotes.len();
                debug!(
                    broadcaster_id,
                    count = total,
                    "loaded bttv channel emotes"
                );
                for bttv in resp.channel_emotes.iter().chain(resp.shared_emotes.iter()) {
                    cache.set_emote(bttv.code.clone(), bttv_to_emote(bttv, "Channel"));
                }
            }
            Err(err) => error!(
                broadcaster_id,
                "failed to load bttv channel emotes: err={}", err
            ),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(broadcaster_id, cache);
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        debug!(scope, "bttv get_emote_cache");
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

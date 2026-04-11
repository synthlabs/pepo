use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tracing::{debug, error, warn};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
};

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct TwitchProvider {
    cache: SharedMap<EmoteCache>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: twitch_oauth2::UserToken,
}

impl TwitchProvider {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Self {
        TwitchProvider {
            cache: Default::default(),
            client,
            token,
        }
    }
}

impl EmoteProvider<MultiCache> for TwitchProvider {
    fn get_name(&self) -> String {
        "TwitchProvider".to_string()
    }

    fn load_global_emotes(&self, _client: &reqwest::Client) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.client.get_global_emotes(&self.token).await })
        }) {
            Ok(resp) => resp
                .iter()
                .map(|v| cache.clone().set_emote(v.name.clone(), v.into()))
                .collect(),
            Err(err) => error!("failed to load global emotes: err={}", err.to_string()),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(GLOBAL_SCOPE_KEY.to_owned(), cache);
    }

    fn load_channel_emotes(&self, _broadcaster_id: String, _client: &reqwest::Client) {
        warn!("load_channel_emotes - not implemented for twitch, they include emote fragments in their eventsub messages now so we load them dynamically per message");
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        debug!(scope, "twitch get_emote_cache");
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

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::TryStreamExt;
use tracing::{debug, error, info, warn};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
};

const USER_EMOTES_SCOPE_KEY: &str = "_user_emotes";

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

    fn load_user_emotes(&self) {
        let cache = EmoteCache::new(USER_EMOTES_SCOPE_KEY.to_owned(), self.get_name());

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client
                    .get_user_emotes(&self.token.user_id, &self.token)
                    .try_collect::<Vec<_>>()
                    .await
            })
        }) {
            Ok(emotes) => {
                info!("loaded {} user emotes", emotes.len());
                for emote in &emotes {
                    cache.set_emote(emote.name.clone(), emote.into());
                }
            }
            Err(err) => error!("failed to load user emotes: err={}", err),
        }

        let mut store = self.cache.lock().unwrap();
        store.insert(USER_EMOTES_SCOPE_KEY.to_owned(), cache);
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let store = self.cache.lock().unwrap();
        debug!(scope, "twitch get_emote_cache");
        let mut caches = Vec::new();
        if let Some(channel_cache) = store.get(&scope) {
            caches.push(channel_cache.clone());
        }
        if let Some(user_cache) = store.get(USER_EMOTES_SCOPE_KEY) {
            caches.push(user_cache.clone());
        }
        if let Some(global_cache) = store.get(GLOBAL_SCOPE_KEY) {
            caches.push(global_cache.clone());
        }
        MultiCache::new(caches)
    }
}

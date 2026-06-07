use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::TryStreamExt;
use tracing::{debug, error, info};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
    Emote,
};
use crate::types::EmoteProviderId;
use crate::SharedTwitchToken;

const USER_EMOTES_SCOPE_KEY: &str = "_user_emotes";

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct TwitchProvider {
    cache: SharedMap<EmoteCache>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: SharedTwitchToken,
}

impl TwitchProvider {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: SharedTwitchToken,
    ) -> Self {
        TwitchProvider {
            cache: Default::default(),
            client,
            token,
        }
    }
}

impl EmoteProvider<MultiCache> for TwitchProvider {
    fn get_id(&self) -> EmoteProviderId {
        EmoteProviderId::Twitch
    }

    fn load_global_emotes(&self, _client: &reqwest::Client) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let token = self.token.lock().await.clone();
                self.client.get_global_emotes(&token).await
            })
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

    fn load_channel_emotes(&self, broadcaster_id: String, _client: &reqwest::Client) {
        let mut store = self.cache.lock().unwrap();
        store
            .entry(broadcaster_id.clone())
            .or_insert_with(|| EmoteCache::new(broadcaster_id.clone(), self.get_name()));
        debug!(
            broadcaster_id,
            "prepared twitch channel emote cache for EventSub fragments"
        );
    }

    fn load_user_emotes(&self) {
        let cache = EmoteCache::new(USER_EMOTES_SCOPE_KEY.to_owned(), self.get_name());

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let token = self.token.lock().await.clone();
                self.client
                    .get_user_emotes(&token.user_id, &token)
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

    fn insert_emote(&self, scope: String, name: String, emote: Emote) {
        let cache = {
            let mut store = self.cache.lock().unwrap();
            store
                .entry(scope.clone())
                .or_insert_with(|| EmoteCache::new(scope, self.get_name()))
                .clone()
        };

        if !cache.has_emote(name.clone()) {
            cache.set_emote(name, emote);
        }
    }

    fn get_emote_cache(&self, scope: String) -> MultiCache {
        let mut store = self.cache.lock().unwrap();
        tracing::trace!(scope, "twitch get_emote_cache");
        let mut caches = Vec::new();
        if scope != GLOBAL_SCOPE_KEY && scope != USER_EMOTES_SCOPE_KEY {
            let cache_name = self.get_name();
            let channel_cache = store
                .entry(scope.clone())
                .or_insert_with(|| EmoteCache::new(scope.clone(), cache_name))
                .clone();
            caches.push(channel_cache);
        } else if let Some(channel_cache) = store.get(&scope) {
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

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
};

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct TwitchProvider {
    cache: SharedMap<EmoteCache>,
}

impl TwitchProvider {
    pub fn new() -> Self {
        TwitchProvider {
            cache: Default::default(),
        }
    }
}

impl EmoteProvider<EmoteCache> for TwitchProvider {
    fn get_name(&self) -> String {
        "TwitchProvider".to_string()
    }

    fn load_global_emotes(
        &self,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) {
        let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.to_owned(), self.get_name());

        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { client.get_global_emotes(&token).await })
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

    fn load_channel_emotes(
        &self,
        _broadcaster_id: String,
        _client: twitch_api::HelixClient<'static, reqwest::Client>,
        _token: twitch_oauth2::UserToken,
    ) {
        error!("load_channel_emotes - not implemented");
    }

    fn get_emote_cache(&self, scope: String) -> EmoteCache {
        // TODO: implement fallback cache
        let store = self.cache.lock().unwrap();
        debug!(scope, "get_emote_cache");
        store.get(GLOBAL_SCOPE_KEY).unwrap().clone()
    }

    // fn get_emote(&self, scope: String, name: String) -> Option<Emote> {
    //     error!("get_emote - not implemented");
    //     None
    // }
}

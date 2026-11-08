use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::emote::persist::SharedEmoteMetadataStore;
use crate::network::Failure;
use futures::TryStreamExt;
use tracing::error;

use crate::emote::{
    cache::{EmoteCache, EmoteCacheTrait, MultiCache},
    providers::{EmoteProvider, GLOBAL_SCOPE_KEY},
    Emote,
};
use crate::token::TokenManager;
use crate::types::{EmoteProviderId, ProviderSettings};

const USER_EMOTES_SCOPE_KEY: &str = "_user_emotes";

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct TwitchProvider {
    cache: SharedMap<EmoteCache>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token_manager: TokenManager,
    persistence: SharedEmoteMetadataStore,
    owner: String,
}

impl TwitchProvider {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token_manager: TokenManager,
        persistence: SharedEmoteMetadataStore,
        owner: String,
    ) -> Self {
        TwitchProvider {
            cache: Default::default(),
            client,
            token_manager,
            persistence,
            owner,
        }
    }
}

impl EmoteProvider<MultiCache> for TwitchProvider {
    fn get_id(&self) -> EmoteProviderId {
        EmoteProviderId::Twitch
    }

    fn hydrate_cache(&self, scope: &str, settings: &ProviderSettings) -> bool {
        let persisted_scope = if scope == USER_EMOTES_SCOPE_KEY {
            format!("{USER_EMOTES_SCOPE_KEY}:{}", self.owner)
        } else {
            scope.to_owned()
        };
        if scope == GLOBAL_SCOPE_KEY {
            self.hydrate_cache(USER_EMOTES_SCOPE_KEY, settings);
        }
        let mut caches = self.cache.lock().unwrap();
        if caches.contains_key(scope) {
            return true;
        }
        if let Some(hydrated) = self.persistence.load_cache(
            EmoteProviderId::Twitch,
            &persisted_scope,
            &self.get_name(),
            settings,
        ) {
            caches.insert(scope.into(), hydrated.cache);
            true
        } else {
            false
        }
    }

    fn refresh_global<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        settings: &'a ProviderSettings,
    ) -> futures::future::BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            self.hydrate_cache(GLOBAL_SCOPE_KEY, settings);
            let token = self
                .token_manager
                .active_twitch_token()
                .await
                .ok_or_else(|| {
                    Failure::new("authentication", "Waiting for Twitch authentication.", true)
                })?;
            let emotes = self
                .client
                .get_global_emotes(&token)
                .await
                .map_err(Failure::helix)?;
            let cache = EmoteCache::new(GLOBAL_SCOPE_KEY.into(), self.get_name());
            for emote in emotes {
                cache.set_emote(emote.name.clone(), (&emote).into());
            }
            self.persistence.save_cache(
                EmoteProviderId::Twitch,
                GLOBAL_SCOPE_KEY,
                &cache,
                settings,
            );
            self.cache
                .lock()
                .unwrap()
                .insert(GLOBAL_SCOPE_KEY.into(), cache);
            Ok(())
        })
    }

    fn refresh_user<'a>(
        &'a self,
        settings: &'a ProviderSettings,
    ) -> futures::future::BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            self.hydrate_cache(USER_EMOTES_SCOPE_KEY, settings);
            let token = self
                .token_manager
                .active_twitch_token()
                .await
                .ok_or_else(|| {
                    Failure::new("authentication", "Waiting for Twitch authentication.", true)
                })?;
            let emotes = self
                .client
                .get_user_emotes(&token.user_id, &token)
                .try_collect::<Vec<_>>()
                .await
                .map_err(Failure::helix)?;
            let cache = EmoteCache::new(USER_EMOTES_SCOPE_KEY.into(), self.get_name());
            for emote in emotes {
                cache.set_emote(emote.name.clone(), (&emote).into());
            }
            let scope = format!("{USER_EMOTES_SCOPE_KEY}:{}", self.owner);
            self.persistence
                .save_cache(EmoteProviderId::Twitch, &scope, &cache, settings);
            self.cache
                .lock()
                .unwrap()
                .insert(USER_EMOTES_SCOPE_KEY.into(), cache);
            Ok(())
        })
    }

    fn load_global_emotes(&self, client: &reqwest::Client, settings: &ProviderSettings) {
        if let Err(error) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.refresh_global(client, settings))
        }) {
            error!("failed to load global emotes: {}", error.message);
        }
    }
    fn load_channel_emotes(
        &self,
        broadcaster_id: String,
        _client: &reqwest::Client,
        _settings: &ProviderSettings,
    ) {
        self.cache
            .lock()
            .unwrap()
            .entry(broadcaster_id.clone())
            .or_insert_with(|| EmoteCache::new(broadcaster_id, self.get_name()));
    }
    fn load_user_emotes(&self) {
        if let Err(error) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.refresh_user(&ProviderSettings::default()))
        }) {
            error!("failed to load user emotes: {}", error.message);
        }
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

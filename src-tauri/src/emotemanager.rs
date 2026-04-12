use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tracing::{debug, error};

use crate::emote::{
    cache::{EmoteCacheTrait, MultiCache},
    providers::{
        bttv::BttvProvider, ffz::FfzProvider, seventv::SeventvProvider, twitch::TwitchProvider,
        EmoteProvider,
    },
};

// TODO: switch to a RWLock instead of Mutex
type SharedVec<V> = Arc<Mutex<Vec<V>>>;
type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    pub providers: SharedVec<Box<dyn EmoteProvider<MultiCache> + Send>>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: twitch_oauth2::UserToken,
    name_cache: SharedMap<String>,
}

impl EmoteManager {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Result<EmoteManager, String> {
        let http_client = reqwest::Client::new();

        let providers: Vec<Box<dyn EmoteProvider<MultiCache> + Send>> = vec![
            Box::new(TwitchProvider::new(client.clone(), token.clone())),
            Box::new(BttvProvider::new()),
            Box::new(FfzProvider::new()),
            Box::new(SeventvProvider::new()),
        ];

        let _: Vec<_> = providers
            .iter()
            .map(|p| p.load_global_emotes(&http_client))
            .collect();

        Ok(EmoteManager {
            providers: Arc::new(Mutex::new(providers)),
            client,
            token,
            name_cache: Default::default(),
        })
    }

    pub async fn load_channel(self, broadcaster_id: String) {
        let http_client = reqwest::Client::new();
        let providers = self.providers.lock().unwrap();

        let _: Vec<_> = providers
            .iter()
            .map(|p| p.load_channel_emotes(broadcaster_id.clone(), &http_client))
            .collect();
    }

    pub fn get_emote_cache(&self, scope: String) -> MultiCache {
        let providers = self.providers.lock().unwrap();
        let caches: Vec<_> = providers
            .iter()
            .flat_map(|p| {
                let mc = p.get_emote_cache(scope.clone());
                mc.into_caches()
            })
            .collect();

        let mc = MultiCache::new(caches);

        debug!(scope = scope, name = mc.name(), "get emote cache");
        mc
    }

    pub fn resolve_user_name(&self, user_id: &str) -> Option<String> {
        // Check cache first
        {
            let cache = self.name_cache.lock().unwrap();
            if let Some(name) = cache.get(user_id) {
                return Some(name.clone());
            }
        }

        // Resolve via Helix API
        let result = tauri::async_runtime::block_on(async {
            self.client
                .get_user_from_id(user_id, &self.token)
                .await
        });

        match result {
            Ok(Some(user)) => {
                let name = user.display_name.to_string();
                debug!(user_id, name, "resolved user name for emote owner");
                let mut cache = self.name_cache.lock().unwrap();
                cache.insert(user_id.to_string(), name.clone());
                Some(name)
            }
            Ok(None) => {
                debug!(user_id, "no user found for emote owner id");
                None
            }
            Err(err) => {
                error!(user_id, "failed to resolve emote owner name: {}", err);
                None
            }
        }
    }
}

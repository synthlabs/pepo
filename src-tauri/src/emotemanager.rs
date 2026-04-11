use std::sync::{Arc, Mutex};

use tracing::debug;

use crate::emote::{
    cache::{EmoteCacheTrait, MultiCache},
    providers::{bttv::BttvProvider, twitch::TwitchProvider, EmoteProvider},
};

// TODO: switch to a RWLock instead of Mutex
type SharedVec<V> = Arc<Mutex<Vec<V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    pub providers: SharedVec<Box<dyn EmoteProvider<MultiCache> + Send>>,
}

impl EmoteManager {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Result<EmoteManager, String> {
        let http_client = reqwest::Client::new();

        let providers: Vec<Box<dyn EmoteProvider<MultiCache> + Send>> = vec![
            Box::new(TwitchProvider::new(client, token)),
            Box::new(BttvProvider::new()),
        ];

        let _: Vec<_> = providers
            .iter()
            .map(|p| p.load_global_emotes(&http_client))
            .collect();

        Ok(EmoteManager {
            providers: Arc::new(Mutex::new(providers)),
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
}

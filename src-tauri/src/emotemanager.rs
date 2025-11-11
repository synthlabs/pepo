use std::sync::{Arc, Mutex};

use tracing::error;

use crate::emote::{
    cache::EmoteCache,
    providers::{twitch::TwitchProvider, EmoteProvider},
};

// TODO: switch to a RWLock instead of Mutex
type SharedVec<V> = Arc<Mutex<Vec<V>>>;

#[derive(Clone)]
pub struct EmoteManager {
    token: twitch_oauth2::UserToken,
    pub providers: SharedVec<Box<dyn EmoteProvider<EmoteCache> + Send>>,
}

impl EmoteManager {
    pub fn new(
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Result<EmoteManager, String> {
        let providers: Vec<Box<dyn EmoteProvider<EmoteCache> + Send>> =
            vec![Box::new(TwitchProvider::new())];

        let _: Vec<_> = providers
            .iter()
            .map(|p| p.load_global_emotes(client.clone(), token.clone()))
            .collect();

        Ok(EmoteManager {
            token: token.clone(),
            providers: Arc::new(Mutex::new(providers)),
        })
    }

    pub async fn load_channel(
        self,
        broadcaster_id: String,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
    ) {
        let providers = self.providers.lock().unwrap();
        let token_ref = self.token.clone();

        let _: Vec<_> = providers
            .iter()
            .map(|p| {
                p.load_channel_emotes(broadcaster_id.clone(), client.clone(), token_ref.clone())
            })
            .collect();
    }

    pub fn get_emote_cache(self, scope: String, provider_name: String) -> Option<EmoteCache> {
        let providers = self.providers.lock().unwrap();

        for p in providers.iter() {
            if p.get_name() == provider_name {
                return Some(p.get_emote_cache(scope));
            }
        }

        error!(scope, provider_name, "no emote cache matching");
        None
    }
}

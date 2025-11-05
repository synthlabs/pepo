use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::debug;
use twitch_api::{helix::chat::get_global_chat_badges, HelixClient};

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;
type Scope<T> = HashMap<String, T>;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Badge {}

impl From<twitch_api::helix::chat::BadgeSet> for Badge {
    fn from(value: twitch_api::helix::chat::BadgeSet) -> Self {
        Badge {}
    }
}

#[derive(Clone)]
pub struct BadgeManager {
    pub client: HelixClient<'static, reqwest::Client>,
    pub global_badges: SharedMap<Badge>,
    pub scoped_badges: SharedMap<Scope<Badge>>,
}

impl BadgeManager {
    pub async fn new(
        client: HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Result<BadgeManager, String> {
        let mut global_badges: HashMap<String, Badge> = Default::default();

        let req = get_global_chat_badges::GetGlobalChatBadgesRequest::new();

        debug!("getting global badges");
        let response: Vec<twitch_api::helix::chat::BadgeSet> =
            match client.req_get(req, &token).await {
                Ok(resp) => resp.data,
                Err(err) => return Err(err.to_string()),
            };

        let _: Vec<_> = response
            .into_iter()
            .map(|b| {
                let new_b = Badge::from(b.clone());
                debug!("adding badge: badge={:?}", b);
                global_badges.insert(b.set_id.to_string(), new_b);
            })
            .collect();

        Ok(BadgeManager {
            client: client,
            global_badges: Arc::new(Mutex::new(global_badges)),
            scoped_badges: Default::default(),
        })
    }

    pub fn set(self, id: String, channel: Option<String>, badge: Badge) {}

    pub fn get(self, id: String, channel: Option<String>) -> Option<Badge> {
        let global_badges = self.global_badges.lock().unwrap();
        let scoped_badges = self.scoped_badges.lock().unwrap();
        match channel {
            Some(channel) => match scoped_badges.get(&channel) {
                Some(scope) => return scope.get(&id).cloned(),
                None => global_badges.get(&id).cloned(),
            },
            None => global_badges.get(&id).cloned(),
        }
    }
}

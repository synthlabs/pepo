use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::{debug, error, trace};
use twitch_api::{
    helix::chat::{get_channel_chat_badges, get_global_chat_badges},
    HelixClient,
};

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;
type Scope<T> = HashMap<String, T>;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BadgeSet {
    /// An ID that identifies this set of chat badges. For example, Bits or Subscriber.
    pub set_id: String,
    /// Contains chat badge objects for the set.
    pub versions: Vec<Badge>,
}

impl BadgeSet {
    pub fn version(self, version: String) -> Option<Badge> {
        for b in self.versions {
            if b.id == version {
                return Some(b.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
pub struct Badge {
    /// An ID that identifies this set of chat badges. For example, Bits or Subscriber.
    pub set_id: String,
    /// An ID that identifies this version of the badge. The ID can be any value.
    /// For example, for Bits, the ID is the Bits tier level, but for World of Warcraft, it could be Alliance or Horde.
    pub id: String,
    /// URL to png of size 28x28
    pub image_url_1x: String,
    /// URL to png of size 56x56
    pub image_url_2x: String,
    /// URL to png of size 112x112
    pub image_url_4x: String,
    /// Title of the badge
    pub title: String,
    /// Descrition of the badge
    pub description: String,
}

impl From<twitch_api::helix::chat::BadgeSet> for BadgeSet {
    fn from(value: twitch_api::helix::chat::BadgeSet) -> Self {
        BadgeSet {
            set_id: value.set_id.to_string(),
            versions: value
                .versions
                .iter()
                .map(|b| Badge {
                    set_id: value.set_id.to_string(),
                    id: b.id.to_string(),
                    image_url_1x: b.image_url_1x.clone(),
                    image_url_2x: b.image_url_2x.clone(),
                    image_url_4x: b.image_url_4x.clone(),
                    title: b.title.clone(),
                    description: b.description.clone(),
                })
                .collect(),
        }
    }
}

// TODO: use a self updating token from auth crate
#[derive(Clone)]
pub struct BadgeManager {
    token: twitch_oauth2::UserToken,
    pub global_badges: SharedMap<BadgeSet>,
    pub scoped_badges: SharedMap<Scope<BadgeSet>>,
}

impl BadgeManager {
    pub async fn new(
        client: HelixClient<'static, reqwest::Client>,
        token: twitch_oauth2::UserToken,
    ) -> Result<BadgeManager, String> {
        let mut global_badges: HashMap<String, BadgeSet> = Default::default();

        let req = get_global_chat_badges::GetGlobalChatBadgesRequest::new();

        debug!("getting global badges");
        let response: Vec<twitch_api::helix::chat::BadgeSet> =
            match client.req_get(req, &token).await {
                Ok(resp) => resp.data,
                Err(err) => return Err(err.to_string()),
            };

        let _: Vec<_> = response
            .iter()
            .map(|b| {
                let new_b = BadgeSet::from(b.clone());
                debug!("adding badgeset: badgeset={:?}", b);
                global_badges.insert(b.set_id.to_string(), new_b.clone());
            })
            .collect();

        Ok(BadgeManager {
            token: token.clone(),
            global_badges: Arc::new(Mutex::new(global_badges)),
            scoped_badges: Default::default(),
        })
    }

    pub async fn load_channel(
        self,
        broadcaster_id: String,
        client: HelixClient<'static, reqwest::Client>,
    ) {
        debug!(broadcaster_id, "loading channel");
        let mut scoped_badges = self.scoped_badges.lock().await;

        if !scoped_badges.contains_key(&broadcaster_id) {
            let mut badges: HashMap<String, BadgeSet> = Default::default();
            debug!(
                broadcaster_id,
                "channel doesn't exist in cache yet, loading"
            );
            let req = get_channel_chat_badges::GetChannelChatBadgesRequest::broadcaster_id(
                broadcaster_id.clone(),
            );

            debug!(broadcaster_id, "getting badges");
            let response: Vec<twitch_api::helix::chat::BadgeSet> =
                match client.req_get(req, &self.token).await {
                    Ok(resp) => resp.data,
                    Err(err) => {
                        error!(
                            broadcaster_id,
                            "failed to load channel badges: err={}",
                            err.to_string()
                        );
                        return;
                    }
                };

            let _: Vec<_> = response
                .iter()
                .map(|b| {
                    let new_b = BadgeSet::from(b.clone());
                    debug!(broadcaster_id, "adding badgeset: badgeset={:?}", b);
                    badges.insert(new_b.set_id.clone(), new_b.clone());
                })
                .collect();

            scoped_badges.insert(broadcaster_id, badges);
        }
    }

    pub async fn get(self, set_id: String, channel: String) -> Option<BadgeSet> {
        let global_badges = self.global_badges.lock().await;
        let scoped_badges = self.scoped_badges.lock().await;

        match scoped_badges.get(&channel) {
            Some(scope) => match scope.get(&set_id) {
                Some(badge_set) => return Some(badge_set.clone()),
                None => trace!(set_id, channel, "set doesn't exist in scope"),
            },
            None => debug!(set_id, channel, "channel scope doesn't exist"),
        }
        global_badges.get(&set_id).cloned()
    }
}

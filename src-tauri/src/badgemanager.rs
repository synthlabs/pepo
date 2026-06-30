use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::{debug, error, trace};

use crate::{
    badgepersist::{SharedBadgeMetadataStore, TauriBadgeMetadataStore},
    emote::providers::GLOBAL_SCOPE_KEY,
    token::TokenManager,
    types::ProviderSettings,
};
use twitch_api::helix::chat::{get_channel_chat_badges, get_global_chat_badges};

type SharedMap<V> = Arc<Mutex<HashMap<String, V>>>;
type Scope<T> = HashMap<String, T>;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default, PartialEq)]
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

#[derive(Clone)]
pub struct BadgeManager {
    token_manager: Option<TokenManager>,
    persistence: SharedBadgeMetadataStore,
    pub global_badges: SharedMap<BadgeSet>,
    pub scoped_badges: SharedMap<Scope<BadgeSet>>,
}

impl BadgeManager {
    pub fn empty(token_manager: TokenManager, app_handle: tauri::AppHandle) -> BadgeManager {
        BadgeManager {
            token_manager: Some(token_manager),
            persistence: Arc::new(TauriBadgeMetadataStore::new(app_handle)),
            global_badges: Arc::new(Mutex::new(HashMap::new())),
            scoped_badges: Default::default(),
        }
    }

    #[cfg(test)]
    fn with_persistence_for_test(persistence: SharedBadgeMetadataStore) -> BadgeManager {
        BadgeManager {
            token_manager: None,
            persistence,
            global_badges: Arc::new(Mutex::new(HashMap::new())),
            scoped_badges: Default::default(),
        }
    }

    pub async fn hydrate_global(&self, provider_settings: &ProviderSettings) {
        if !self.global_badges.lock().await.is_empty() {
            return;
        }

        let Some(hydrated) = self
            .persistence
            .load_scope(GLOBAL_SCOPE_KEY, provider_settings)
        else {
            return;
        };
        let count = hydrated.count();
        let age_seconds = hydrated.age_seconds();
        *self.global_badges.lock().await = badge_sets_to_scope(hydrated.badge_sets);
        debug!(
            count,
            age_seconds, "hydrated persisted global badge metadata"
        );
    }

    pub async fn hydrate_channel(
        &self,
        broadcaster_id: &str,
        provider_settings: &ProviderSettings,
    ) {
        {
            let scoped_badges = self.scoped_badges.lock().await;
            if scoped_badges.contains_key(broadcaster_id) {
                return;
            }
        }

        let Some(hydrated) = self
            .persistence
            .load_scope(broadcaster_id, provider_settings)
        else {
            return;
        };
        let count = hydrated.count();
        let age_seconds = hydrated.age_seconds();
        self.scoped_badges.lock().await.insert(
            broadcaster_id.to_string(),
            badge_sets_to_scope(hydrated.badge_sets),
        );
        debug!(
            broadcaster_id,
            count, age_seconds, "hydrated persisted channel badge metadata"
        );
    }

    pub async fn load_global(
        &self,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        provider_settings: &ProviderSettings,
    ) -> Result<(), String> {
        self.hydrate_global(provider_settings).await;

        let req = get_global_chat_badges::GetGlobalChatBadgesRequest::new();

        debug!("getting global badges");
        let Some(token_manager) = &self.token_manager else {
            return Err("no active token".to_owned());
        };
        let Some(token) = token_manager.active_twitch_token().await else {
            return Err("no active token".to_owned());
        };
        let response: Vec<twitch_api::helix::chat::BadgeSet> =
            match client.req_get(req, &token).await {
                Ok(resp) => resp.data,
                Err(err) => return Err(err.to_string()),
            };

        let badge_sets = response
            .into_iter()
            .map(|b| {
                debug!("adding badgeset: badgeset={:?}", b);
                BadgeSet::from(b)
            })
            .collect();
        self.store_global_badges(badge_sets, provider_settings)
            .await;

        Ok(())
    }

    pub async fn load_channel(
        self,
        broadcaster_id: String,
        client: twitch_api::HelixClient<'static, reqwest::Client>,
        provider_settings: &ProviderSettings,
    ) {
        debug!(broadcaster_id, "loading channel");
        self.hydrate_channel(&broadcaster_id, provider_settings)
            .await;

        let mut badges: HashMap<String, BadgeSet> = Default::default();
        debug!(broadcaster_id, "refreshing channel badges");
        let req = get_channel_chat_badges::GetChannelChatBadgesRequest::broadcaster_id(
            broadcaster_id.clone(),
        );

        debug!(broadcaster_id, "getting badges");
        let Some(token_manager) = &self.token_manager else {
            error!(
                broadcaster_id,
                "failed to load channel badges: no active token"
            );
            return;
        };
        let Some(token) = token_manager.active_twitch_token().await else {
            error!(
                broadcaster_id,
                "failed to load channel badges: no active token"
            );
            return;
        };
        let response: Vec<twitch_api::helix::chat::BadgeSet> =
            match client.req_get(req, &token).await {
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

        for b in response {
            let new_b = BadgeSet::from(b.clone());
            debug!(broadcaster_id, "adding badgeset: badgeset={:?}", b);
            badges.insert(new_b.set_id.clone(), new_b.clone());
        }

        self.store_channel_badges(
            broadcaster_id,
            scope_to_badge_sets(badges),
            provider_settings,
        )
        .await;
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

    async fn store_global_badges(
        &self,
        badge_sets: Vec<BadgeSet>,
        provider_settings: &ProviderSettings,
    ) {
        self.persistence
            .save_scope(GLOBAL_SCOPE_KEY, badge_sets.clone(), provider_settings);
        *self.global_badges.lock().await = badge_sets_to_scope(badge_sets);
    }

    async fn store_channel_badges(
        &self,
        broadcaster_id: String,
        badge_sets: Vec<BadgeSet>,
        provider_settings: &ProviderSettings,
    ) {
        self.persistence
            .save_scope(&broadcaster_id, badge_sets.clone(), provider_settings);
        self.scoped_badges
            .lock()
            .await
            .insert(broadcaster_id, badge_sets_to_scope(badge_sets));
    }
}

fn badge_sets_to_scope(badge_sets: Vec<BadgeSet>) -> Scope<BadgeSet> {
    badge_sets
        .into_iter()
        .map(|badge_set| (badge_set.set_id.clone(), badge_set))
        .collect()
}

fn scope_to_badge_sets(scope: Scope<BadgeSet>) -> Vec<BadgeSet> {
    scope.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::badgepersist::{BadgeMetadataStore, MemoryBadgeMetadataStore};

    const NOW: u64 = 1_800_000_000;

    fn badge_set(set_id: &str, version_id: &str, url: &str) -> BadgeSet {
        BadgeSet {
            set_id: set_id.to_string(),
            versions: vec![Badge {
                set_id: set_id.to_string(),
                id: version_id.to_string(),
                image_url_4x: url.to_string(),
                title: set_id.to_string(),
                ..Default::default()
            }],
        }
    }

    #[tokio::test]
    async fn hydrate_global_uses_persisted_badges() {
        let persistence = MemoryBadgeMetadataStore::new(NOW);
        persistence.insert(
            GLOBAL_SCOPE_KEY,
            vec![badge_set("moderator", "1", "https://example.com/mod.png")],
            NOW,
        );
        let manager = BadgeManager::with_persistence_for_test(persistence);

        manager.hydrate_global(&ProviderSettings::default()).await;

        let badge_set = manager
            .clone()
            .get("moderator".to_string(), "1234".to_string())
            .await
            .unwrap();
        let badge = badge_set.version("1".to_string()).unwrap();
        assert_eq!(badge.image_url_4x, "https://example.com/mod.png");
    }

    #[tokio::test]
    async fn hydrate_channel_uses_persisted_badges_before_refresh() {
        let persistence = MemoryBadgeMetadataStore::new(NOW);
        persistence.insert(
            "1234",
            vec![badge_set(
                "subscriber",
                "12",
                "https://example.com/sub-12.png",
            )],
            NOW,
        );
        let manager = BadgeManager::with_persistence_for_test(persistence);

        manager
            .hydrate_channel("1234", &ProviderSettings::default())
            .await;

        let badge_set = manager
            .clone()
            .get("subscriber".to_string(), "1234".to_string())
            .await
            .unwrap();
        let badge = badge_set.version("12".to_string()).unwrap();
        assert_eq!(badge.image_url_4x, "https://example.com/sub-12.png");
    }

    #[tokio::test]
    async fn channel_load_without_token_keeps_hydrated_badges() {
        let persistence = MemoryBadgeMetadataStore::new(NOW);
        persistence.insert(
            "1234",
            vec![badge_set(
                "subscriber",
                "12",
                "https://example.com/cached-sub.png",
            )],
            NOW,
        );
        let manager = BadgeManager::with_persistence_for_test(persistence);
        let client = twitch_api::HelixClient::with_client(reqwest::Client::new());

        manager
            .clone()
            .load_channel("1234".to_string(), client, &ProviderSettings::default())
            .await;

        let badge_set = manager
            .clone()
            .get("subscriber".to_string(), "1234".to_string())
            .await
            .unwrap();
        let badge = badge_set.version("12".to_string()).unwrap();
        assert_eq!(badge.image_url_4x, "https://example.com/cached-sub.png");
    }

    #[tokio::test]
    async fn successful_empty_channel_refresh_replaces_cached_badges() {
        let persistence = MemoryBadgeMetadataStore::new(NOW);
        persistence.insert(
            "1234",
            vec![badge_set(
                "subscriber",
                "12",
                "https://example.com/cached-sub.png",
            )],
            NOW,
        );
        let manager = BadgeManager::with_persistence_for_test(persistence.clone());
        manager
            .hydrate_channel("1234", &ProviderSettings::default())
            .await;

        manager
            .store_channel_badges("1234".to_string(), Vec::new(), &ProviderSettings::default())
            .await;

        assert!(manager
            .clone()
            .get("subscriber".to_string(), "1234".to_string())
            .await
            .is_none());
        let persisted = persistence
            .load_scope("1234", &ProviderSettings::default())
            .unwrap();
        assert_eq!(persisted.count(), 0);
    }
}

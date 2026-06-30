#[cfg(test)]
use std::{collections::HashMap, sync::Mutex};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use tracing::{debug, error, warn};

use crate::{badgemanager::BadgeSet, emote::providers::GLOBAL_SCOPE_KEY, types::ProviderSettings};

const STORE_FILE: &str = "badge-cache.json";
const SCHEMA_VERSION: u32 = 1;

pub(crate) type SharedBadgeMetadataStore = Arc<dyn BadgeMetadataStore>;

pub(crate) trait BadgeMetadataStore: Send + Sync {
    fn load_scope(
        &self,
        scope_key: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedBadgeScope>;

    fn save_scope(
        &self,
        scope_key: &str,
        badge_sets: Vec<BadgeSet>,
        provider_settings: &ProviderSettings,
    );
}

pub(crate) struct HydratedBadgeScope {
    pub badge_sets: Vec<BadgeSet>,
    pub saved_at_unix_secs: u64,
    pub loaded_at_unix_secs: u64,
}

impl HydratedBadgeScope {
    pub fn count(&self) -> usize {
        self.badge_sets.len()
    }

    pub fn age_seconds(&self) -> u64 {
        self.loaded_at_unix_secs
            .saturating_sub(self.saved_at_unix_secs)
    }
}

#[derive(Clone)]
pub(crate) struct TauriBadgeMetadataStore {
    app: AppHandle,
}

impl TauriBadgeMetadataStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl BadgeMetadataStore for TauriBadgeMetadataStore {
    fn load_scope(
        &self,
        scope_key: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedBadgeScope> {
        let key = cache_key(scope_key);
        let store = match self.app.store(STORE_FILE) {
            Ok(store) => store,
            Err(err) => {
                error!(%key, "failed to open badge metadata store: {err}");
                return None;
            }
        };

        let value = store.get(&key)?;
        let payload = match serde_json::from_value::<StoredBadgeScope>(value) {
            Ok(payload) => payload,
            Err(err) => {
                warn!(%key, "failed to decode persisted badge metadata: {err}");
                return None;
            }
        };

        let now = unix_now();
        match payload.validate_for(scope_key, now, provider_settings) {
            Ok(()) => Some(payload.into_hydrated(now)),
            Err(LoadRejection::Expired) => {
                debug!(%key, "deleting expired persisted badge metadata");
                store.delete(&key);
                if let Err(err) = store.save() {
                    warn!(%key, "failed to save badge metadata store after delete: {err}");
                }
                None
            }
            Err(reason) => {
                warn!(%key, ?reason, "ignoring invalid persisted badge metadata");
                None
            }
        }
    }

    fn save_scope(
        &self,
        scope_key: &str,
        badge_sets: Vec<BadgeSet>,
        provider_settings: &ProviderSettings,
    ) {
        let key = cache_key(scope_key);
        let store = match self.app.store(STORE_FILE) {
            Ok(store) => store,
            Err(err) => {
                error!(%key, "failed to open badge metadata store: {err}");
                return;
            }
        };

        let now = unix_now();
        let payload = StoredBadgeScope::new(scope_key.to_string(), badge_sets, now);
        store.set(&key, serde_json::json!(payload));
        prune_expired_channel_entries(&store, now, provider_settings);

        if let Err(err) = store.save() {
            error!(%key, "failed to save badge metadata store: {err}");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StoredBadgeScope {
    schema_version: u32,
    scope_key: String,
    saved_at_unix_secs: u64,
    badge_sets: Vec<BadgeSet>,
}

impl StoredBadgeScope {
    fn new(scope_key: String, badge_sets: Vec<BadgeSet>, saved_at_unix_secs: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            scope_key,
            saved_at_unix_secs,
            badge_sets,
        }
    }

    fn validate_for(
        &self,
        scope_key: &str,
        now_unix_secs: u64,
        provider_settings: &ProviderSettings,
    ) -> Result<(), LoadRejection> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(LoadRejection::SchemaVersion);
        }
        if self.scope_key != scope_key {
            return Err(LoadRejection::Scope);
        }
        if is_expired_channel_scope(
            &self.scope_key,
            self.saved_at_unix_secs,
            now_unix_secs,
            provider_settings,
        ) {
            return Err(LoadRejection::Expired);
        }
        Ok(())
    }

    fn into_hydrated(self, loaded_at_unix_secs: u64) -> HydratedBadgeScope {
        HydratedBadgeScope {
            badge_sets: self.badge_sets,
            saved_at_unix_secs: self.saved_at_unix_secs,
            loaded_at_unix_secs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadRejection {
    SchemaVersion,
    Scope,
    Expired,
}

fn cache_key(scope_key: &str) -> String {
    format!("badge_cache:v{SCHEMA_VERSION}:{scope_key}")
}

fn is_expired_channel_scope(
    scope_key: &str,
    saved_at_unix_secs: u64,
    now_unix_secs: u64,
    provider_settings: &ProviderSettings,
) -> bool {
    provider_settings.metadata_retention_enabled
        && scope_key != GLOBAL_SCOPE_KEY
        && now_unix_secs.saturating_sub(saved_at_unix_secs)
            > provider_settings.metadata_retention_secs
}

fn prune_expired_channel_entries(
    store: &tauri_plugin_store::Store<tauri::Wry>,
    now_unix_secs: u64,
    provider_settings: &ProviderSettings,
) {
    let prefix = format!("badge_cache:v{SCHEMA_VERSION}:");
    for key in store
        .keys()
        .into_iter()
        .filter(|key| key.starts_with(&prefix))
    {
        let Some(value) = store.get(&key) else {
            continue;
        };
        let Ok(payload) = serde_json::from_value::<StoredBadgeScope>(value) else {
            continue;
        };
        if is_expired_channel_scope(
            &payload.scope_key,
            payload.saved_at_unix_secs,
            now_unix_secs,
            provider_settings,
        ) {
            debug!(%key, "pruning expired badge metadata");
            store.delete(key);
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) struct MemoryBadgeMetadataStore {
    now_unix_secs: Mutex<u64>,
    entries: Mutex<HashMap<String, StoredBadgeScope>>,
}

#[cfg(test)]
impl MemoryBadgeMetadataStore {
    pub(crate) fn new(now_unix_secs: u64) -> Arc<Self> {
        Arc::new(Self {
            now_unix_secs: Mutex::new(now_unix_secs),
            entries: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn insert(
        &self,
        scope_key: &str,
        badge_sets: Vec<BadgeSet>,
        saved_at_unix_secs: u64,
    ) {
        self.entries.lock().unwrap().insert(
            cache_key(scope_key),
            StoredBadgeScope::new(scope_key.to_string(), badge_sets, saved_at_unix_secs),
        );
    }

    pub(crate) fn has_scope(&self, scope_key: &str) -> bool {
        self.entries
            .lock()
            .unwrap()
            .contains_key(&cache_key(scope_key))
    }
}

#[cfg(test)]
impl BadgeMetadataStore for MemoryBadgeMetadataStore {
    fn load_scope(
        &self,
        scope_key: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedBadgeScope> {
        let key = cache_key(scope_key);
        let now = *self.now_unix_secs.lock().unwrap();
        let payload = self.entries.lock().unwrap().get(&key).cloned()?;
        match payload.validate_for(scope_key, now, provider_settings) {
            Ok(()) => Some(payload.into_hydrated(now)),
            Err(LoadRejection::Expired) => {
                self.entries.lock().unwrap().remove(&key);
                None
            }
            Err(_) => None,
        }
    }

    fn save_scope(
        &self,
        scope_key: &str,
        badge_sets: Vec<BadgeSet>,
        provider_settings: &ProviderSettings,
    ) {
        let now = *self.now_unix_secs.lock().unwrap();
        self.insert(scope_key, badge_sets, now);

        let keys = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(key, payload)| {
                is_expired_channel_scope(
                    &payload.scope_key,
                    payload.saved_at_unix_secs,
                    now,
                    provider_settings,
                )
                .then_some(key.clone())
            })
            .collect::<Vec<_>>();

        let mut entries = self.entries.lock().unwrap();
        for key in keys {
            entries.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::badgemanager::Badge;

    const NOW: u64 = 1_800_000_000;

    fn badge_set(set_id: &str, version_id: &str) -> BadgeSet {
        BadgeSet {
            set_id: set_id.to_string(),
            versions: vec![Badge {
                set_id: set_id.to_string(),
                id: version_id.to_string(),
                image_url_4x: format!("https://example.com/{set_id}-{version_id}.png"),
                title: set_id.to_string(),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn stored_scope_round_trips_through_json() {
        let payload = StoredBadgeScope::new(
            GLOBAL_SCOPE_KEY.to_string(),
            vec![badge_set("subscriber", "12")],
            NOW,
        );

        let value = serde_json::to_value(&payload).unwrap();
        let decoded: StoredBadgeScope = serde_json::from_value(value).unwrap();

        assert_eq!(decoded, payload);
        decoded
            .validate_for(GLOBAL_SCOPE_KEY, NOW, &ProviderSettings::default())
            .unwrap();
    }

    #[test]
    fn channel_metadata_expires_after_retention() {
        let payload =
            StoredBadgeScope::new("1234".to_string(), vec![badge_set("subscriber", "12")], NOW);

        assert_eq!(
            payload.validate_for(
                "1234",
                NOW + ProviderSettings::default().metadata_retention_secs + 1,
                &ProviderSettings::default(),
            ),
            Err(LoadRejection::Expired)
        );
    }

    #[test]
    fn channel_metadata_does_not_expire_when_retention_is_disabled() {
        let payload =
            StoredBadgeScope::new("1234".to_string(), vec![badge_set("subscriber", "12")], NOW);
        let settings = ProviderSettings {
            metadata_retention_enabled: false,
            ..Default::default()
        };

        payload
            .validate_for(
                "1234",
                NOW + ProviderSettings::default().metadata_retention_secs + 1,
                &settings,
            )
            .unwrap();
    }

    #[test]
    fn global_metadata_does_not_expire() {
        let payload = StoredBadgeScope::new(
            GLOBAL_SCOPE_KEY.to_string(),
            vec![badge_set("subscriber", "12")],
            NOW,
        );

        payload
            .validate_for(
                GLOBAL_SCOPE_KEY,
                NOW + ProviderSettings::default().metadata_retention_secs + 1,
                &ProviderSettings::default(),
            )
            .unwrap();
    }

    #[test]
    fn schema_mismatch_is_rejected() {
        let mut payload = StoredBadgeScope::new(
            GLOBAL_SCOPE_KEY.to_string(),
            vec![badge_set("subscriber", "12")],
            NOW,
        );
        payload.schema_version = SCHEMA_VERSION + 1;

        assert_eq!(
            payload.validate_for(GLOBAL_SCOPE_KEY, NOW, &ProviderSettings::default()),
            Err(LoadRejection::SchemaVersion)
        );
    }

    #[test]
    fn memory_store_prunes_expired_channel_entries_on_save() {
        let persistence = MemoryBadgeMetadataStore::new(NOW);
        persistence.insert(
            "old-channel",
            vec![badge_set("subscriber", "12")],
            NOW - ProviderSettings::default().metadata_retention_secs - 1,
        );
        persistence.insert(GLOBAL_SCOPE_KEY, vec![badge_set("global", "1")], NOW);

        persistence.save_scope(
            "fresh-channel",
            vec![badge_set("subscriber", "24")],
            &ProviderSettings::default(),
        );

        assert!(!persistence.has_scope("old-channel"));
        assert!(persistence.has_scope(GLOBAL_SCOPE_KEY));
        assert!(persistence.has_scope("fresh-channel"));
    }
}

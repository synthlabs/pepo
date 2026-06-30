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

use crate::{
    emote::{cache::EmoteCache, providers::GLOBAL_SCOPE_KEY, Emote},
    types::{EmoteProviderId, ProviderSettings},
};

const STORE_FILE: &str = "emote-cache.json";
const SCHEMA_VERSION: u32 = 1;

pub(crate) type SharedEmoteMetadataStore = Arc<dyn EmoteMetadataStore>;

pub(crate) trait EmoteMetadataStore: Send + Sync {
    fn load_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        provider_name: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedEmoteCache>;

    fn save_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        cache: &EmoteCache,
        provider_settings: &ProviderSettings,
    );
}

pub(crate) struct HydratedEmoteCache {
    pub cache: EmoteCache,
    pub saved_at_unix_secs: u64,
    pub loaded_at_unix_secs: u64,
}

impl HydratedEmoteCache {
    pub fn count(&self) -> usize {
        self.cache.len()
    }

    pub fn age_seconds(&self) -> u64 {
        self.loaded_at_unix_secs
            .saturating_sub(self.saved_at_unix_secs)
    }
}

#[derive(Clone)]
pub(crate) struct TauriEmoteMetadataStore {
    app: AppHandle,
}

impl TauriEmoteMetadataStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EmoteMetadataStore for TauriEmoteMetadataStore {
    fn load_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        provider_name: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedEmoteCache> {
        let key = cache_key(provider_id, scope_key);
        let store = match self.app.store(STORE_FILE) {
            Ok(store) => store,
            Err(err) => {
                error!(%key, "failed to open emote metadata store: {err}");
                return None;
            }
        };

        let value = store.get(&key)?;
        let payload = match serde_json::from_value::<StoredEmoteCache>(value) {
            Ok(payload) => payload,
            Err(err) => {
                warn!(%key, "failed to decode persisted emote metadata: {err}");
                return None;
            }
        };

        let now = unix_now();
        match payload.validate_for(provider_id, scope_key, now, provider_settings) {
            Ok(()) => Some(payload.into_hydrated(provider_name.to_string(), now)),
            Err(LoadRejection::Expired) => {
                debug!(%key, "deleting expired persisted emote metadata");
                store.delete(&key);
                if let Err(err) = store.save() {
                    warn!(%key, "failed to save emote metadata store after delete: {err}");
                }
                None
            }
            Err(reason) => {
                warn!(%key, ?reason, "ignoring invalid persisted emote metadata");
                None
            }
        }
    }

    fn save_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        cache: &EmoteCache,
        provider_settings: &ProviderSettings,
    ) {
        let key = cache_key(provider_id, scope_key);
        let store = match self.app.store(STORE_FILE) {
            Ok(store) => store,
            Err(err) => {
                error!(%key, "failed to open emote metadata store: {err}");
                return;
            }
        };

        let payload = StoredEmoteCache::new(
            provider_id,
            scope_key.to_string(),
            cache.emotes(),
            unix_now(),
        );
        store.set(&key, serde_json::json!(payload));
        prune_expired_channel_entries(&store, provider_id, unix_now(), provider_settings);

        if let Err(err) = store.save() {
            error!(%key, "failed to save emote metadata store: {err}");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StoredEmoteCache {
    schema_version: u32,
    provider: EmoteProviderId,
    scope_key: String,
    saved_at_unix_secs: u64,
    emotes: Vec<Emote>,
}

impl StoredEmoteCache {
    fn new(
        provider: EmoteProviderId,
        scope_key: String,
        emotes: Vec<Emote>,
        saved_at_unix_secs: u64,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            provider,
            scope_key,
            saved_at_unix_secs,
            emotes,
        }
    }

    fn validate_for(
        &self,
        provider: EmoteProviderId,
        scope_key: &str,
        now_unix_secs: u64,
        provider_settings: &ProviderSettings,
    ) -> Result<(), LoadRejection> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(LoadRejection::SchemaVersion);
        }
        if self.provider != provider {
            return Err(LoadRejection::Provider);
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

    fn into_hydrated(self, provider_name: String, loaded_at_unix_secs: u64) -> HydratedEmoteCache {
        HydratedEmoteCache {
            cache: EmoteCache::from_emotes(self.scope_key, provider_name, self.emotes),
            saved_at_unix_secs: self.saved_at_unix_secs,
            loaded_at_unix_secs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadRejection {
    SchemaVersion,
    Provider,
    Scope,
    Expired,
}

fn cache_key(provider_id: EmoteProviderId, scope_key: &str) -> String {
    format!(
        "emote_cache:v{SCHEMA_VERSION}:{}:{scope_key}",
        provider_key(provider_id)
    )
}

fn provider_key(provider_id: EmoteProviderId) -> &'static str {
    match provider_id {
        EmoteProviderId::Twitch => "twitch",
        EmoteProviderId::Bttv => "bttv",
        EmoteProviderId::Ffz => "ffz",
        EmoteProviderId::Seventv => "seventv",
    }
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
    provider_id: EmoteProviderId,
    now_unix_secs: u64,
    provider_settings: &ProviderSettings,
) {
    let prefix = format!(
        "emote_cache:v{SCHEMA_VERSION}:{}:",
        provider_key(provider_id)
    );
    for key in store
        .keys()
        .into_iter()
        .filter(|key| key.starts_with(&prefix))
    {
        let Some(value) = store.get(&key) else {
            continue;
        };
        let Ok(payload) = serde_json::from_value::<StoredEmoteCache>(value) else {
            continue;
        };
        if is_expired_channel_scope(
            &payload.scope_key,
            payload.saved_at_unix_secs,
            now_unix_secs,
            provider_settings,
        ) {
            debug!(%key, "pruning expired emote metadata");
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
pub(crate) struct MemoryEmoteMetadataStore {
    now_unix_secs: Mutex<u64>,
    entries: Mutex<HashMap<String, StoredEmoteCache>>,
}

#[cfg(test)]
impl MemoryEmoteMetadataStore {
    pub(crate) fn new(now_unix_secs: u64) -> Arc<Self> {
        Arc::new(Self {
            now_unix_secs: Mutex::new(now_unix_secs),
            entries: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn insert(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        emotes: Vec<Emote>,
        saved_at_unix_secs: u64,
    ) {
        self.entries.lock().unwrap().insert(
            cache_key(provider_id, scope_key),
            StoredEmoteCache::new(
                provider_id,
                scope_key.to_string(),
                emotes,
                saved_at_unix_secs,
            ),
        );
    }
}

#[cfg(test)]
impl EmoteMetadataStore for MemoryEmoteMetadataStore {
    fn load_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        provider_name: &str,
        provider_settings: &ProviderSettings,
    ) -> Option<HydratedEmoteCache> {
        let key = cache_key(provider_id, scope_key);
        let now = *self.now_unix_secs.lock().unwrap();
        let payload = self.entries.lock().unwrap().get(&key).cloned()?;
        match payload.validate_for(provider_id, scope_key, now, provider_settings) {
            Ok(()) => Some(payload.into_hydrated(provider_name.to_string(), now)),
            Err(LoadRejection::Expired) => {
                self.entries.lock().unwrap().remove(&key);
                None
            }
            Err(_) => None,
        }
    }

    fn save_cache(
        &self,
        provider_id: EmoteProviderId,
        scope_key: &str,
        cache: &EmoteCache,
        _provider_settings: &ProviderSettings,
    ) {
        let now = *self.now_unix_secs.lock().unwrap();
        self.insert(provider_id, scope_key, cache.emotes(), now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emote::Emote;

    const NOW: u64 = 1_800_000_000;

    fn emote(name: &str) -> Emote {
        Emote {
            id: format!("id-{name}"),
            name: name.to_string(),
            provider: "7TV".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn stored_cache_round_trips_through_json() {
        let payload = StoredEmoteCache::new(
            EmoteProviderId::Seventv,
            GLOBAL_SCOPE_KEY.to_string(),
            vec![emote("LUL")],
            NOW,
        );

        let value = serde_json::to_value(&payload).unwrap();
        let decoded: StoredEmoteCache = serde_json::from_value(value).unwrap();

        assert_eq!(decoded, payload);
        decoded
            .validate_for(
                EmoteProviderId::Seventv,
                GLOBAL_SCOPE_KEY,
                NOW,
                &ProviderSettings::default(),
            )
            .unwrap();
    }

    #[test]
    fn channel_metadata_expires_after_retention() {
        let payload = StoredEmoteCache::new(
            EmoteProviderId::Bttv,
            "1234".to_string(),
            vec![emote("LUL")],
            NOW,
        );

        assert_eq!(
            payload.validate_for(
                EmoteProviderId::Bttv,
                "1234",
                NOW + ProviderSettings::default().metadata_retention_secs + 1,
                &ProviderSettings::default(),
            ),
            Err(LoadRejection::Expired)
        );
    }

    #[test]
    fn global_metadata_does_not_expire() {
        let payload = StoredEmoteCache::new(
            EmoteProviderId::Ffz,
            GLOBAL_SCOPE_KEY.to_string(),
            vec![emote("LUL")],
            NOW,
        );

        payload
            .validate_for(
                EmoteProviderId::Ffz,
                GLOBAL_SCOPE_KEY,
                NOW + ProviderSettings::default().metadata_retention_secs + 1,
                &ProviderSettings::default(),
            )
            .unwrap();
    }

    #[test]
    fn schema_mismatch_is_rejected() {
        let mut payload = StoredEmoteCache::new(
            EmoteProviderId::Seventv,
            GLOBAL_SCOPE_KEY.to_string(),
            vec![emote("LUL")],
            NOW,
        );
        payload.schema_version = SCHEMA_VERSION + 1;

        assert_eq!(
            payload.validate_for(
                EmoteProviderId::Seventv,
                GLOBAL_SCOPE_KEY,
                NOW,
                &ProviderSettings::default(),
            ),
            Err(LoadRejection::SchemaVersion)
        );
    }
}

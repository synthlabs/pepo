use crate::types;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use twitch_api::HelixClient;
use twitch_oauth2::{Scope, TwitchToken, UserToken};

const CLIENT_ID: &str = "uyf8apz7jdx3ujc3pboj58vim8c8a6";
const ACCOUNT_STORE_FILE: &str = "account.json";
const TOKENS_KEY: &str = "tokens";
const ACTIVE_USER_ID_KEY: &str = "active_user_id";
const LEGACY_TOKEN_KEY: &str = "token";

lazy_static! {
    static ref DEFAULT_SCOPES: Vec<Scope> = vec![
        Scope::UserReadChat,
        Scope::UserWriteChat,
        Scope::UserReadFollows,
        Scope::UserReadEmotes,
        Scope::UserReadBlockedUsers,
        Scope::UserReadSubscriptions,
    ];
}

#[derive(Clone)]
struct ManagedToken {
    twitch: UserToken,
    public: types::UserToken,
}

#[derive(Clone, Default)]
struct TokenManagerState {
    tokens: BTreeMap<String, ManagedToken>,
    active_user_id: Option<String>,
    loaded: bool,
}

impl TokenManagerState {
    fn persisted(&self) -> StoredTokenState {
        StoredTokenState {
            tokens: self
                .tokens
                .iter()
                .map(|(user_id, token)| (user_id.clone(), token.public.clone()))
                .collect(),
            active_user_id: self.active_user_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct StoredTokenState {
    pub tokens: BTreeMap<String, types::UserToken>,
    pub active_user_id: Option<String>,
}

impl StoredTokenState {
    fn seed_from_auth_token(&mut self, token: types::UserToken) -> bool {
        if token.user_id.is_empty() {
            return false;
        }

        let user_id = token.user_id.clone();
        let changed = self
            .tokens
            .get(&user_id)
            .map(|existing| !public_tokens_match(existing, &token))
            .unwrap_or(true);

        self.tokens.insert(user_id.clone(), token);
        if self.active_user_id.is_none() {
            self.active_user_id = Some(user_id);
            return true;
        }

        changed
    }

    fn repair_active_user_id(&mut self) -> bool {
        let repaired = match self.active_user_id.as_deref() {
            Some(user_id) if self.tokens.contains_key(user_id) => self.active_user_id.clone(),
            _ => self.tokens.keys().next().cloned(),
        };

        if repaired == self.active_user_id {
            return false;
        }

        self.active_user_id = repaired;
        true
    }

    fn remove_active_token(&mut self) -> Option<types::UserToken> {
        let active_user_id = self.active_user_id.clone()?;
        let removed = self.tokens.remove(&active_user_id);
        self.repair_active_user_id();
        removed
    }
}

fn public_tokens_match(left: &types::UserToken, right: &types::UserToken) -> bool {
    left.access_token == right.access_token
        && left.client_id == right.client_id
        && left.login == right.login
        && left.user_id == right.user_id
        && left.refresh_token == right.refresh_token
        && left.expires_in == right.expires_in
        && left.profile_image_url == right.profile_image_url
}

fn public_from_twitch_token(
    token: UserToken,
    existing_profile_image_url: Option<String>,
) -> types::UserToken {
    let mut public = types::UserToken::from_twitch_token(token);
    if let Some(profile_image_url) = existing_profile_image_url {
        public.profile_image_url = profile_image_url;
    }
    public
}

fn new_device_builder() -> twitch_oauth2::DeviceUserTokenBuilder {
    twitch_oauth2::tokens::DeviceUserTokenBuilder::new(
        CLIENT_ID.to_string(),
        DEFAULT_SCOPES.clone(),
    )
}

#[derive(Clone)]
pub struct TokenManager {
    state: Arc<Mutex<TokenManagerState>>,
    client: HelixClient<'static, reqwest::Client>,
    app_handle: tauri::AppHandle,
    builder: Arc<Mutex<twitch_oauth2::DeviceUserTokenBuilder>>,
}

impl TokenManager {
    pub fn new(
        client: HelixClient<'static, reqwest::Client>,
        app_handle: tauri::AppHandle,
    ) -> Self {
        TokenManager {
            state: Arc::new(Mutex::new(TokenManagerState::default())),
            client,
            app_handle,
            builder: Arc::new(Mutex::new(new_device_builder())),
        }
    }

    pub async fn ensure_loaded(&self, seed_token: Option<types::UserToken>) -> Result<(), String> {
        if self.state.lock().await.loaded {
            return Ok(());
        }

        let mut stored = self.read_persisted_state();
        let mut changed = seed_token
            .map(|token| stored.seed_from_auth_token(token))
            .unwrap_or(false);
        changed = stored.repair_active_user_id() || changed;

        if changed {
            self.write_persisted_state(&stored)?;
        }

        let mut runtime_tokens = BTreeMap::new();
        for (user_id, public_token) in &stored.tokens {
            match public_token
                .clone()
                .to_twitch_token(self.client.clone())
                .await
            {
                Ok(twitch_token) => {
                    let token_user_id = twitch_token.user_id.to_string();
                    let public = public_from_twitch_token(
                        twitch_token.clone(),
                        Some(public_token.profile_image_url.clone()),
                    );
                    runtime_tokens.insert(
                        token_user_id,
                        ManagedToken {
                            twitch: twitch_token,
                            public,
                        },
                    );
                }
                Err(err) => warn!(
                    user_id,
                    "failed to restore persisted token into runtime manager: {}", err
                ),
            }
        }

        let active_user_id = stored
            .active_user_id
            .filter(|user_id| runtime_tokens.contains_key(user_id))
            .or_else(|| runtime_tokens.keys().next().cloned());

        let mut state = self.state.lock().await;
        state.tokens = runtime_tokens;
        state.active_user_id = active_user_id;
        state.loaded = true;
        Ok(())
    }

    pub async fn start_device_code_flow(&self) -> twitch_oauth2::id::DeviceCodeResponse {
        debug!("starting device flow");

        let mut build_guard = self.builder.lock().await;
        build_guard.start(&self.client).await.unwrap().clone()
    }

    pub async fn finish_device_code_flow(&self) -> twitch_oauth2::UserToken {
        debug!("finishing device flow");

        let mut build_guard = self.builder.lock().await;
        let token = build_guard
            .wait_for_code(&self.client, tokio::time::sleep)
            .await
            .unwrap();
        drop(build_guard);

        if let Err(err) = self.set_active_twitch_token(token.clone(), None).await {
            error!("failed to persist completed device flow token: {}", err);
        }

        token
    }

    pub async fn is_active_token_valid(&self) -> bool {
        let Some(token) = self.active_twitch_token().await else {
            debug!("active token isn't set");
            return false;
        };

        match token.validate_token(&self.client.clone()).await {
            Ok(_) => true,
            Err(err) => {
                error!("token validation failed: {}", err);
                false
            }
        }
    }

    pub async fn active_twitch_token(&self) -> Option<twitch_oauth2::UserToken> {
        let state = self.state.lock().await;
        let active_user_id = state.active_user_id.as_ref()?;
        state
            .tokens
            .get(active_user_id)
            .map(|token| token.twitch.clone())
    }

    pub async fn active_public_token(&self) -> Option<types::UserToken> {
        let state = self.state.lock().await;
        let active_user_id = state.active_user_id.as_ref()?;
        state
            .tokens
            .get(active_user_id)
            .map(|token| token.public.clone())
    }

    pub async fn set_active_twitch_token(
        &self,
        token: twitch_oauth2::UserToken,
        profile_image_url: Option<String>,
    ) -> Result<types::UserToken, String> {
        let user_id = token.user_id.to_string();
        let (public, persisted) = {
            let mut state = self.state.lock().await;
            let profile_image_url = profile_image_url.or_else(|| {
                state
                    .tokens
                    .get(&user_id)
                    .map(|token| token.public.profile_image_url.clone())
            });
            let public = public_from_twitch_token(token.clone(), profile_image_url);

            state.tokens.insert(
                user_id.clone(),
                ManagedToken {
                    twitch: token,
                    public: public.clone(),
                },
            );
            state.active_user_id = Some(user_id);
            state.loaded = true;

            (public, state.persisted())
        };

        self.write_persisted_state(&persisted)?;
        Ok(public)
    }

    pub async fn update_profile_image(
        &self,
        user_id: &str,
        profile_image_url: String,
    ) -> Result<Option<types::UserToken>, String> {
        let (updated, persisted) = {
            let mut state = self.state.lock().await;
            let is_active = state.active_user_id.as_deref() == Some(user_id);
            let Some(token) = state.tokens.get_mut(user_id) else {
                return Ok(None);
            };

            token.public.profile_image_url = profile_image_url;
            let updated = is_active.then(|| token.public.clone());
            (updated, state.persisted())
        };

        self.write_persisted_state(&persisted)?;
        Ok(updated)
    }

    pub async fn remove_active_token(&self) -> Result<Option<types::UserToken>, String> {
        let (removed, persisted) = {
            let mut state = self.state.lock().await;
            let mut persisted = state.persisted();
            let removed = persisted.remove_active_token();

            state.tokens = state
                .tokens
                .iter()
                .filter(|(user_id, _)| persisted.tokens.contains_key(*user_id))
                .map(|(user_id, token)| (user_id.clone(), token.clone()))
                .collect();
            state.active_user_id = persisted.active_user_id.clone();
            state.loaded = true;

            (removed, persisted)
        };

        self.write_persisted_state(&persisted)?;
        Ok(removed)
    }

    pub async fn refresh_all_tokens(
        &self,
        force_validate: bool,
        refresh_threshold: std::time::Duration,
    ) -> Result<Option<types::UserToken>, String> {
        let snapshots = {
            let state = self.state.lock().await;
            state
                .tokens
                .iter()
                .map(|(user_id, token)| {
                    (
                        user_id.clone(),
                        token.twitch.clone(),
                        token.twitch.access_token.secret().to_string(),
                    )
                })
                .collect::<Vec<_>>()
        };

        let mut updates = Vec::new();
        for (user_id, token, original_access_token) in snapshots {
            if force_validate {
                if let Err(err) = token.validate_token(&self.client.clone()).await {
                    warn!(user_id, "token validation failed (will retry): {}", err);
                }
            }

            let remaining = token.expires_in();
            info!(
                user_id,
                login = %token.login,
                expires_in = ?remaining,
                "token freshness checked"
            );
            if remaining >= refresh_threshold {
                continue;
            }

            info!(user_id, login = %token.login, "refreshing token");
            let mut refreshed = token.clone();
            match refreshed.refresh_token(&self.client).await {
                Ok(_) => updates.push((user_id, original_access_token, refreshed)),
                Err(err) => warn!(
                    user_id,
                    "token refresh failed (will retry next tick): {}", err
                ),
            }
        }

        if updates.is_empty() {
            return Ok(None);
        }

        let (active_update, persisted, changed) = {
            let mut state = self.state.lock().await;
            let active_user_id = state.active_user_id.clone();
            let mut active_update = None;
            let mut changed = false;

            for (user_id, original_access_token, refreshed) in updates {
                let Some(existing) = state.tokens.get_mut(&user_id) else {
                    continue;
                };
                if existing.twitch.access_token.secret() != original_access_token {
                    continue;
                }

                let profile_image_url = existing.public.profile_image_url.clone();
                existing.twitch = refreshed.clone();
                existing.public = public_from_twitch_token(refreshed, Some(profile_image_url));
                changed = true;

                if active_user_id.as_deref() == Some(user_id.as_str()) {
                    active_update = Some(existing.public.clone());
                }
            }

            (active_update, state.persisted(), changed)
        };

        if changed {
            self.write_persisted_state(&persisted)?;
        }

        Ok(active_update)
    }

    fn read_persisted_state(&self) -> StoredTokenState {
        let store = match self.app_handle.store(ACCOUNT_STORE_FILE) {
            Ok(store) => store,
            Err(err) => {
                warn!("failed to open account token store: {}", err);
                return StoredTokenState::default();
            }
        };

        let tokens = store
            .get(TOKENS_KEY)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default();
        let active_user_id = store
            .get(ACTIVE_USER_ID_KEY)
            .and_then(|value| value.as_str().map(ToOwned::to_owned));

        StoredTokenState {
            tokens,
            active_user_id,
        }
    }

    fn write_persisted_state(&self, state: &StoredTokenState) -> Result<(), String> {
        let store = self
            .app_handle
            .store(ACCOUNT_STORE_FILE)
            .map_err(|err| format!("failed to open account token store: {err}"))?;

        store.set(TOKENS_KEY, serde_json::json!(state.tokens));
        if let Some(active_user_id) = &state.active_user_id {
            store.set(
                ACTIVE_USER_ID_KEY,
                serde_json::Value::String(active_user_id.clone()),
            );
        } else {
            store.delete(ACTIVE_USER_ID_KEY);
        }
        store.delete(LEGACY_TOKEN_KEY);
        store
            .save()
            .map_err(|err| format!("failed to save account token store: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_token(user_id: &str, login: &str) -> types::UserToken {
        types::UserToken {
            access_token: format!("access-{user_id}"),
            client_id: "client".to_owned(),
            login: login.to_owned(),
            user_id: user_id.to_owned(),
            refresh_token: Some(format!("refresh-{user_id}")),
            expires_in: 3600,
            profile_image_url: format!("https://example.com/{login}.png"),
        }
    }

    #[test]
    fn seed_from_auth_token_inserts_and_sets_active() {
        let mut state = StoredTokenState::default();
        let changed = state.seed_from_auth_token(public_token("1", "first"));

        assert!(changed);
        assert_eq!(state.active_user_id.as_deref(), Some("1"));
        assert_eq!(state.tokens["1"].login, "first");
    }

    #[test]
    fn repair_active_user_id_uses_first_stored_token() {
        let mut state = StoredTokenState {
            tokens: BTreeMap::from([
                ("2".to_owned(), public_token("2", "second")),
                ("1".to_owned(), public_token("1", "first")),
            ]),
            active_user_id: Some("missing".to_owned()),
        };

        assert!(state.repair_active_user_id());
        assert_eq!(state.active_user_id.as_deref(), Some("1"));
    }

    #[test]
    fn active_only_logout_preserves_inactive_tokens() {
        let mut state = StoredTokenState {
            tokens: BTreeMap::from([
                ("1".to_owned(), public_token("1", "first")),
                ("2".to_owned(), public_token("2", "second")),
            ]),
            active_user_id: Some("1".to_owned()),
        };

        let removed = state.remove_active_token();

        assert_eq!(removed.unwrap().user_id, "1");
        assert!(!state.tokens.contains_key("1"));
        assert!(state.tokens.contains_key("2"));
        assert_eq!(state.active_user_id.as_deref(), Some("2"));
    }

    #[test]
    fn active_only_logout_clears_active_when_no_tokens_remain() {
        let mut state = StoredTokenState {
            tokens: BTreeMap::from([("1".to_owned(), public_token("1", "first"))]),
            active_user_id: Some("1".to_owned()),
        };

        let removed = state.remove_active_token();

        assert_eq!(removed.unwrap().user_id, "1");
        assert!(state.tokens.is_empty());
        assert_eq!(state.active_user_id, None);
    }

    #[test]
    fn seed_from_auth_token_updates_existing_token() {
        let mut existing = public_token("1", "first");
        existing.access_token = "old-access".to_owned();
        let mut state = StoredTokenState {
            tokens: BTreeMap::from([("1".to_owned(), existing)]),
            active_user_id: Some("1".to_owned()),
        };

        assert!(state.seed_from_auth_token(public_token("1", "first")));
        assert_eq!(state.tokens["1"].access_token, "access-1");
        assert_eq!(state.active_user_id.as_deref(), Some("1"));
    }
}

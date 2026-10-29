use crate::types;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{debug, error, info, warn};
use twitch_api::HelixClient;
use twitch_oauth2::tokens::errors::{RefreshTokenError, ValidationError};
use twitch_oauth2::{Scope, TwitchToken, UserToken};

const CLIENT_ID: &str = "uyf8apz7jdx3ujc3pboj58vim8c8a6";
const ACCOUNT_STORE_FILE: &str = "account.json";
const TOKENS_KEY: &str = "tokens";
const ACTIVE_USER_ID_KEY: &str = "active_user_id";
const LEGACY_TOKEN_KEY: &str = "token";
const AUTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TokenIdentity {
    pub user_id: String,
    pub session: u64,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct TokenSnapshot {
    pub identity: TokenIdentity,
    pub token: UserToken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuthIssue {
    category: &'static str,
    status: Option<u16>,
    terminal: bool,
}

impl AuthIssue {
    fn retry(category: &'static str) -> Self {
        Self {
            category,
            status: None,
            terminal: false,
        }
    }

    pub(crate) fn permanent(category: &'static str) -> Self {
        Self {
            terminal: true,
            ..Self::retry(category)
        }
    }
}

impl std::fmt::Display for AuthIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (status={:?})", self.category, self.status)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthFailure {
    pub identity: TokenIdentity,
    pub issue: AuthIssue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryError {
    RetryLater(AuthIssue),
    ReauthenticationRequired(AuthFailure),
    SessionChanged,
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RetryLater(issue) => write!(f, "auth recovery will retry: {issue}"),
            Self::ReauthenticationRequired(failure) => {
                write!(f, "reauthentication required: {}", failure.issue)
            }
            Self::SessionChanged => f.write_str("auth session changed"),
        }
    }
}

impl std::error::Error for RecoveryError {}

fn request_issue(error: &(dyn std::error::Error + 'static)) -> AuthIssue {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<reqwest::Error>() {
            let category = if error.is_timeout() {
                "timeout"
            } else if error.is_connect() {
                "connect"
            } else if error.is_body() {
                "response_body"
            } else {
                "request"
            };
            return AuthIssue {
                status: error.status().map(|status| status.as_u16()),
                ..AuthIssue::retry(category)
            };
        }
        if let Some(error) = error.downcast_ref::<std::io::Error>() {
            if error.kind() == std::io::ErrorKind::TimedOut {
                return AuthIssue::retry("timeout");
            }
        }
        current = error.source();
    }
    AuthIssue::retry("request")
}

fn refresh_issue<E: std::error::Error + Send + Sync + 'static>(
    error: &RefreshTokenError<E>,
) -> AuthIssue {
    match error {
        RefreshTokenError::NoRefreshToken => AuthIssue::permanent("missing_refresh_token"),
        RefreshTokenError::RequestError(error) => request_issue(error),
        RefreshTokenError::RequestParseError(twitch_oauth2::RequestParseError::TwitchError(
            error,
        )) => {
            let status = error.status.as_u16();
            let invalid_client = error.message.to_ascii_lowercase().contains("client")
                || error.error.as_deref() == Some("invalid_client");
            let terminal = (status == 401 && !invalid_client)
                || (status == 400 && error.message.eq_ignore_ascii_case("Invalid refresh token"));
            AuthIssue {
                category: if terminal {
                    "refresh_token_rejected"
                } else {
                    "oauth_response"
                },
                status: Some(status),
                terminal,
            }
        }
        RefreshTokenError::RequestParseError(twitch_oauth2::RequestParseError::Other(status)) => {
            AuthIssue {
                status: Some(status.as_u16()),
                ..AuthIssue::retry("unexpected_response")
            }
        }
        _ => AuthIssue::retry("invalid_refresh_response"),
    }
}

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
    identity: TokenIdentity,
    last_failure: Option<(Instant, AuthIssue)>,
}

#[derive(Clone, Default)]
struct TokenManagerState {
    tokens: BTreeMap<String, ManagedToken>,
    unrestored_tokens: BTreeMap<String, types::UserToken>,
    active_user_id: Option<String>,
    loaded: bool,
    generation: u64,
    dirty: bool,
}

impl TokenManagerState {
    fn next_identity(&mut self, user_id: String, session: Option<u64>) -> TokenIdentity {
        self.generation += 1;
        TokenIdentity {
            user_id,
            session: session.unwrap_or(self.generation),
            generation: self.generation,
        }
    }

    fn active_snapshot(&self) -> Option<TokenSnapshot> {
        let token = self.tokens.get(self.active_user_id.as_ref()?)?;
        Some(TokenSnapshot {
            identity: token.identity.clone(),
            token: token.twitch.clone(),
        })
    }

    fn matches(&self, identity: &TokenIdentity) -> bool {
        self.active_snapshot()
            .is_some_and(|snapshot| snapshot.identity == *identity)
    }

    fn persisted(&self) -> StoredTokenState {
        let mut tokens = self.unrestored_tokens.clone();
        tokens.extend(
            self.tokens
                .iter()
                .map(|(user_id, token)| (user_id.clone(), token.public.clone())),
        );
        StoredTokenState {
            tokens,
            active_user_id: self.active_user_id.clone(),
        }
    }
}

#[derive(Default)]
struct TokenMaintenance {
    gate: Mutex<()>,
}

#[derive(Clone, Copy)]
struct MaintenanceOptions {
    validate: bool,
    force_refresh: bool,
    threshold: Duration,
    retry_interval: Duration,
    active_only: bool,
}

impl TokenMaintenance {
    async fn run<C: twitch_oauth2::client::Client>(
        &self,
        state: &Mutex<TokenManagerState>,
        client: &C,
        identity: &TokenIdentity,
        options: MaintenanceOptions,
        persist: impl Fn(&TokenManagerState) -> Result<(), String>,
    ) -> Result<TokenSnapshot, RecoveryError> {
        let _guard = self.gate.lock().await;
        let snapshot = {
            let mut state = state.lock().await;
            if options.active_only
                && state.active_user_id.as_deref() != Some(identity.user_id.as_str())
            {
                return Err(RecoveryError::SessionChanged);
            }
            if state.dirty {
                persist(&state)
                    .map_err(|_| RecoveryError::RetryLater(AuthIssue::retry("persistence")))?;
                state.dirty = false;
            }
            let token = state
                .tokens
                .get(&identity.user_id)
                .ok_or(RecoveryError::SessionChanged)?;
            if token.identity.session != identity.session {
                return Err(RecoveryError::SessionChanged);
            }
            let snapshot = TokenSnapshot {
                identity: token.identity.clone(),
                token: token.twitch.clone(),
            };
            if token.identity.generation != identity.generation && options.force_refresh {
                return Ok(snapshot);
            }
            if let Some((attempted, issue)) = token.last_failure {
                if issue.terminal {
                    return Err(RecoveryError::ReauthenticationRequired(AuthFailure {
                        identity: snapshot.identity.clone(),
                        issue,
                    }));
                }
                if attempted.elapsed() < options.retry_interval {
                    return Err(RecoveryError::RetryLater(issue));
                }
            }
            snapshot
        };
        let identity = &snapshot.identity;

        let mut rejected = false;
        if options.validate {
            match tokio::time::timeout(AUTH_REQUEST_TIMEOUT, snapshot.token.validate_token(client))
                .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(ValidationError::NotAuthorized)) => rejected = true,
                Ok(Err(error)) => {
                    let issue = match error {
                        ValidationError::Request(ref error) => request_issue(error),
                        _ => AuthIssue::retry("invalid_validation_response"),
                    };
                    warn!(user_id = %identity.user_id, operation = "validate", category = issue.category, status = issue.status, "token validation will retry");
                }
                Err(_) => {
                    warn!(user_id = %identity.user_id, operation = "validate", category = "timeout", "token validation will retry")
                }
            }
        }
        let remaining = snapshot.token.expires_in();
        debug!(user_id = %identity.user_id, expires_in = ?remaining, "token freshness checked");
        if !options.force_refresh
            && !rejected
            && !remaining.is_zero()
            && remaining >= options.threshold
        {
            return Ok(snapshot);
        }

        info!(user_id = %identity.user_id, operation = "refresh", "refreshing token");
        let mut refreshed = snapshot.token.clone();
        let result =
            match tokio::time::timeout(AUTH_REQUEST_TIMEOUT, refreshed.refresh_token(client)).await
            {
                Ok(Ok(())) => Ok(refreshed),
                Ok(Err(error)) => Err(refresh_issue(&error)),
                Err(_) => Err(AuthIssue::retry("timeout")),
            };
        let mut state = state.lock().await;
        if options.active_only && state.active_user_id.as_deref() != Some(identity.user_id.as_str())
        {
            return Err(RecoveryError::SessionChanged);
        }
        let Some(current) = state.tokens.get(&identity.user_id) else {
            return Err(RecoveryError::SessionChanged);
        };
        if current.identity != snapshot.identity {
            return Err(RecoveryError::SessionChanged);
        }
        match result {
            Ok(refreshed) => {
                let identity =
                    state.next_identity(identity.user_id.clone(), Some(identity.session));
                let current = state.tokens.get_mut(&identity.user_id).unwrap();
                current.public = public_from_twitch_token(
                    refreshed.clone(),
                    Some(current.public.profile_image_url.clone()),
                );
                current.twitch = refreshed.clone();
                current.identity = identity.clone();
                current.last_failure = None;
                state.dirty = true;
                persist(&state)
                    .map_err(|_| RecoveryError::RetryLater(AuthIssue::retry("persistence")))?;
                state.dirty = false;
                Ok(TokenSnapshot {
                    identity,
                    token: refreshed,
                })
            }
            Err(issue) => {
                state
                    .tokens
                    .get_mut(&identity.user_id)
                    .unwrap()
                    .last_failure = Some((Instant::now(), issue));
                warn!(user_id = %identity.user_id, operation = "refresh", category = issue.category, status = issue.status, retry = !issue.terminal, "token refresh failed");
                if issue.terminal {
                    Err(RecoveryError::ReauthenticationRequired(AuthFailure {
                        identity: identity.clone(),
                        issue,
                    }))
                } else {
                    Err(RecoveryError::RetryLater(issue))
                }
            }
        }
    }
}

pub(crate) struct AuthSessionGuard<'a> {
    state: MutexGuard<'a, TokenManagerState>,
    manager: &'a TokenManager,
}

impl AuthSessionGuard<'_> {
    pub fn remove_active_token(&mut self) -> Result<(), String> {
        let mut persisted = self.state.persisted();
        persisted.remove_active_token();
        self.state
            .tokens
            .retain(|user_id, _| persisted.tokens.contains_key(user_id));
        self.state.active_user_id = persisted.active_user_id.clone();
        self.state.loaded = true;
        self.state.dirty = true;
        self.manager.write_persisted_state(&persisted)?;
        self.state.dirty = false;
        Ok(())
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
    maintenance: Arc<TokenMaintenance>,
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
            maintenance: Arc::new(TokenMaintenance::default()),
            client,
            app_handle,
            builder: Arc::new(Mutex::new(new_device_builder())),
        }
    }

    pub async fn ensure_loaded(&self, seed_token: Option<types::UserToken>) -> Result<(), String> {
        let _guard = self.maintenance.gate.lock().await;
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
                        token_user_id.clone(),
                        ManagedToken {
                            twitch: twitch_token,
                            public,
                            identity: TokenIdentity {
                                user_id: token_user_id,
                                session: runtime_tokens.len() as u64 + 1,
                                generation: runtime_tokens.len() as u64 + 1,
                            },
                            last_failure: None,
                        },
                    );
                }
                Err(_) => warn!(
                    user_id,
                    "failed to restore persisted token into runtime manager"
                ),
            }
        }

        let active_user_id = stored
            .active_user_id
            .filter(|user_id| runtime_tokens.contains_key(user_id))
            .or_else(|| runtime_tokens.keys().next().cloned());

        let mut state = self.state.lock().await;
        if state.loaded {
            return Ok(());
        }
        state.generation = runtime_tokens.len() as u64;
        state.unrestored_tokens = stored
            .tokens
            .into_iter()
            .filter(|(user_id, _)| !runtime_tokens.contains_key(user_id))
            .collect();
        state.tokens = runtime_tokens;
        state.active_user_id = active_user_id;
        state.loaded = true;
        state.dirty = true;
        self.persist_runtime_state(&state)?;
        state.dirty = false;
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
            Err(_) => {
                error!("token validation failed");
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

    pub(crate) async fn active_snapshot(&self) -> Option<TokenSnapshot> {
        self.state.lock().await.active_snapshot()
    }

    pub(crate) async fn guard_session(
        &self,
        identity: Option<&TokenIdentity>,
    ) -> Option<AuthSessionGuard<'_>> {
        let state = self.state.lock().await;
        if identity.is_some_and(|identity| !state.matches(identity)) {
            return None;
        }
        Some(AuthSessionGuard {
            state,
            manager: self,
        })
    }

    fn maintenance_options(&self) -> MaintenanceOptions {
        let syncer = self.app_handle.state::<crate::StateSyncer>();
        let settings = crate::app_settings(syncer.inner()).auth;
        MaintenanceOptions {
            validate: false,
            force_refresh: false,
            threshold: settings.refresh_threshold(),
            retry_interval: settings.refresh_supervisor_tick(),
            active_only: true,
        }
    }

    pub(crate) async fn ready_token_for_session(
        &self,
        identity: &TokenIdentity,
    ) -> Result<TokenSnapshot, RecoveryError> {
        self.maintain(identity, self.maintenance_options()).await
    }

    pub(crate) async fn recover_rejected_token(
        &self,
        identity: &TokenIdentity,
    ) -> Result<TokenSnapshot, RecoveryError> {
        if self.state.lock().await.active_user_id.as_deref() != Some(identity.user_id.as_str()) {
            return Err(RecoveryError::SessionChanged);
        }
        self.maintain(
            identity,
            MaintenanceOptions {
                force_refresh: true,
                ..self.maintenance_options()
            },
        )
        .await
    }

    pub(crate) async fn revoked_authorization(&self, session: &TokenIdentity) -> RecoveryError {
        match self.active_snapshot().await {
            Some(snapshot)
                if snapshot.identity.user_id == session.user_id
                    && snapshot.identity.session == session.session =>
            {
                RecoveryError::ReauthenticationRequired(AuthFailure {
                    identity: snapshot.identity,
                    issue: AuthIssue::permanent("authorization_revoked"),
                })
            }
            _ => RecoveryError::SessionChanged,
        }
    }

    async fn maintain(
        &self,
        identity: &TokenIdentity,
        options: MaintenanceOptions,
    ) -> Result<TokenSnapshot, RecoveryError> {
        self.maintenance
            .run(&self.state, &self.client, identity, options, |state| {
                self.persist_runtime_state(state)
            })
            .await
    }

    fn persist_runtime_state(&self, state: &TokenManagerState) -> Result<(), String> {
        let result = self.write_persisted_state(&state.persisted());
        let syncer = self.app_handle.state::<crate::StateSyncer>();
        let auth = syncer.snapshot::<types::AuthState>("auth_state");
        if matches!(auth.phase, types::AuthPhase::Authorized) {
            if let Some(active) = state
                .active_user_id
                .as_ref()
                .and_then(|id| state.tokens.get(id))
            {
                if auth
                    .token
                    .as_ref()
                    .is_some_and(|token| token.user_id == active.public.user_id)
                {
                    crate::persist_authorized_token(syncer.inner(), active.public.clone());
                }
            }
        }
        result
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
        let mut state = self.state.lock().await;
        let public = {
            let profile_image_url = profile_image_url.or_else(|| {
                state
                    .tokens
                    .get(&user_id)
                    .map(|token| token.public.profile_image_url.clone())
            });
            let public = public_from_twitch_token(token.clone(), profile_image_url);
            let identity = state.next_identity(user_id.clone(), None);

            state.tokens.insert(
                user_id.clone(),
                ManagedToken {
                    twitch: token,
                    public: public.clone(),
                    identity,
                    last_failure: None,
                },
            );
            state.active_user_id = Some(user_id);
            state.loaded = true;

            public
        };

        state.dirty = true;
        self.persist_runtime_state(&state)?;
        state.dirty = false;
        Ok(public)
    }

    pub async fn update_profile_image(
        &self,
        user_id: &str,
        profile_image_url: String,
    ) -> Result<Option<types::UserToken>, String> {
        let mut state = self.state.lock().await;
        let updated = {
            let is_active = state.active_user_id.as_deref() == Some(user_id);
            let Some(token) = state.tokens.get_mut(user_id) else {
                return Ok(None);
            };

            token.public.profile_image_url = profile_image_url;
            is_active.then(|| token.public.clone())
        };

        state.dirty = true;
        self.persist_runtime_state(&state)?;
        state.dirty = false;
        Ok(updated)
    }

    pub(crate) async fn refresh_all_tokens(
        &self,
        force_validate: bool,
        refresh_threshold: Duration,
    ) -> Vec<AuthFailure> {
        let identities = self
            .state
            .lock()
            .await
            .tokens
            .values()
            .map(|token| token.identity.clone())
            .collect::<Vec<_>>();
        let options = MaintenanceOptions {
            validate: force_validate,
            threshold: refresh_threshold,
            active_only: false,
            ..self.maintenance_options()
        };
        let mut failures = Vec::new();
        for identity in identities {
            if let Err(RecoveryError::ReauthenticationRequired(failure)) =
                self.maintain(&identity, options).await
            {
                failures.push(failure);
            }
        }
        failures
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
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use tauri::http::{Request, Response};
    use tokio::sync::Notify;

    enum ScriptStep {
        Response(u16, &'static str),
        Failure(std::io::ErrorKind),
        Wait(Arc<Notify>, Arc<Notify>, u16, &'static str),
    }

    struct ScriptedClient {
        steps: StdMutex<VecDeque<ScriptStep>>,
        requests: AtomicUsize,
    }

    const ROTATED: &str = r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":14400,"scope":[],"token_type":"bearer"}"#;

    impl ScriptedClient {
        fn new(steps: impl IntoIterator<Item = ScriptStep>) -> Self {
            Self {
                steps: StdMutex::new(steps.into_iter().collect()),
                requests: AtomicUsize::new(0),
            }
        }
    }

    impl twitch_oauth2::client::Client for ScriptedClient {
        type Error = std::io::Error;

        fn req(
            &self,
            _: Request<Vec<u8>>,
        ) -> futures::future::BoxFuture<'_, Result<Response<Vec<u8>>, Self::Error>> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            let step = self
                .steps
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected OAuth request");
            Box::pin(async move {
                let (status, body) = match step {
                    ScriptStep::Response(status, body) => (status, body),
                    ScriptStep::Failure(kind) => return Err(std::io::Error::new(kind, "https://id.twitch.tv/oauth2/token?refresh_token=secret-refresh&access_token=secret-access")),
                    ScriptStep::Wait(started, release, status, body) => {
                        started.notify_one();
                        release.notified().await;
                        (status, body)
                    }
                };
                Ok(Response::builder()
                    .status(status)
                    .header("content-type", "application/json")
                    .body(body.as_bytes().to_vec())
                    .unwrap())
            })
        }
    }

    fn runtime_token(remaining: Duration) -> UserToken {
        UserToken::from_existing_unchecked(
            "old-access",
            Some("old-refresh".into()),
            "client",
            None,
            "first".into(),
            "1".into(),
            Some(vec![]),
            Some(remaining),
        )
    }

    fn runtime_state(remaining: Duration) -> (Mutex<TokenManagerState>, TokenIdentity) {
        let mut state = TokenManagerState::default();
        let identity = state.next_identity("1".to_owned(), None);
        let token = runtime_token(remaining);
        state.tokens.insert(
            "1".to_owned(),
            ManagedToken {
                public: public_from_twitch_token(token.clone(), None),
                twitch: token,
                identity: identity.clone(),
                last_failure: None,
            },
        );
        state.active_user_id = Some("1".to_owned());
        (Mutex::new(state), identity)
    }

    fn options(force_refresh: bool) -> MaintenanceOptions {
        MaintenanceOptions {
            validate: false,
            force_refresh,
            threshold: Duration::from_secs(600),
            retry_interval: Duration::from_secs(15),
            active_only: true,
        }
    }

    #[tokio::test]
    async fn auth_recovery_wake_failure_then_401_preserves_credentials_until_retry() {
        let (state, identity) = runtime_state(Duration::ZERO);
        let maintenance = TokenMaintenance::default();
        let client = ScriptedClient::new([
            ScriptStep::Failure(std::io::ErrorKind::NotConnected),
            ScriptStep::Response(200, ROTATED),
        ]);
        let persisted = StdMutex::new(Vec::new());
        let save = |state: &TokenManagerState| {
            persisted
                .lock()
                .unwrap()
                .push((state.persisted(), state.active_snapshot().unwrap().token));
            Ok(())
        };
        assert!(matches!(
            maintenance
                .run(&state, &client, &identity, options(false), save)
                .await,
            Err(RecoveryError::RetryLater(_))
        ));
        assert!(matches!(
            maintenance
                .run(&state, &client, &identity, options(true), save)
                .await,
            Err(RecoveryError::RetryLater(_))
        ));
        assert_eq!(client.requests.load(Ordering::SeqCst), 1);
        {
            let mut state = state.lock().await;
            let token = state.tokens.get_mut("1").unwrap();
            assert_eq!(
                token.twitch.refresh_token.as_ref().unwrap().secret(),
                "old-refresh"
            );
            let (_, issue) = token.last_failure.unwrap();
            token.last_failure = Some((Instant::now() - Duration::from_secs(16), issue));
        }
        let result = maintenance
            .run(&state, &client, &identity, options(true), save)
            .await
            .unwrap();
        assert_eq!(result.token.access_token.secret(), "new-access");
        let saved = persisted.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0.tokens["1"].access_token, "new-access");
        assert_eq!(
            saved[0].0.tokens["1"].refresh_token.as_deref(),
            Some("new-refresh")
        );
        assert_eq!(
            saved[0].1.refresh_token.as_ref().unwrap().secret(),
            "new-refresh"
        );
    }

    #[tokio::test]
    async fn auth_recovery_concurrent_callers_share_one_rotated_token() {
        let (state, identity) = runtime_state(Duration::ZERO);
        let maintenance = TokenMaintenance::default();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let client = ScriptedClient::new([ScriptStep::Wait(
            started.clone(),
            release.clone(),
            200,
            ROTATED,
        )]);
        let writes = AtomicUsize::new(0);
        let save = |_: &TokenManagerState| {
            writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let first = maintenance.run(&state, &client, &identity, options(false), save);
        let others = async {
            started.notified().await;
            let second = maintenance.run(&state, &client, &identity, options(true), save);
            let third = async {
                release.notify_one();
                maintenance
                    .run(&state, &client, &identity, options(false), save)
                    .await
            };
            tokio::join!(second, third)
        };
        let (first, (second, third)) = tokio::join!(first, others);
        let first = first.unwrap();
        assert_eq!(first.identity, second.unwrap().identity);
        assert_eq!(first.identity, third.unwrap().identity);
        assert_eq!(client.requests.load(Ordering::SeqCst), 1);
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn auth_recovery_access_rejection_forces_refresh_before_estimated_expiry() {
        let (state, identity) = runtime_state(Duration::from_secs(3600));
        let client = ScriptedClient::new([ScriptStep::Response(200, ROTATED)]);
        let result = TokenMaintenance::default()
            .run(&state, &client, &identity, options(true), |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(result.token.access_token.secret(), "new-access");
    }

    #[tokio::test]
    async fn auth_recovery_validation_rejection_forces_refresh() {
        let (state, identity) = runtime_state(Duration::from_secs(3600));
        let client = ScriptedClient::new([
            ScriptStep::Response(401, r#"{"status":401,"message":"invalid access token"}"#),
            ScriptStep::Response(200, ROTATED),
        ]);
        let result = TokenMaintenance::default()
            .run(
                &state,
                &client,
                &identity,
                MaintenanceOptions {
                    validate: true,
                    ..options(false)
                },
                |_| Ok(()),
            )
            .await
            .unwrap();
        assert_eq!(result.token.access_token.secret(), "new-access");
        assert_eq!(client.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn auth_recovery_only_confirmed_refresh_rejection_is_terminal() {
        for (status, body, terminal) in [
            (
                400,
                r#"{"status":400,"message":"Invalid refresh token"}"#,
                true,
            ),
            (
                401,
                r#"{"status":401,"message":"Invalid refresh token"}"#,
                true,
            ),
            (400, r#"{"status":400,"message":"Invalid client"}"#, false),
            (401, r#"{"status":401,"message":"Invalid client"}"#, false),
            (
                429,
                r#"{"status":429,"message":"Too Many Requests"}"#,
                false,
            ),
            (503, r#"{"status":503,"message":"Unavailable"}"#, false),
            (200, "unparseable response", false),
            (401, "unparseable response", false),
        ] {
            let (state, identity) = runtime_state(Duration::ZERO);
            let client = ScriptedClient::new([ScriptStep::Response(status, body)]);
            let error = TokenMaintenance::default()
                .run(&state, &client, &identity, options(false), |_| Ok(()))
                .await
                .unwrap_err();
            assert_eq!(
                matches!(error, RecoveryError::ReauthenticationRequired(_)),
                terminal,
                "status={status}"
            );
            assert!(state.lock().await.matches(&identity));
        }
    }

    #[tokio::test]
    async fn auth_recovery_missing_refresh_token_requires_login() {
        let (state, identity) = runtime_state(Duration::ZERO);
        state
            .lock()
            .await
            .tokens
            .get_mut("1")
            .unwrap()
            .twitch
            .refresh_token = None;
        let client = ScriptedClient::new([]);
        assert!(matches!(
            TokenMaintenance::default()
                .run(&state, &client, &identity, options(false), |_| Ok(()))
                .await,
            Err(RecoveryError::ReauthenticationRequired(_))
        ));
        assert_eq!(client.requests.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn auth_recovery_late_result_cannot_replace_logout_or_new_session() {
        for replacement in [false, true] {
            for (status, body) in [
                (200, ROTATED),
                (400, r#"{"status":400,"message":"Invalid refresh token"}"#),
            ] {
                let (state, identity) = runtime_state(Duration::ZERO);
                let maintenance = TokenMaintenance::default();
                let started = Arc::new(Notify::new());
                let release = Arc::new(Notify::new());
                let client = ScriptedClient::new([ScriptStep::Wait(
                    started.clone(),
                    release.clone(),
                    status,
                    body,
                )]);
                let writes = AtomicUsize::new(0);
                let save = |_: &TokenManagerState| {
                    writes.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                };
                let pending = maintenance.run(&state, &client, &identity, options(false), save);
                let change_session = async {
                    started.notified().await;
                    let mut state = state.lock().await;
                    if replacement {
                        let new_identity = state.next_identity("1".to_owned(), None);
                        state.tokens.get_mut("1").unwrap().identity = new_identity;
                    } else {
                        state.tokens.clear();
                        state.active_user_id = None;
                    }
                    release.notify_one();
                };
                let (result, _) = tokio::join!(pending, change_session);
                assert_eq!(result.unwrap_err(), RecoveryError::SessionChanged);
                assert!(!state.lock().await.matches(&identity));
                assert_eq!(writes.load(Ordering::SeqCst), 0);
            }
        }
    }

    #[tokio::test]
    async fn auth_recovery_persistence_failure_retries_saving_without_rotating_again() {
        let (state, identity) = runtime_state(Duration::ZERO);
        let maintenance = TokenMaintenance::default();
        let client = ScriptedClient::new([ScriptStep::Response(200, ROTATED)]);
        assert!(matches!(
            maintenance
                .run(&state, &client, &identity, options(false), |_| Err(
                    "disk unavailable".to_owned()
                ))
                .await,
            Err(RecoveryError::RetryLater(_))
        ));
        let result = maintenance
            .run(&state, &client, &identity, options(true), |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(
            result.token.refresh_token.as_ref().unwrap().secret(),
            "new-refresh"
        );
        assert!(!state.lock().await.dirty);
        assert_eq!(client.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn auth_recovery_preflight_checks_expiry_after_prior_rotation() {
        let (state, identity) = runtime_state(Duration::ZERO);
        let maintenance = TokenMaintenance::default();
        let client = ScriptedClient::new([
            ScriptStep::Response(200, ROTATED),
            ScriptStep::Response(200, ROTATED),
        ]);
        maintenance
            .run(&state, &client, &identity, options(false), |_| Ok(()))
            .await
            .unwrap();
        state.lock().await.tokens.get_mut("1").unwrap().twitch = runtime_token(Duration::ZERO);
        maintenance
            .run(&state, &client, &identity, options(false), |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(client.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn auth_recovery_request_diagnostics_exclude_credentials() {
        use tracing::instrument::WithSubscriber;
        #[derive(Clone)]
        struct Capture(Arc<StdMutex<Vec<u8>>>);
        impl std::io::Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(StdMutex::new(Vec::new()));
        let capture = bytes.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || Capture(capture.clone()))
            .finish();
        let (state, identity) = runtime_state(Duration::ZERO);
        let client = ScriptedClient::new([ScriptStep::Failure(std::io::ErrorKind::NotConnected)]);
        let error = TokenMaintenance::default()
            .run(&state, &client, &identity, options(false), |_| Ok(()))
            .with_subscriber(subscriber)
            .await
            .unwrap_err();
        let output = format!(
            "{} {error:?}",
            String::from_utf8(bytes.lock().unwrap().clone()).unwrap()
        );
        assert!(
            output.contains("token refresh failed"),
            "captured diagnostics: {output}"
        );
        assert!(!output.contains("secret-refresh"));
        assert!(!output.contains("secret-access"));
        assert!(!output.contains("id.twitch.tv"));
    }

    #[tokio::test]
    async fn auth_recovery_persistence_keeps_accounts_that_could_not_be_restored() {
        let (state, identity) = runtime_state(Duration::ZERO);
        state
            .lock()
            .await
            .unrestored_tokens
            .insert("2".to_owned(), public_token("2", "second"));
        let client = ScriptedClient::new([ScriptStep::Response(200, ROTATED)]);
        TokenMaintenance::default()
            .run(&state, &client, &identity, options(false), |state| {
                let stored = state.persisted();
                assert_eq!(stored.tokens["1"].access_token, "new-access");
                assert_eq!(
                    stored.tokens["2"].refresh_token.as_deref(),
                    Some("refresh-2")
                );
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn auth_recovery_timeout_keeps_saved_credentials() {
        let (state, identity) = runtime_state(Duration::ZERO);
        let client = ScriptedClient::new([ScriptStep::Failure(std::io::ErrorKind::TimedOut)]);
        let error = TokenMaintenance::default()
            .run(&state, &client, &identity, options(false), |_| Ok(()))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            RecoveryError::RetryLater(AuthIssue {
                category: "timeout",
                ..
            })
        ));
        assert_eq!(
            state.lock().await.persisted().tokens["1"]
                .refresh_token
                .as_deref(),
            Some("old-refresh")
        );
    }

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

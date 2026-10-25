use super::{
    admission::{AdmissionAction, AdmissionCoordinator},
    membership::STEADY_ROOMS,
    ChatEngine, ChatFeedContext, ChatFeedRequest, EngineStatus, FeedFrame, FeedScope,
};
use crate::eventsub::{subscription_error_status, JoinOutcome};
use crate::token::{AuthFailure, RecoveryError, TokenIdentity};
use crate::types::{ChatRoomInfo, ChatSettings};
use crate::{app_settings, SharedBadgeManager, SharedEmoteManager, SharedEventSubManager};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};

#[derive(Default)]
pub struct ChatRuntime {
    session: Mutex<Option<Arc<ChatEngine>>>,
    translations: Mutex<HashMap<String, tauri::EventId>>,
    tasks: Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>,
}

impl ChatRuntime {
    pub fn current(&self) -> Result<Arc<ChatEngine>, String> {
        self.session
            .lock()
            .unwrap()
            .clone()
            .filter(|e| e.active())
            .ok_or_else(|| "chat engine is not running".into())
    }
    pub fn activate(&self, app: &AppHandle, settings: ChatSettings) -> Arc<ChatEngine> {
        self.stop(app);
        let engine = ChatEngine::new(settings);
        *self.session.lock().unwrap() = Some(engine.clone());
        engine
    }
    pub fn stop(&self, app: &AppHandle) {
        if let Some(engine) = self.session.lock().unwrap().take() {
            engine.shutdown();
        }
        for handle in self.tasks.lock().unwrap().drain(..) {
            handle.abort();
        }
        for (_, id) in self.translations.lock().unwrap().drain() {
            app.unlisten(id);
        }
    }
    pub fn bridge_translation(&self, app: &AppHandle, engine: &Arc<ChatEngine>, login: &str) {
        let mut listeners = self.translations.lock().unwrap();
        if listeners.contains_key(login) || !engine.active() {
            return;
        }
        let weak = Arc::downgrade(engine);
        let channel = login.to_owned();
        let id = app.listen(format!("chat_translation:{login}"), move |event| {
            if let (Some(engine), Ok(update)) =
                (weak.upgrade(), serde_json::from_str(event.payload()))
            {
                engine.translate(&channel, update);
            }
        });
        listeners.insert(login.to_owned(), id);
    }
}

pub async fn resolve_channel(
    app: &AppHandle,
    engine: &Arc<ChatEngine>,
    login: &str,
) -> Result<ChatRoomInfo, String> {
    if let Some(info) = engine.state.lock().unwrap().directory.get(login).cloned() {
        return Ok(info);
    }
    let token = app
        .state::<crate::token::TokenManager>()
        .active_twitch_token()
        .await
        .ok_or("no active token")?;
    let client = app
        .state::<twitch_api::HelixClient<'static, reqwest::Client>>()
        .inner()
        .clone();
    let channel = client
        .get_channel_from_login(login, &token)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("channel not found: {login}"))?;
    let info = ChatRoomInfo::from(channel);
    let mut state = engine.state.lock().unwrap();
    if !state.active {
        return Err("chat session ended".into());
    }
    state.directory.insert(login.to_owned(), info.clone());
    drop(state);
    app.state::<ChatRuntime>()
        .bridge_translation(app, engine, login);
    crate::spawn_joined_channel_cache_refresh(app.clone(), info.clone(), token);
    Ok(info)
}

#[tauri::command]
#[specta::specta]
pub fn get_chat_feed_context(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Result<ChatFeedContext, String> {
    Ok(app
        .state::<ChatRuntime>()
        .current()?
        .feed_context(window.label()))
}

pub(super) async fn prepare_feed(
    engine: &Arc<ChatEngine>,
    owner: &str,
    request: &ChatFeedRequest,
    login: String,
    resolve: impl std::future::Future<Output = Result<ChatRoomInfo, String>>,
) -> Result<(ChatRoomInfo, super::FeedSubscription), String> {
    if !engine.begin_request(owner, request) {
        return Err("subscription canceled".into());
    }
    let info = match resolve.await {
        Ok(info) => info,
        Err(error) => {
            engine.detach(owner, request);
            return Err(error);
        }
    };
    let feed = engine.subscribe(FeedScope::Channel(login.clone()));
    if !engine.attach(owner, request, login, feed.id) {
        return Err("subscription canceled".into());
    }
    Ok((info, feed))
}

#[tauri::command]
#[specta::specta]
pub async fn subscribe_chat(
    channel_name: String,
    request: ChatFeedRequest,
    on_event: tauri::ipc::Channel<FeedFrame>,
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Result<ChatRoomInfo, String> {
    let engine = app.state::<ChatRuntime>().current()?;
    let login = crate::normalize_channel_login(&channel_name);
    let owner = window.label().to_owned();
    let (info, mut feed) = prepare_feed(
        &engine,
        &owner,
        &request,
        login.clone(),
        resolve_channel(&app, &engine, &login),
    )
    .await?;
    let weak = Arc::downgrade(&engine);
    tauri::async_runtime::spawn(async move {
        loop {
            let Some(engine) = weak.upgrade() else {
                break;
            };
            if !engine.request_active(&owner, &request) {
                break;
            }
            for frame in feed.poll() {
                if on_event.send(frame.as_ref().clone()).is_err() {
                    engine.detach(&owner, &request);
                    return;
                }
            }
            drop(engine);
            if !feed.changed().await {
                break;
            }
        }
    });
    Ok(info)
}

#[tauri::command]
#[specta::specta]
pub fn acknowledge_chat(
    request: ChatFeedRequest,
    cursor: u64,
    window: tauri::WebviewWindow,
    app: AppHandle,
) {
    if let Ok(engine) = app.state::<ChatRuntime>().current() {
        engine.acknowledge(window.label(), &request, cursor);
    }
}
#[tauri::command]
#[specta::specta]
pub fn unsubscribe_chat(request: ChatFeedRequest, window: tauri::WebviewWindow, app: AppHandle) {
    if let Ok(engine) = app.state::<ChatRuntime>().current() {
        engine.detach(window.label(), &request);
    }
}
#[tauri::command]
#[specta::specta]
pub fn get_chat_engine_status(app: AppHandle) -> EngineStatus {
    app.state::<ChatRuntime>()
        .current()
        .map(|e| e.state.lock().unwrap().status.clone())
        .unwrap_or_default()
}

pub async fn run_diagnostics(engine: Arc<ChatEngine>) {
    let mut changed = engine.changed.subscribe();
    let mut timer = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    loop {
        tokio::select! {
            _ = timer.tick() => {
                if !engine.active() { break; }
                let metrics = engine.metrics();
                let joined = engine.state.lock().unwrap().status.joined;
                tracing::debug!(joined_channels = joined, channels_with_history = metrics.channels_with_history,
                    retained_messages = metrics.retained_messages, history_limit_per_channel = metrics.history_limit_per_channel,
                    estimated_history_bytes = metrics.estimated_history_bytes, "chat store summary");
            }
            result = changed.changed() => {
                if result.is_err() || !engine.active() { break; }
            }
        }
    }
}

async fn stop_for_admission_auth_failure(
    engine: &ChatEngine,
    error: &str,
    recovery: impl std::future::Future<Output = Result<(), RecoveryError>>,
    clear_auth: impl FnOnce(AuthFailure),
) -> bool {
    if !engine.active() {
        return true;
    }
    if !crate::is_auth_failure_error(error) && !error.contains("AUTH_EXPIRED") {
        return false;
    }
    let result = recovery.await;
    if !engine.active() {
        return true;
    }
    match result {
        Ok(()) => false,
        Err(RecoveryError::SessionChanged) => true,
        Err(RecoveryError::ReauthenticationRequired(failure)) => {
            clear_auth(failure);
            true
        }
        Err(error) => {
            tracing::warn!(operation = "chat_admission_recovery", "{error}");
            false
        }
    }
}

async fn recover_failed_auth(
    app: &AppHandle,
    engine: &ChatEngine,
    identity: &TokenIdentity,
    error: &str,
) -> bool {
    let token_manager = app.state::<crate::token::TokenManager>();
    stop_for_admission_auth_failure(
        engine,
        error,
        async {
            token_manager
                .recover_rejected_token(identity)
                .await
                .map(|_| ())
        },
        |failure| {
            let clear_app = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::clear_auth_if_current_async(&clear_app, true, Some(&failure.identity)).await;
            });
        },
    )
    .await
}

pub(super) fn admission_action(
    engine: &ChatEngine,
    manager: &crate::eventsub::EventSubManager,
    coordinator: &mut AdmissionCoordinator,
    now: Instant,
) -> (Option<AdmissionAction>, usize, usize) {
    let desired: Vec<_> = manager
        .desired_channels_snapshot()
        .into_iter()
        .map(|(login, _)| login)
        .collect();
    let physical = manager.channels_with_subscriptions();
    let complete: HashSet<_> = physical
        .iter()
        .filter(|login| manager.channel_complete(login))
        .cloned()
        .collect();
    let mut state = engine.state.lock().unwrap();
    let mode = state.settings.join_mode;
    let intended = state.membership.intended(mode);
    state
        .failures
        .retain(|login, _| intended.contains(login) || physical.contains(login));
    state.membership.unavailable = state
        .failures
        .iter()
        .filter(|(_, failure)| failure.retry_at > now)
        .map(|(login, _)| login.clone())
        .collect();
    state
        .membership
        .selected
        .retain(|login| complete.contains(login));
    let ranked = state.membership.ranked(mode);
    let eligible = ranked.len();
    let pending = ranked
        .iter()
        .take(STEADY_ROOMS)
        .filter(|login| !complete.contains(*login))
        .count();
    let action =
        coordinator.next_action(&state.membership, mode, &desired, &physical, &complete, now);
    (action, eligible, pending)
}

pub fn start(app: AppHandle, engine: Arc<ChatEngine>) {
    let registry_app = app.clone();
    let metrics_task = tauri::async_runtime::spawn(run_diagnostics(engine.clone()));
    let membership_task = tauri::async_runtime::spawn(async move {
        let manager = app.state::<SharedEventSubManager>().lock().await.clone();
        let badges = app.state::<SharedBadgeManager>().lock().await.clone();
        let emotes = app.state::<SharedEmoteManager>().lock().await.clone();
        let client = app
            .state::<twitch_api::HelixClient<'static, reqwest::Client>>()
            .inner()
            .clone();
        let cosmetics = Arc::new(tokio::sync::Semaphore::new(4));
        let mut refreshes = tokio::task::JoinSet::<String>::new();
        let mut refreshing = HashSet::new();
        let mut coordinator = AdmissionCoordinator::default();
        let mut hydrated = HashSet::new();
        while engine.active() {
            while let Some(result) = refreshes.try_join_next() {
                if let Ok(login) = result {
                    refreshing.remove(&login);
                }
            }
            for (login, reason) in manager.take_room_failures() {
                let mut state = engine.state.lock().unwrap();
                if state
                    .failures
                    .get(&login)
                    .is_none_or(|failure| failure.reason != reason)
                {
                    state.fail_room(&login, reason, Instant::now());
                }
                drop(state);
                coordinator.require_cleanup(&login);
                engine.wake();
            }
            let Some(snapshot) = app
                .state::<crate::token::TokenManager>()
                .active_snapshot()
                .await
            else {
                break;
            };
            if !engine.active() {
                break;
            }
            let token = snapshot.token;
            let (action, _, _) =
                admission_action(&engine, &manager, &mut coordinator, Instant::now());
            match action {
                Some(AdmissionAction::Leave { login, displaced }) => {
                    match manager
                        .clone()
                        .leave_chat(login.clone(), &client, token.clone())
                        .await
                    {
                        Ok(()) => {
                            coordinator.cleanup_succeeded(&login);
                            let mut state = engine.state.lock().unwrap();
                            state.membership.selected.remove(&login);
                            if displaced {
                                state.membership.displaced(&login);
                            }
                            drop(state);
                            engine.wake();
                        }
                        Err(error) => {
                            if recover_failed_auth(
                                &app,
                                &engine,
                                &snapshot.identity,
                                &error.to_string(),
                            )
                            .await
                            {
                                return;
                            }
                            let now = Instant::now();
                            if subscription_error_status::<reqwest::Client>(&error) == Some(429) {
                                coordinator.throttle(now);
                            }
                            coordinator.cleanup_failed(&login, error.to_string(), now);
                        }
                    }
                }
                Some(AdmissionAction::Join(login)) if manager.is_connected() => {
                    let info = engine.state.lock().unwrap().directory.get(&login).cloned();
                    if let Some(info) = info {
                        let settings = app_settings(
                            app.state::<tauri_svelte_synced_store::StateSyncer>()
                                .inner(),
                        );
                        if hydrated.insert(login.clone()) {
                            badges.hydrate_global(&settings.providers).await;
                            badges
                                .hydrate_channel(&info.broadcaster_id, &settings.providers)
                                .await;
                            emotes.preload(
                                &info.broadcaster_id,
                                &settings.emotes,
                                &settings.providers,
                            );
                        }
                        let (current, _, _) =
                            admission_action(&engine, &manager, &mut coordinator, Instant::now());
                        if engine.active()
                            && current == Some(AdmissionAction::Join(login.clone()))
                            && coordinator.acquire_join(Instant::now())
                        {
                            app.state::<ChatRuntime>()
                                .bridge_translation(&app, &engine, &login);
                            match manager
                                .clone()
                                .join_chat(
                                    info.broadcaster_id.clone().into(),
                                    login.clone(),
                                    &client,
                                    token.clone(),
                                )
                                .await
                            {
                                Ok(JoinOutcome::Joined) if manager.channel_complete(&login) => {
                                    let mut state = engine.state.lock().unwrap();
                                    state.failures.remove(&login);
                                    let wanted = state
                                        .membership
                                        .would_admit(&login, state.settings.join_mode);
                                    if wanted {
                                        state.membership.selected.insert(login.clone());
                                    }
                                    drop(state);
                                    engine.wake();
                                    if !wanted {
                                        coordinator.require_cleanup(&login);
                                    } else if refreshing.insert(login.clone()) {
                                        let refreshed_login = login.clone();
                                        let permits = cosmetics.clone();
                                        let badges = badges.clone();
                                        let emotes = emotes.clone();
                                        let client = client.clone();
                                        let weak = Arc::downgrade(&engine);
                                        refreshes.spawn(async move {
                                            let Ok(_permit) = permits.acquire_owned().await else {
                                                return refreshed_login;
                                            };
                                            if !weak.upgrade().is_some_and(|engine| engine.active())
                                            {
                                                return refreshed_login;
                                            }
                                            tokio::join!(
                                                badges.load_channel(
                                                    info.broadcaster_id.clone(),
                                                    client,
                                                    &settings.providers
                                                ),
                                                emotes.load_channel(
                                                    info.broadcaster_id,
                                                    &settings.emotes,
                                                    &settings.providers
                                                )
                                            );
                                            refreshed_login
                                        });
                                    }
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    if recover_failed_auth(
                                        &app,
                                        &engine,
                                        &snapshot.identity,
                                        &error.to_string(),
                                    )
                                    .await
                                    {
                                        return;
                                    }
                                    let now = Instant::now();
                                    if subscription_error_status::<reqwest::Client>(&error)
                                        == Some(429)
                                    {
                                        coordinator.throttle(now);
                                    }
                                    engine.state.lock().unwrap().fail_room(
                                        &login,
                                        error.to_string(),
                                        now,
                                    );
                                    coordinator.require_cleanup(&login);
                                    engine.wake();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            let (_, eligible, pending) =
                admission_action(&engine, &manager, &mut coordinator, Instant::now());
            let (status, status_changed) = {
                let mut state = engine.state.lock().unwrap();
                let mut failures: Vec<_> = state
                    .failures
                    .iter()
                    .map(|(login, failure)| format!("{login}: {}", failure.reason))
                    .collect();
                failures.extend(
                    coordinator
                        .cleanup_errors()
                        .map(|(login, failure)| format!("{login}: cleanup: {}", failure.reason)),
                );
                failures.sort();
                let status = EngineStatus {
                    connected: manager.is_connected(),
                    joined: manager.joined_channels().len(),
                    eligible,
                    lingering: state.membership.lingering.len(),
                    pending,
                    limited: eligible.saturating_sub(STEADY_ROOMS),
                    failures,
                };
                let status_changed = state.status != status;
                state.status = status.clone();
                (status, status_changed)
            };
            if status_changed {
                let _ = app.emit("chat_engine_status", &status);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    registry_app
        .state::<ChatRuntime>()
        .tasks
        .lock()
        .unwrap()
        .extend([metrics_task, membership_task]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::AuthIssue;

    fn terminal_failure() -> AuthFailure {
        AuthFailure {
            identity: TokenIdentity {
                user_id: "user".into(),
                session: 2,
                generation: 3,
            },
            issue: AuthIssue::permanent("refresh_token_rejected"),
        }
    }

    #[tokio::test]
    async fn admission_access_rejection_recovers_without_stopping_engine() {
        let engine = ChatEngine::new(ChatSettings::default());
        let mut recovered = false;
        let stop = stop_for_admission_auth_failure(
            &engine,
            "AUTH_EXPIRED: subscription rejected with 401",
            async {
                recovered = true;
                Ok(())
            },
            |_| panic!("a recovered access token must not clear auth"),
        )
        .await;
        assert!(recovered);
        assert!(!stop);
        assert!(engine.active());
    }

    #[tokio::test]
    async fn admission_retryable_recovery_preserves_engine_and_auth() {
        let engine = ChatEngine::new(ChatSettings::default());
        let stop = stop_for_admission_auth_failure(
            &engine,
            "401 Unauthorized",
            async { Err(RecoveryError::RetryLater(AuthIssue::retry("network"))) },
            |_| panic!("a transient recovery failure must not clear auth"),
        )
        .await;
        assert!(!stop);
        assert!(engine.active());
    }

    #[tokio::test]
    async fn admission_terminal_recovery_passes_identity_to_guarded_cleanup() {
        let engine = ChatEngine::new(ChatSettings::default());
        let failure = terminal_failure();
        let mut cleared = None;
        let stop = stop_for_admission_auth_failure(
            &engine,
            "401 Unauthorized",
            async { Err(RecoveryError::ReauthenticationRequired(failure.clone())) },
            |failure| cleared = Some(failure),
        )
        .await;
        assert!(stop);
        assert_eq!(cleared, Some(failure));
    }

    #[tokio::test]
    async fn admission_session_change_stops_old_work_without_clearing_auth() {
        let engine = ChatEngine::new(ChatSettings::default());
        let stop = stop_for_admission_auth_failure(
            &engine,
            "401 Unauthorized",
            async { Err(RecoveryError::SessionChanged) },
            |_| panic!("a different session must not be cleared"),
        )
        .await;
        assert!(stop);
    }

    #[tokio::test]
    async fn admission_late_recovery_cannot_clear_replacement_engine() {
        let engine = ChatEngine::new(ChatSettings::default());
        let replacement = ChatEngine::new(ChatSettings::default());
        let stop = stop_for_admission_auth_failure(
            &engine,
            "401 Unauthorized",
            async {
                tokio::task::yield_now().await;
                engine.shutdown();
                Err(RecoveryError::ReauthenticationRequired(terminal_failure()))
            },
            |_| panic!("a stopped engine must not clear the replacement session"),
        )
        .await;
        assert!(stop);
        assert!(replacement.active());
    }

    #[tokio::test]
    async fn admission_room_failure_does_not_attempt_token_recovery() {
        let engine = ChatEngine::new(ChatSettings::default());
        let stop = stop_for_admission_auth_failure(
            &engine,
            "403 subscription missing proper authorization",
            async { panic!("a room failure must not refresh account credentials") },
            |_| panic!("a room failure must not clear auth"),
        )
        .await;
        assert!(!stop);
        assert!(engine.active());
    }
}

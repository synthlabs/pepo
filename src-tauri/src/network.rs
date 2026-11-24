use crate::{app_settings, StateSyncer};
use futures::{future::BoxFuture, FutureExt};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{sync::Notify, task::JoinSet};

#[derive(Clone, Debug)]
pub struct Failure {
    pub category: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after: Duration,
    pub reportable: bool,
}

impl Failure {
    pub fn new(category: &str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            category: category.into(),
            message: message.into(),
            retryable,
            retry_after: Duration::ZERO,
            reportable: matches!(category, "decode" | "response" | "builder" | "panic"),
        }
    }
    pub fn request(error: &reqwest::Error) -> Self {
        let category = if error.is_builder() {
            "builder"
        } else if error.is_timeout() {
            "timeout"
        } else if error.is_connect() {
            "connection"
        } else if error.is_body() || error.is_decode() {
            "transfer"
        } else {
            "request"
        };
        Self::new(
            category,
            format!("Request failed ({category})."),
            !error.is_builder(),
        )
    }
    pub fn status(status: u16) -> Self {
        Self::new(
            &format!("http_{status}"),
            format!("Server returned HTTP {status}."),
            status == 401 || status == 408 || status == 429 || (500..600).contains(&status),
        )
    }
    pub fn helix(error: twitch_api::helix::ClientRequestError<reqwest::Error>) -> Self {
        use twitch_api::helix::{ClientRequestError as E, HelixRequestGetError as G};
        match error {
            E::RequestError(error) => Self::request(&error),
            E::HelixRequestGetError(G::DeserializeError(_, _, _, status))
                if status.is_success() =>
            {
                Self::new("decode", "Twitch metadata could not be decoded.", false)
            }
            E::HelixRequestGetError(
                G::Error { status, .. }
                | G::InvalidResponse { status, .. }
                | G::DeserializeError(_, _, _, status),
            ) => Self::status(status.as_u16()),
            _ => Self::new("response", "The server response could not be read.", false),
        }
    }
}

#[derive(Clone, Debug, Serialize, specta::Type)]
pub struct NetworkIssue {
    pub id: String,
    pub feature: String,
    pub category: String,
    pub message: String,
    pub failures: u32,
    pub affected: usize,
    pub cached: bool,
    #[specta(type = specta_typescript::Number)]
    pub last_failure_at: f64,
    #[specta(type = Option<specta_typescript::Number>)]
    pub retry_at: Option<f64>,
    pub retryable: bool,
    pub core: bool,
    pub can_retry: bool,
}

#[derive(Clone, Default, Serialize, specta::Type)]
pub struct NetworkHealthSnapshot {
    pub revision: u32,
    pub issues: Vec<NetworkIssue>,
}

type Run = Arc<dyn Fn() -> BoxFuture<'static, Result<(), Failure>> + Send + Sync>;
async fn run_job(run: Run) -> Result<(), Failure> {
    std::panic::AssertUnwindSafe(async move { run().await })
        .catch_unwind()
        .await
        .unwrap_or_else(|_| {
            Err(Failure::new(
                "panic",
                "Metadata refresh stopped unexpectedly.",
                false,
            ))
        })
}
pub type Priority = Arc<dyn Fn() -> Option<u8> + Send + Sync>;
pub fn metadata_priority(engine: &Arc<crate::chat::ChatEngine>, login: Option<String>) -> Priority {
    let engine = Arc::downgrade(engine);
    Arc::new(move || {
        let engine = engine.upgrade()?;
        let state = engine.state.lock().unwrap();
        if !state.active {
            return None;
        }
        match login.as_ref() {
            None => Some(1),
            Some(login) if state.membership.foreground.contains(login) => Some(0),
            Some(login)
                if state
                    .membership
                    .intended(state.settings.join_mode)
                    .contains(login) =>
            {
                Some(2)
            }
            _ => None,
        }
    })
}
pub struct MetadataJob {
    pub key: String,
    pub provider: String,
    pub feature: String,
    pub cached: bool,
    pub run: Run,
    pub priority: Priority,
}
struct Pending {
    job: MetadataJob,
    attempts: u32,
    due: Instant,
    running: bool,
    paused: bool,
}
struct Incident {
    issue: NetworkIssue,
    operations: HashSet<String>,
    last_log: Instant,
    suppressed: u32,
}
struct RequestLease {
    provider: String,
    owner: Option<String>,
}
#[derive(Default)]
struct State {
    jobs: BTreeMap<String, Pending>,
    incidents: BTreeMap<String, Incident>,
    cooldowns: HashMap<String, Instant>,
    server_waits: HashMap<String, Instant>,
    revision: u32,
    leases: HashMap<String, RequestLease>,
    next_lease: u64,
}
#[derive(Clone, Default)]
pub struct NetworkRuntime {
    state: Arc<Mutex<State>>,
    wake: Arc<Notify>,
    task: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    pub channels_retry: Arc<Notify>,
}

fn unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0
}

pub fn retry_after(headers: &reqwest::header::HeaderMap) -> Duration {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .parse::<u64>()
                .ok()
                .map(Duration::from_secs)
                .or_else(|| {
                    httpdate::parse_http_date(value)
                        .ok()
                        .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
                })
        })
        .filter(|delay| Instant::now().checked_add(*delay).is_some())
        .unwrap_or_default()
}

impl NetworkRuntime {
    pub fn snapshot(&self) -> NetworkHealthSnapshot {
        let state = self.state.lock().unwrap();
        NetworkHealthSnapshot {
            revision: state.revision,
            issues: state
                .incidents
                .values()
                .map(|entry| {
                    let mut issue = entry.issue.clone();
                    issue.can_retry = entry.operations.iter().any(|operation| {
                        state.jobs.contains_key(operation) || operation == "channels.refresh"
                    });
                    issue
                })
                .collect(),
        }
    }
    fn publish(&self, app: &AppHandle) {
        let _ = app.emit("network_status", self.snapshot());
    }
    pub fn reset(&self, app: &AppHandle) {
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
        {
            let mut state = self.state.lock().unwrap();
            let leases = std::mem::take(&mut state.leases);
            *state = State {
                revision: state.revision.wrapping_add(1),
                next_lease: state.next_lease,
                leases,
                ..Default::default()
            };
        }
        self.publish(app);
    }
    pub fn start(&self, app: AppHandle) {
        self.reset(&app);
        let runtime = self.clone();
        *self.task.lock().unwrap() = Some(tauri::async_runtime::spawn(async move {
            runtime.run(app).await
        }));
    }
    pub fn enqueue(&self, job: MetadataJob) {
        self.state
            .lock()
            .unwrap()
            .jobs
            .entry(job.key.clone())
            .or_insert(Pending {
                job,
                attempts: 0,
                due: Instant::now(),
                running: false,
                paused: false,
            });
        self.wake.notify_one();
    }
    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        &self,
        app: &AppHandle,
        operation: &str,
        feature: &str,
        failure: &Failure,
        cached: bool,
        core: bool,
        delay: Option<Duration>,
    ) {
        self.state.lock().unwrap().record_failure(
            operation,
            feature,
            failure,
            cached,
            core,
            delay,
            Instant::now(),
        );
        self.publish(app);
    }
    pub fn recovered(&self, app: &AppHandle, operation: &str) {
        let changed = {
            let mut state = self.state.lock().unwrap();
            let mut changed = false;
            state.incidents.retain(|_, entry| {
                if entry.operations.remove(operation) {
                    changed = true;
                    entry.issue.affected = entry.operations.len();
                }
                if entry.operations.is_empty() {
                    tracing::info!(
                        feature = entry.issue.feature,
                        failures = entry.issue.failures,
                        "network operation recovered"
                    );
                    false
                } else {
                    true
                }
            });
            if changed {
                state.revision = state.revision.wrapping_add(1);
            }
            changed
        };
        if changed {
            self.publish(app);
        }
    }
    pub fn retry(&self, id: &str) {
        let mut state = self.state.lock().unwrap();
        let operations = state
            .incidents
            .get(id)
            .map(|entry| entry.operations.clone())
            .unwrap_or_default();
        for operation in operations {
            if operation == "channels.refresh" {
                self.channels_retry.notify_one();
            }
            if let Some(job) = state.jobs.get_mut(&operation) {
                job.due = Instant::now();
                job.paused = false;
                let provider = job.job.provider.clone();
                let due = state
                    .server_waits
                    .get(&provider)
                    .copied()
                    .unwrap_or_else(Instant::now);
                state.cooldowns.insert(provider, due);
            }
        }
        drop(state);
        self.wake.notify_one();
    }
    pub async fn acquire_request(
        &self,
        app: &AppHandle,
        provider: &str,
        owner: Option<String>,
    ) -> Result<String, String> {
        loop {
            if owner
                .as_ref()
                .is_some_and(|label| app.get_webview_window(label).is_none())
            {
                return Err("Window closed".into());
            }
            let limit = app_settings(app.state::<StateSyncer>().inner())
                .network
                .metadata_concurrency;
            {
                let mut state = self.state.lock().unwrap();
                if let Some(id) = state.try_acquire_request(limit, provider, owner.clone()) {
                    return Ok(id);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    pub async fn request_guard(&self, app: &AppHandle, provider: &str) -> Box<dyn Send> {
        let id = self
            .acquire_request(app, provider, None)
            .await
            .expect("native request has no window owner");
        Box::new(RequestGuard {
            runtime: self.clone(),
            id,
        })
    }
    pub fn release_owner(&self, owner: &str) {
        self.state
            .lock()
            .unwrap()
            .leases
            .retain(|_, lease| lease.owner.as_deref() != Some(owner));
        self.wake.notify_one();
    }
    async fn run(self, app: AppHandle) {
        let mut running = JoinSet::<(String, String, Result<(), Failure>)>::new();
        loop {
            let settings = app_settings(app.state::<StateSyncer>().inner()).network;
            let obsolete = {
                let mut state = self.state.lock().unwrap();
                let keys: Vec<_> = state
                    .jobs
                    .iter()
                    .filter(|(_, pending)| !pending.running && (pending.job.priority)().is_none())
                    .map(|(key, _)| key.clone())
                    .collect();
                for key in &keys {
                    state.jobs.remove(key);
                }
                keys
            };
            for key in obsolete {
                self.recovered(&app, &key);
            }
            {
                let mut state = self.state.lock().unwrap();
                let mut busy: HashSet<String> = state
                    .jobs
                    .values()
                    .filter(|job| job.running)
                    .map(|job| job.job.provider.clone())
                    .collect();
                busy.extend(state.leases.values().map(|lease| lease.provider.clone()));
                while running.len() + state.leases.len() < settings.metadata_concurrency {
                    let selected = select_job(&state, &busy, Instant::now());
                    let Some(key) = selected else {
                        break;
                    };
                    let pending = state.jobs.get_mut(&key).unwrap();
                    pending.running = true;
                    let provider = pending.job.provider.clone();
                    busy.insert(provider.clone());
                    let run = pending.job.run.clone();
                    running.spawn(async move { (key, provider, run_job(run).await) });
                }
            }
            tokio::select! {
                Some(result) = running.join_next(), if !running.is_empty() => {
                    if let Ok((key, provider, result)) = result {
                        match result {
                            Ok(()) => {
                                self.state.lock().unwrap().jobs.remove(&key);
                                self.recovered(&app, &key);
                            }
                            Err(failure) => {
                                let (feature, cached, delay) = {
                                    let mut state = self.state.lock().unwrap();
                                    let Some(pending) = state.jobs.get_mut(&key) else { continue; };
                                    pending.running = false;
                                    pending.paused = !failure.retryable;
                                    pending.attempts = pending.attempts.saturating_add(1);
                                    let jitter = Duration::from_millis((unix_ms() as u64) % 1000);
                                    let delay = settings.retry_delay(pending.attempts).saturating_add(jitter).max(failure.retry_after);
                                    pending.due = Instant::now() + delay;
                                    let result = (pending.job.feature.clone(), pending.job.cached, delay);
                                    if !failure.retry_after.is_zero() { state.server_waits.insert(provider.clone(), Instant::now() + failure.retry_after); }
                                    if failure.retryable { state.cooldowns.insert(provider, Instant::now() + delay); }
                                    result
                                };
                                self.failed(&app, &key, &feature, &failure, cached, false, failure.retryable.then_some(delay));
                                if failure.reportable { tracing::error!(feature, category = failure.category, "metadata refresh failed: {}", failure.message); }
                            }
                        }
                    }
                }
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
    }
}

struct RequestGuard {
    runtime: NetworkRuntime,
    id: String,
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.runtime.state.lock().unwrap().leases.remove(&self.id);
        self.runtime.wake.notify_one();
    }
}

#[tauri::command]
#[specta::specta]
pub async fn begin_background_request(
    app: AppHandle,
    window: tauri::Window,
    provider: String,
) -> Result<String, String> {
    if provider != "updates" {
        return Err("Unsupported background request".into());
    }
    app.state::<NetworkRuntime>()
        .acquire_request(&app, &provider, Some(window.label().into()))
        .await
}

#[tauri::command]
#[specta::specta]
pub fn end_background_request(app: AppHandle, window: tauri::Window, id: String) {
    let runtime = app.state::<NetworkRuntime>();
    let mut state = runtime.state.lock().unwrap();
    if state
        .leases
        .get(&id)
        .is_some_and(|lease| lease.owner.as_deref() == Some(window.label()))
    {
        state.leases.remove(&id);
    }
    runtime.wake.notify_one();
}

fn select_job(state: &State, busy: &HashSet<String>, now: Instant) -> Option<String> {
    state
        .jobs
        .iter()
        .filter(|(_, pending)| {
            !pending.running
                && !pending.paused
                && pending.due <= now
                && !busy.contains(&pending.job.provider)
                && state
                    .cooldowns
                    .get(&pending.job.provider)
                    .is_none_or(|due| *due <= now)
        })
        .filter_map(|(key, pending)| (pending.job.priority)().map(|priority| (priority, key)))
        .min()
        .map(|(_, key)| key.clone())
}

#[tauri::command]
#[specta::specta]
pub fn get_network_status(app: AppHandle) -> NetworkHealthSnapshot {
    app.state::<NetworkRuntime>().snapshot()
}

#[tauri::command]
#[specta::specta]
pub fn retry_network_operation(app: AppHandle, id: String) {
    app.state::<NetworkRuntime>().retry(&id);
}

impl State {
    fn try_acquire_request(
        &mut self,
        limit: usize,
        provider: &str,
        owner: Option<String>,
    ) -> Option<String> {
        let running = self.jobs.values().filter(|job| job.running).count();
        let busy = self
            .jobs
            .values()
            .any(|job| job.running && job.job.provider == provider)
            || self.leases.values().any(|lease| lease.provider == provider);
        if running + self.leases.len() >= limit || busy {
            return None;
        }
        self.next_lease = self.next_lease.wrapping_add(1);
        let id = self.next_lease.to_string();
        self.leases.insert(
            id.clone(),
            RequestLease {
                provider: provider.into(),
                owner,
            },
        );
        Some(id)
    }
    #[allow(clippy::too_many_arguments)]
    fn record_failure(
        &mut self,
        operation: &str,
        feature: &str,
        failure: &Failure,
        cached: bool,
        core: bool,
        delay: Option<Duration>,
        now: Instant,
    ) {
        let id = format!("{feature}:{}", failure.category);
        let entry = self
            .incidents
            .entry(id.clone())
            .or_insert_with(|| Incident {
                issue: NetworkIssue {
                    id,
                    feature: feature.into(),
                    category: failure.category.clone(),
                    message: failure.message.clone(),
                    failures: 0,
                    affected: 0,
                    cached,
                    last_failure_at: unix_ms(),
                    retry_at: None,
                    retryable: failure.retryable,
                    core,
                    can_retry: false,
                },
                operations: HashSet::new(),
                last_log: now,
                suppressed: 0,
            });
        entry.issue.failures = entry.issue.failures.saturating_add(1);
        entry.issue.message = failure.message.clone();
        entry.operations.insert(operation.into());
        entry.issue.affected = entry.operations.len();
        entry.issue.cached |= cached;
        entry.issue.last_failure_at = unix_ms();
        entry.issue.retry_at = delay.map(|value| unix_ms() + value.as_secs_f64() * 1000.0);
        if entry.issue.failures == 1
            || now.duration_since(entry.last_log) >= Duration::from_secs(300)
        {
            tracing::warn!(
                feature,
                category = failure.category,
                failures = entry.issue.failures,
                suppressed = entry.suppressed,
                "{}",
                failure.message
            );
            entry.last_log = now;
            entry.suppressed = 0;
        } else {
            entry.suppressed = entry.suppressed.saturating_add(1);
        }
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn a_panicked_refresh_returns_a_reportable_failure_and_releases_dispatch() {
        let failure = run_job(Arc::new(|| Box::pin(async { panic!("fixture failure") })))
            .await
            .unwrap_err();
        assert!(failure.reportable);
        assert!(!failure.retryable);
    }
    fn pending(key: &str, provider: &str, priority: u8) -> Pending {
        Pending {
            job: MetadataJob {
                key: key.into(),
                provider: provider.into(),
                feature: provider.into(),
                cached: true,
                run: Arc::new(|| Box::pin(async { Ok(()) })),
                priority: Arc::new(move || Some(priority)),
            },
            attempts: 0,
            due: Instant::now(),
            running: false,
            paused: false,
        }
    }
    #[test]
    fn foreground_priority_and_provider_limits_apply_before_dispatch() {
        let mut state = State::default();
        state
            .jobs
            .insert("background".into(), pending("background", "ffz", 2));
        state
            .jobs
            .insert("foreground".into(), pending("foreground", "ffz", 0));
        state
            .jobs
            .insert("global".into(), pending("global", "bttv", 1));
        assert_eq!(
            select_job(&state, &HashSet::new(), Instant::now()).as_deref(),
            Some("foreground")
        );
        let busy = HashSet::from(["ffz".into()]);
        assert_eq!(
            select_job(&state, &busy, Instant::now()).as_deref(),
            Some("global")
        );
        state
            .cooldowns
            .insert("bttv".into(), Instant::now() + Duration::from_secs(60));
        assert!(select_job(&state, &busy, Instant::now()).is_none());
    }
    #[test]
    fn catalog_and_update_checks_share_capacity_with_metadata_jobs() {
        let mut state = State::default();
        for provider in ["ffz", "bttv", "twitch"] {
            let mut job = pending(provider, provider, 1);
            job.running = true;
            state.jobs.insert(provider.into(), job);
        }
        let catalog = state.try_acquire_request(4, "catalog", None).unwrap();
        assert!(state
            .try_acquire_request(4, "updates", Some("main".into()))
            .is_none());
        state.leases.remove(&catalog);
        let update = state
            .try_acquire_request(4, "updates", Some("main".into()))
            .unwrap();
        assert!(state
            .try_acquire_request(4, "updates", Some("other".into()))
            .is_none());
        state.leases.remove(&update);
        assert!(state.try_acquire_request(2, "updates", None).is_none());
    }
    #[test]
    fn cancelled_and_permanent_jobs_do_not_start_automatically() {
        let mut state = State::default();
        let mut obsolete = pending("obsolete", "ffz", 0);
        obsolete.job.priority = Arc::new(|| None);
        state.jobs.insert("obsolete".into(), obsolete);
        let mut permanent = pending("permanent", "bttv", 1);
        permanent.paused = true;
        state.jobs.insert("permanent".into(), permanent);
        assert!(select_job(&state, &HashSet::new(), Instant::now()).is_none());
    }
    #[tokio::test]
    async fn startup_burst_keeps_one_incident_with_counts_and_classifies_bad_payloads() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let mut mock = server.mock(|when, then| {
            when.method(GET);
            then.status(503).body("unavailable");
        });
        let client = reqwest::Client::new();
        let mut state = State::default();
        let now = Instant::now();
        for channel in 0..17 {
            let error = crate::emote::providers::http::fetch_json::<serde_json::Value>(
                &client,
                "FFZ",
                format!("channel:{channel}"),
                &server.url("/emotes"),
            )
            .await
            .unwrap_err();
            let failure: Failure = error.into();
            assert!(failure.retryable);
            assert!(!failure.reportable);
            state.record_failure(
                &format!("ffz:{channel}"),
                "FFZ emotes",
                &failure,
                true,
                false,
                Some(Duration::from_secs(15)),
                now,
            );
        }
        assert_eq!(state.incidents.len(), 1);
        let incident = state.incidents.values().next().unwrap();
        assert_eq!(incident.issue.failures, 17);
        assert_eq!(incident.issue.affected, 17);
        assert_eq!(incident.suppressed, 16);
        mock.delete();
        let _bad = server.mock(|when, then| {
            when.method(GET);
            then.status(200).body("bad JSON");
        });
        let failure: Failure = crate::emote::providers::http::fetch_json::<serde_json::Value>(
            &client,
            "FFZ",
            "global",
            &server.url("/emotes"),
        )
        .await
        .unwrap_err()
        .into();
        assert!(!failure.retryable);
        assert!(failure.reportable);
    }
    #[test]
    fn server_retry_after_accepts_seconds_and_http_dates() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
        assert_eq!(retry_after(&headers), Duration::from_secs(120));
        headers.insert(
            reqwest::header::RETRY_AFTER,
            u64::MAX.to_string().parse().unwrap(),
        );
        assert_eq!(retry_after(&headers), Duration::ZERO);
        let date = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(120));
        headers.insert(reqwest::header::RETRY_AFTER, date.parse().unwrap());
        assert!((119..=120).contains(&retry_after(&headers).as_secs()));
    }
}

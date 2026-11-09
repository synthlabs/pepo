use super::{ChatEngine, EngineState};
use crate::types::{
    translation_input_from_fragments, ChannelMessage, ChannelMessageTranslation,
    ChannelMessageTranslationUpdate, ChatDetectionPolicy, TranslationQueuePolicy,
    TranslationSettings,
};
use futures::{future::BoxFuture, stream::FuturesUnordered, FutureExt, StreamExt};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use synth_translate::plugin::{
    dto::{ServicePhase, ServiceSnapshot},
    ErrorCode, ServiceError, TranslationService,
};
use synth_translate::{DetectionPolicy, TranslationOutcome, TranslationRequest};
use tokio::sync::watch;

#[derive(Clone)]
struct Job {
    id: u64,
    login: String,
    message_id: String,
    index: u64,
    live: bool,
    request: TranslationRequest,
}

struct Room {
    after: u64,
    before: u64,
}

struct Flight {
    job: Job,
    cancel: watch::Sender<bool>,
}

#[derive(Default)]
pub(super) struct Coordinator {
    settings: TranslationSettings,
    rooms: HashMap<String, Room>,
    waiting: VecDeque<Job>,
    running: HashMap<u64, Flight>,
    history_exclusions: HashSet<u64>,
    next_id: u64,
}

impl Coordinator {
    fn configure(&mut self, settings: TranslationSettings, cursor: u64) {
        let old = &self.settings;
        let changed = old.enabled != settings.enabled
            || old.target_language != settings.target_language
            || old.translate_history != settings.translate_history
            || old.detection_policy != settings.detection_policy
            || old.queue_policy != settings.queue_policy
            || old.queue_capacity != settings.queue_capacity
            || old.max_in_flight != settings.max_in_flight;
        self.settings = settings;
        if changed {
            self.cancel_all();
            self.seed_history(cursor);
        }
    }

    pub(super) fn open(&mut self, login: &str, cursor: u64) {
        let before = if self.settings.enabled && self.settings.translate_history {
            cursor + 1
        } else {
            0
        };
        self.rooms
            .entry(login.into())
            .or_insert(Room { after: 0, before });
    }

    pub(super) fn close(&mut self, login: &str) {
        self.rooms.remove(login);
        self.waiting.retain(|job| job.login != login);
        self.running.retain(|_, flight| {
            if flight.job.login == login {
                flight.cancel.send_replace(true);
                false
            } else {
                true
            }
        });
    }

    pub(super) fn cancel_all(&mut self) {
        self.waiting.clear();
        self.history_exclusions.clear();
        for (_, flight) in self.running.drain() {
            flight.cancel.send_replace(true);
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.cancel_all();
        self.rooms.clear();
    }

    fn seed_history(&mut self, cursor: u64) {
        self.history_exclusions = self
            .waiting
            .iter()
            .chain(self.running.values().map(|flight| &flight.job))
            .map(|job| job.index)
            .collect();
        for room in self.rooms.values_mut() {
            room.after = 0;
            room.before = if self.settings.enabled && self.settings.translate_history {
                cursor + 1
            } else {
                0
            };
        }
    }

    fn unfinished(&self) -> usize {
        self.waiting.len() + self.running.len()
    }

    fn contains(&self, login: &str, id: &str) -> bool {
        self.waiting
            .iter()
            .chain(self.running.values().map(|f| &f.job))
            .any(|job| job.login == login && job.message_id == id)
    }

    pub(super) fn admit(&mut self, message: &ChannelMessage, live: bool) {
        let login = message.broadcaster_user_login.to_lowercase();
        if !self.settings.enabled
            || !self.rooms.contains_key(&login)
            || message.translation.is_some()
            || self.contains(&login, &message.message_id)
        {
            return;
        }
        let Some(text) = translation_input_from_fragments(&message.fragments) else {
            return;
        };
        if self.unfinished() >= self.settings.queue_capacity {
            if !live || self.settings.queue_policy == TranslationQueuePolicy::OldestFirst {
                return;
            }
            let Some(oldest) = self
                .waiting
                .iter()
                .enumerate()
                .min_by_key(|(_, job)| job.index)
                .map(|(index, _)| index)
            else {
                return;
            };
            self.waiting.remove(oldest);
        }
        self.next_id += 1;
        let mut request =
            TranslationRequest::new(text, self.settings.target_language.parse().unwrap());
        request.detection_policy = match self.settings.detection_policy {
            ChatDetectionPolicy::ReliableOnly => DetectionPolicy::ReliableOnly,
            ChatDetectionPolicy::BestGuess => DetectionPolicy::BestGuess,
        };
        self.waiting.push_back(Job {
            id: self.next_id,
            login,
            message_id: message.message_id.clone(),
            index: message.index,
            live,
            request,
        });
    }

    fn take(&mut self) -> Option<(Job, watch::Receiver<bool>)> {
        if self.running.len()
            >= self
                .settings
                .max_in_flight
                .min(self.settings.queue_capacity)
        {
            return None;
        }
        let index = match self.settings.queue_policy {
            TranslationQueuePolicy::OldestFirst => self
                .waiting
                .iter()
                .enumerate()
                .min_by_key(|(_, job)| job.index)
                .map(|(index, _)| index),
            TranslationQueuePolicy::NewestFirst => self
                .waiting
                .iter()
                .enumerate()
                .max_by_key(|(_, job)| (job.live, job.index))
                .map(|(index, _)| index),
        }?;
        let job = self.waiting.remove(index).unwrap();
        let (cancel, receiver) = watch::channel(false);
        self.running.insert(
            job.id,
            Flight {
                job: job.clone(),
                cancel,
            },
        );
        Some((job, receiver))
    }
}

impl EngineState {
    fn refill_translations(&mut self) {
        let coordinator = &mut self.translations;
        if !coordinator.settings.enabled
            || !coordinator.settings.translate_history
            || coordinator.unfinished() >= coordinator.settings.queue_capacity
        {
            return;
        }
        // Cursors, rather than a second history queue, bound tracking to the open rooms.
        let mut candidates = Vec::new();
        let newest = coordinator.settings.queue_policy == TranslationQueuePolicy::NewestFirst;
        let limit = coordinator.settings.queue_capacity - coordinator.unfinished()
            + coordinator.history_exclusions.len();
        for (login, room) in &coordinator.rooms {
            if room.before > room.after + 1 {
                candidates.extend(self.repository.translation_candidates(
                    login,
                    room.after,
                    room.before,
                    newest,
                    limit,
                ));
            }
        }
        candidates.sort_unstable_by_key(|message| message.index);
        if newest {
            candidates.reverse();
        }
        let mut exhausted = true;
        for message in candidates {
            let login = message.broadcaster_user_login.to_lowercase();
            let room = coordinator.rooms.get_mut(&login).unwrap();
            match coordinator.settings.queue_policy {
                TranslationQueuePolicy::NewestFirst => room.before = message.index,
                TranslationQueuePolicy::OldestFirst => room.after = message.index,
            }
            if !coordinator.history_exclusions.remove(&message.index) {
                coordinator.admit(&message, false);
            }
            if coordinator.unfinished() >= coordinator.settings.queue_capacity {
                exhausted = false;
                break;
            }
        }
        if exhausted {
            for room in coordinator.rooms.values_mut() {
                room.before = 0;
            }
            coordinator.history_exclusions.clear();
        }
    }
}

impl ChatEngine {
    pub fn configure_translation(&self, settings: TranslationSettings) {
        let mut state = self.state.lock().unwrap();
        let cursor = state.cursor;
        state.translations.configure(settings.normalized(), cursor);
        drop(state);
        self.wake();
    }

    fn finish_translation(&self, id: u64, result: Result<TranslationOutcome, ServiceError>) {
        let mut state = self.state.lock().unwrap();
        let Some(flight) = state.translations.running.remove(&id) else {
            return;
        };
        if !state.active {
            return;
        }
        match result {
            Ok(TranslationOutcome::Translated(translation)) => {
                state.apply_translation(
                    &flight.job.login,
                    ChannelMessageTranslationUpdate {
                        message_id: flight.job.message_id,
                        translation: ChannelMessageTranslation {
                            source_language: translation.source.to_string(),
                            target_language: translation.target.to_string(),
                            translated_text: translation.text,
                        },
                    },
                );
            }
            Ok(TranslationOutcome::Skipped(_)) => {}
            Err(error) => {
                drop(state);
                report_error(&error);
                self.wake();
                return;
            }
        }
        drop(state);
        self.wake();
    }
}

fn report_error(error: &ServiceError) {
    match error.code {
        ErrorCode::InputTooLong => {
            tracing::debug!(?error.details, "chat translation input exceeds model limit")
        }
        ErrorCode::Cancelled
        | ErrorCode::Capacity
        | ErrorCode::Evicted
        | ErrorCode::NotReady
        | ErrorCode::Paused
        | ErrorCode::Shutdown
        | ErrorCode::UnsupportedPair
        | ErrorCode::NotInstalled => {}
        _ => crate::logging::error_repeated(
            format!("chat_translation:{:?}", error.code),
            format!(
                "Chat translation failed ({:?}): {}",
                error.code, error.message
            ),
            Duration::from_secs(300),
        ),
    }
}

#[derive(Default)]
struct Recovery {
    ready: bool,
    installed: String,
}

impl Recovery {
    fn observe(&mut self, snapshot: &ServiceSnapshot, target: &str) -> bool {
        let ready = snapshot.phase == ServicePhase::Ready;
        let mut recovered = ready && !self.ready;
        self.ready = ready;
        if let Some(installation) = &snapshot.installation {
            if installation.result.is_some() && self.installed != installation.id {
                self.installed.clone_from(&installation.id);
                recovered |= ready && installation.model.pair.target == target;
            }
        }
        recovered
    }
}

pub async fn run(engine: Arc<ChatEngine>, service: Arc<TranslationService>) {
    let snapshots = service.subscribe();
    run_with(engine, snapshots, move |request| {
        let service = service.clone();
        async move { service.submit(request).await?.result().await }
    })
    .await;
}

async fn run_with<F, Fut>(
    engine: Arc<ChatEngine>,
    mut snapshots: watch::Receiver<ServiceSnapshot>,
    submit: F,
) where
    F: Fn(TranslationRequest) -> Fut,
    Fut: Future<Output = Result<TranslationOutcome, ServiceError>> + Send + 'static,
{
    let mut changes = engine.changed.subscribe();
    let mut recovery = Recovery::default();
    let mut futures: FuturesUnordered<
        BoxFuture<'static, (u64, Result<TranslationOutcome, ServiceError>)>,
    > = FuturesUnordered::new();
    let mut service_error = None;
    loop {
        let snapshot = snapshots.borrow_and_update().clone();
        let jobs = {
            let mut state = engine.state.lock().unwrap();
            if !state.active {
                break;
            }
            if recovery.observe(&snapshot, &state.translations.settings.target_language) {
                let cursor = state.cursor;
                state.translations.seed_history(cursor);
            }
            if snapshot.phase == ServicePhase::Unavailable
                && state.translations.settings.enabled
                && !state.translations.rooms.is_empty()
                && service_error != snapshot.error
            {
                if let Some(error) = &snapshot.error {
                    report_error(error);
                }
                service_error = snapshot.error.clone();
            } else if snapshot.phase != ServicePhase::Unavailable
                || !state.translations.settings.enabled
                || state.translations.rooms.is_empty()
            {
                service_error = None;
            }
            let mut jobs = Vec::new();
            if snapshot.phase == ServicePhase::Ready {
                state.refill_translations();
                while futures.len() + jobs.len() < 256 {
                    let Some(job) = state.translations.take() else {
                        break;
                    };
                    jobs.push(job);
                }
            }
            jobs
        };
        for (job, mut cancel) in jobs {
            let result = submit(job.request);
            futures.push(async move {
                let result = tokio::select! {
                    biased;
                    _ = async { if !*cancel.borrow() { let _ = cancel.changed().await; } } =>
                        Err(ServiceError::new(ErrorCode::Cancelled, "chat translation cancelled")),
                    result = result => result,
                };
                (job.id, result)
            }.boxed());
        }
        tokio::select! {
            biased;
            Some((id, result)) = futures.next(), if !futures.is_empty() => engine.finish_translation(id, result),
            result = changes.changed() => { if result.is_err() { break; } }
            result = snapshots.changed() => { if result.is_err() { break; } }
        }
    }
}

#[cfg(test)]
mod tests;

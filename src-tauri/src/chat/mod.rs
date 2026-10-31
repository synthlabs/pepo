pub mod admission;
pub mod membership;
pub mod runtime;
pub mod store;

use crate::types::{ChannelMessage, ChannelMessageTranslationUpdate, ChatRoomInfo, ChatSettings};
use admission::{retry_delay, RoomFailure};
use membership::Membership;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use store::{MemoryRepository, MessageRepository, StoreMetrics};
use tokio::sync::watch;

pub const FEED_CAPACITY: usize = 1024;
pub const DELIVERY_WINDOW: usize = 128;
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, specta::Type)]
pub struct ChatFeedContext {
    pub epoch: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, specta::Type)]
pub struct ChatFeedRequest {
    pub epoch: String,
    pub revision: u64,
}

struct FeedOwner {
    epoch: String,
    revision: u64,
    pending: bool,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeedEvent {
    Snapshot {
        messages: Vec<ChannelMessage>,
    },
    Message {
        message: ChannelMessage,
    },
    Translation {
        update: ChannelMessageTranslationUpdate,
    },
    Reset {
        messages: Vec<ChannelMessage>,
    },
    Gap {
        after: u64,
        through: u64,
    },
    Unavailable {
        error: String,
    },
}

#[derive(Clone, Debug, Serialize, specta::Type)]
pub struct FeedFrame {
    pub cursor: u64,
    pub event: FeedEvent,
}

#[derive(Clone, Debug, Default, Serialize, specta::Type, PartialEq, Eq)]
pub struct EngineStatus {
    pub connected: bool,
    pub joined: usize,
    pub eligible: usize,
    pub lingering: usize,
    pub pending: usize,
    pub limited: usize,
    pub failures: Vec<String>,
}

#[derive(Clone)]
pub enum FeedScope {
    Channel(String),
    All,
}

struct Subscriber {
    scope: FeedScope,
    queue: VecDeque<Arc<FeedFrame>>,
    in_flight: VecDeque<u64>,
    acknowledged: u64,
    lagged: bool,
}

pub struct RoomLease {
    pub request: ChatFeedRequest,
    pub login: String,
    pub feed_id: u64,
}

pub struct EngineState {
    pub active: bool,
    repository: Box<dyn MessageRepository>,
    pub membership: Membership,
    pub directory: HashMap<String, ChatRoomInfo>,
    subscribers: HashMap<u64, Subscriber>,
    pub leases: HashMap<String, RoomLease>,
    owners: HashMap<String, FeedOwner>,
    pub failures: HashMap<String, RoomFailure>,
    cursor: u64,
    next_feed: u64,
    pub settings: ChatSettings,
    pub status: EngineStatus,
}

pub struct ChatEngine {
    pub state: Mutex<EngineState>,
    changed: watch::Sender<u64>,
}

impl ChatEngine {
    pub fn new(settings: ChatSettings) -> Arc<Self> {
        let (changed, _) = watch::channel(0);
        Arc::new(Self {
            state: Mutex::new(EngineState {
                active: true,
                repository: Box::new(MemoryRepository::new(settings.message_limit)),
                membership: Membership::default(),
                directory: HashMap::new(),
                subscribers: HashMap::new(),
                leases: HashMap::new(),
                owners: HashMap::new(),
                failures: HashMap::new(),
                cursor: 0,
                next_feed: 0,
                settings,
                status: EngineStatus::default(),
            }),
            changed,
        })
    }
    pub fn wake(&self) {
        self.changed
            .send_modify(|value| *value = value.wrapping_add(1));
    }
    pub fn active(&self) -> bool {
        self.state.lock().unwrap().active
    }
    pub fn insert(&self, mut message: ChannelMessage) -> bool {
        let mut s = self.state.lock().unwrap();
        if !s.active {
            return false;
        }
        let login = message.broadcaster_user_login.to_lowercase();
        message.index = s.cursor + 1;
        let Some(message) = s.repository.insert(message) else {
            return false;
        };
        s.cursor += 1;
        s.publish(&login, FeedEvent::Message { message });
        drop(s);
        self.wake();
        true
    }
    pub fn translate(&self, login: &str, update: ChannelMessageTranslationUpdate) {
        let mut s = self.state.lock().unwrap();
        if !s.active {
            return;
        }
        // Relay late results to a paused UI even if backend retention already evicted the message.
        s.repository.update(login, &update);
        s.cursor += 1;
        s.publish(login, FeedEvent::Translation { update });
        drop(s);
        self.wake();
    }
    pub fn subscribe(self: &Arc<Self>, scope: FeedScope) -> FeedSubscription {
        let changed = self.changed.subscribe();
        let mut s = self.state.lock().unwrap();
        s.next_feed += 1;
        let id = s.next_feed;
        let mut queue = VecDeque::new();
        if let FeedScope::Channel(login) = &scope {
            queue.push_back(Arc::new(FeedFrame {
                cursor: s.cursor,
                event: FeedEvent::Snapshot {
                    messages: s.repository.history(login),
                },
            }));
            if let Some(failure) = s.failures.get(login) {
                queue.push_back(Arc::new(FeedFrame {
                    cursor: s.cursor,
                    event: FeedEvent::Unavailable {
                        error: failure.reason.clone(),
                    },
                }));
            }
        }
        let acknowledged = s.cursor;
        if s.active {
            s.subscribers.insert(
                id,
                Subscriber {
                    scope,
                    queue,
                    in_flight: VecDeque::new(),
                    acknowledged,
                    lagged: false,
                },
            );
        }
        FeedSubscription {
            engine: Arc::downgrade(self),
            id,
            changed,
        }
    }
    pub fn feed_context(&self, owner: &str) -> ChatFeedContext {
        let mut s = self.state.lock().unwrap();
        let record = s
            .owners
            .entry(owner.to_owned())
            .or_insert_with(|| FeedOwner {
                epoch: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed).to_string(),
                revision: 0,
                pending: false,
            });
        ChatFeedContext {
            epoch: record.epoch.clone(),
        }
    }

    pub fn begin_request(&self, owner: &str, request: &ChatFeedRequest) -> bool {
        let mut s = self.state.lock().unwrap();
        if !s.active || request.revision == 0 || request.revision > 9_007_199_254_740_991 {
            return false;
        }
        let Some(record) = s.owners.get(owner) else {
            return false;
        };
        if record.epoch != request.epoch || record.revision >= request.revision {
            return false;
        }
        s.detach_owner(owner);
        let record = s.owners.get_mut(owner).unwrap();
        record.revision = request.revision;
        record.pending = true;
        drop(s);
        self.wake();
        true
    }

    pub fn attach(
        &self,
        owner: &str,
        request: &ChatFeedRequest,
        login: String,
        feed_id: u64,
    ) -> bool {
        let mut s = self.state.lock().unwrap();
        if !s.active
            || !s.owners.get(owner).is_some_and(|record| {
                record.epoch == request.epoch
                    && record.revision == request.revision
                    && record.pending
            })
        {
            return false;
        }
        s.owners.get_mut(owner).unwrap().pending = false;
        s.membership.foreground.insert(login.clone());
        s.membership.lingering.insert(login.clone());
        if let Some(failure) = s.failures.get_mut(&login) {
            failure.retry_at = Instant::now();
        }
        s.leases.insert(
            owner.to_owned(),
            RoomLease {
                request: request.clone(),
                login,
                feed_id,
            },
        );
        drop(s);
        self.wake();
        true
    }
    pub fn detach(&self, owner: &str, request: &ChatFeedRequest) {
        let mut s = self.state.lock().unwrap();
        if let Some(record) = s.owners.get_mut(owner) {
            if record.epoch == request.epoch
                && request.revision >= record.revision
                && request.revision <= 9_007_199_254_740_991
            {
                record.revision = request.revision;
                record.pending = false;
                s.detach_owner(owner);
            }
        }
        drop(s);
        self.wake();
    }
    pub fn close_owner(&self, owner: &str) {
        let mut s = self.state.lock().unwrap();
        s.detach_owner(owner);
        s.owners.remove(owner);
        drop(s);
        self.wake();
    }
    pub fn request_active(&self, owner: &str, request: &ChatFeedRequest) -> bool {
        let s = self.state.lock().unwrap();
        s.active
            && s.leases
                .get(owner)
                .is_some_and(|lease| &lease.request == request)
    }

    pub fn acknowledge(&self, owner: &str, request: &ChatFeedRequest, cursor: u64) {
        let mut s = self.state.lock().unwrap();
        let id = s
            .leases
            .get(owner)
            .filter(|lease| &lease.request == request)
            .map(|lease| lease.feed_id);
        if let Some(id) = id {
            s.acknowledge(id, cursor);
        }
        drop(s);
        self.wake();
    }
    pub fn configure(&self, settings: ChatSettings) {
        let mut s = self.state.lock().unwrap();
        s.repository.set_limit(settings.message_limit);
        s.settings = settings;
        drop(s);
        self.wake();
    }
    pub fn history(&self, login: &str) -> Vec<ChannelMessage> {
        self.state.lock().unwrap().repository.history(login)
    }
    pub fn metrics(&self) -> StoreMetrics {
        self.state.lock().unwrap().repository.metrics()
    }
    pub fn shutdown(&self) {
        let mut s = self.state.lock().unwrap();
        s.active = false;
        s.repository.clear();
        s.subscribers.clear();
        s.leases.clear();
        s.directory.clear();
        s.owners.clear();
        s.failures.clear();
        s.membership = Membership::default();
        drop(s);
        self.wake();
    }
}

impl EngineState {
    fn detach_owner(&mut self, owner: &str) {
        if let Some(lease) = self.leases.remove(owner) {
            self.subscribers.remove(&lease.feed_id);
            if !self.leases.values().any(|other| other.login == lease.login) {
                self.membership.foreground.remove(&lease.login);
            }
        }
    }
    pub fn unavailable(&mut self, login: &str, error: String) {
        self.fail_room(login, error, Instant::now());
    }
    pub fn fail_room(&mut self, login: &str, error: String, now: Instant) {
        let previous = self.failures.get(login);
        let changed = previous.is_none_or(|failure| failure.reason != error);
        let attempts = previous.map_or(1, |failure| failure.attempts.saturating_add(1));
        self.failures.insert(
            login.to_owned(),
            RoomFailure {
                reason: error.clone(),
                attempts,
                retry_at: now + retry_delay(attempts),
            },
        );
        if !changed {
            return;
        }
        self.cursor += 1;
        self.publish(login, FeedEvent::Unavailable { error });
    }
    fn publish(&mut self, login: &str, event: FeedEvent) {
        let frame = Arc::new(FeedFrame {
            cursor: self.cursor,
            event,
        });
        for subscriber in self.subscribers.values_mut() {
            if matches!(&subscriber.scope, FeedScope::Channel(channel) if channel != login) {
                continue;
            }
            if subscriber.lagged {
                continue;
            }
            if subscriber.queue.len() >= FEED_CAPACITY {
                subscriber.queue.clear();
                subscriber.lagged = true;
            } else {
                subscriber.queue.push_back(frame.clone());
            }
        }
    }
    fn acknowledge(&mut self, id: u64, cursor: u64) {
        if let Some(sub) = self.subscribers.get_mut(&id) {
            while sub.in_flight.front().is_some_and(|value| *value <= cursor) {
                sub.acknowledged = sub.in_flight.pop_front().unwrap();
            }
        }
    }
}

pub struct FeedSubscription {
    engine: Weak<ChatEngine>,
    pub id: u64,
    changed: watch::Receiver<u64>,
}

impl FeedSubscription {
    pub fn poll(&self) -> Vec<Arc<FeedFrame>> {
        let Some(engine) = self.engine.upgrade() else {
            return vec![];
        };
        let mut s = engine.state.lock().unwrap();
        let Some(mut sub) = s.subscribers.remove(&self.id) else {
            return vec![];
        };
        let mut frames = Vec::new();
        if sub.in_flight.len() < DELIVERY_WINDOW {
            if sub.lagged {
                let event = match &sub.scope {
                    FeedScope::Channel(login) => FeedEvent::Reset {
                        messages: s.repository.history(login),
                    },
                    FeedScope::All => FeedEvent::Gap {
                        after: sub.in_flight.back().copied().unwrap_or(sub.acknowledged),
                        through: s.cursor,
                    },
                };
                sub.queue.clear();
                sub.lagged = false;
                sub.queue.push_back(Arc::new(FeedFrame {
                    cursor: s.cursor,
                    event,
                }));
            }
            while sub.in_flight.len() < DELIVERY_WINDOW {
                let Some(frame) = sub.queue.pop_front() else {
                    break;
                };
                sub.in_flight.push_back(frame.cursor);
                frames.push(frame);
            }
        }
        s.subscribers.insert(self.id, sub);
        frames
    }
    pub fn acknowledge(&self, cursor: u64) {
        if let Some(engine) = self.engine.upgrade() {
            engine.state.lock().unwrap().acknowledge(self.id, cursor);
            engine.wake();
        }
    }
    pub async fn changed(&mut self) -> bool {
        if !self.engine.upgrade().is_some_and(|e| {
            let s = e.state.lock().unwrap();
            s.active && s.subscribers.contains_key(&self.id)
        }) {
            return false;
        }
        if self.changed.changed().await.is_err() {
            return false;
        }
        self.engine.upgrade().is_some_and(|e| {
            let s = e.state.lock().unwrap();
            s.active && s.subscribers.contains_key(&self.id)
        })
    }
}

impl Drop for FeedSubscription {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.upgrade() {
            engine.state.lock().unwrap().subscribers.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests;

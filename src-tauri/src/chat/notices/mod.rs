pub mod irc;
pub mod runtime;
#[cfg(test)]
mod tests;

use super::timeline::{SystemMessageIcon, SystemMessageInput, SystemMessageSegment};
use crate::types::ChatRoomInfo;
use futures::future::BoxFuture;
use std::collections::{BTreeMap, HashSet, VecDeque};
use tokio::sync::{mpsc, watch};

pub const NOTICE_CAPACITY: usize = 128;
const RECENT_ID_CAPACITY: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeAction {
    UserBanned {
        user_id: String,
        login: String,
    },
    UserTimedOut {
        user_id: String,
        login: String,
        duration_secs: u64,
    },
    ChatCleared,
}

#[derive(Clone, Debug)]
pub struct NoticeEvent {
    pub id: String,
    pub timestamp: String,
    pub channel_id: String,
    pub generation: u64,
    pub action: NoticeAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticeSubscription {
    pub room: ChatRoomInfo,
    pub generation: u64,
}

pub type Roster = BTreeMap<String, NoticeSubscription>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderHealth {
    pub issue: Option<String>,
    pub coverage_gaps: u64,
}

pub trait NoticeProvider: Send {
    fn run(
        self: Box<Self>,
        desired: watch::Receiver<Roster>,
        sink: NoticeSink,
        health: watch::Sender<ProviderHealth>,
    ) -> BoxFuture<'static, ()>;
}

#[derive(Clone)]
pub struct NoticeSink {
    sender: mpsc::Sender<NoticeEvent>,
    health: watch::Sender<ProviderHealth>,
}

impl NoticeSink {
    pub fn new(sender: mpsc::Sender<NoticeEvent>, health: watch::Sender<ProviderHealth>) -> Self {
        Self { sender, health }
    }

    pub fn emit(&self, event: NoticeEvent) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.sender.try_send(event) {
            self.health.send_modify(|status| {
                status.coverage_gaps = status.coverage_gaps.saturating_add(1)
            });
        }
    }
}

#[derive(Default)]
pub struct RecentIds {
    ids: HashSet<String>,
    order: VecDeque<String>,
}

impl RecentIds {
    pub fn admit(&mut self, id: &str) -> bool {
        if !self.ids.insert(id.to_owned()) {
            return false;
        }
        self.order.push_back(id.to_owned());
        if self.order.len() > RECENT_ID_CAPACITY {
            if let Some(expired) = self.order.pop_front() {
                self.ids.remove(&expired);
            }
        }
        true
    }
}

pub struct Subscriptions {
    pub roster: watch::Sender<Roster>,
    next_generation: u64,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            roster: watch::channel(Roster::new()).0,
            next_generation: 0,
        }
    }
}

impl Subscriptions {
    pub fn reconcile(&mut self, rooms: impl IntoIterator<Item = ChatRoomInfo>) {
        let current = self.roster.borrow().clone();
        let mut next = Roster::new();
        for room in rooms {
            let generation = current
                .get(&room.broadcaster_login)
                .filter(|sub| sub.room.broadcaster_id == room.broadcaster_id)
                .map(|sub| sub.generation)
                .unwrap_or_else(|| {
                    self.next_generation += 1;
                    self.next_generation
                });
            next.insert(
                room.broadcaster_login.clone(),
                NoticeSubscription { room, generation },
            );
        }
        if next != current {
            self.roster.send_replace(next);
        }
    }

    pub fn accepts(&self, event: &NoticeEvent) -> Option<ChatRoomInfo> {
        self.roster
            .borrow()
            .values()
            .find(|sub| {
                sub.room.broadcaster_id == event.channel_id && sub.generation == event.generation
            })
            .map(|sub| sub.room.clone())
    }
}

pub fn format_notice(event: NoticeEvent) -> SystemMessageInput {
    let (user, text, icon) = match event.action {
        NoticeAction::UserBanned { user_id, login } => (
            Some((user_id, login)),
            " was banned.".into(),
            SystemMessageIcon::Shield,
        ),
        NoticeAction::UserTimedOut {
            user_id,
            login,
            duration_secs,
        } => (
            Some((user_id, login)),
            format!(" was timed out for {}.", format_duration(duration_secs)),
            SystemMessageIcon::Clock,
        ),
        NoticeAction::ChatCleared => (None, "Chat was cleared.".into(), SystemMessageIcon::Shield),
    };
    let mut segments = Vec::new();
    if let Some((user_id, login)) = user {
        segments.push(SystemMessageSegment::User {
            user_id,
            display_name: login.clone(),
            login,
        });
    }
    segments.push(SystemMessageSegment::Text { text });
    SystemMessageInput {
        message_id: event.id,
        ts: event.timestamp,
        segments,
        icon: Some(icon),
    }
}

fn format_duration(mut seconds: u64) -> String {
    let mut parts = Vec::new();
    for (unit, suffix) in [(86400, "d"), (3600, "h"), (60, "m"), (1, "s")] {
        let value = seconds / unit;
        seconds %= unit;
        if value != 0 {
            parts.push(format!("{value}{suffix}"));
        }
    }
    if parts.is_empty() {
        return "0s".into();
    }
    parts.join(" ")
}

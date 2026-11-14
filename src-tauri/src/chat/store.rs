use super::timeline::{ChatTimelineEntry, SystemMessageSegment};
use crate::types::{ChannelMessage, ChannelMessageTranslationUpdate};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, Default, serde::Serialize, specta::Type)]
pub struct StoreMetrics {
    pub channels_with_history: usize,
    pub retained_messages: usize,
    pub estimated_history_bytes: usize,
    pub history_limit_per_channel: usize,
}

pub trait MessageRepository: Send {
    fn insert(&mut self, message: ChatTimelineEntry) -> Option<ChatTimelineEntry>;
    fn history(&self, login: &str) -> Vec<ChatTimelineEntry>;
    fn translation_candidates(
        &self,
        login: &str,
        after: u64,
        before: u64,
        newest: bool,
        limit: usize,
    ) -> Vec<ChannelMessage>;
    fn update(&mut self, login: &str, update: &ChannelMessageTranslationUpdate) -> bool;
    fn set_limit(&mut self, limit: usize);
    fn metrics(&self) -> StoreMetrics;
    fn clear(&mut self);
}

struct Entry {
    message: ChatTimelineEntry,
    bytes: usize,
}

pub struct MemoryRepository {
    rooms: HashMap<String, VecDeque<Entry>>,
    logins: HashMap<String, String>,
    next_chat_sequences: HashMap<String, u64>,
    metrics: StoreMetrics,
}

impl MemoryRepository {
    pub fn new(limit: usize) -> Self {
        Self {
            rooms: HashMap::new(),
            logins: HashMap::new(),
            next_chat_sequences: HashMap::new(),
            metrics: StoreMetrics {
                history_limit_per_channel: limit.max(1),
                ..Default::default()
            },
        }
    }
    fn trim(&mut self) {
        for room in self.rooms.values_mut() {
            while room.len() > self.metrics.history_limit_per_channel {
                let entry = room.pop_front().unwrap();
                self.metrics.retained_messages -= 1;
                self.metrics.estimated_history_bytes -= entry.bytes;
            }
        }
    }
}

impl MessageRepository for MemoryRepository {
    fn insert(&mut self, mut message: ChatTimelineEntry) -> Option<ChatTimelineEntry> {
        let room = self
            .rooms
            .entry(message.broadcaster_id().to_owned())
            .or_default();
        if room
            .iter()
            .any(|entry| entry.message.same_identity(&message))
        {
            return None;
        }
        if let ChatTimelineEntry::Chat(chat) = &mut message {
            let sequence = self
                .next_chat_sequences
                .entry(chat.broadcaster_user_id.clone())
                .or_default();
            chat.channel_sequence = *sequence;
            *sequence += 1;
        }
        if room.is_empty() {
            self.metrics.channels_with_history += 1;
        }
        self.logins.insert(
            message.login().to_lowercase(),
            message.broadcaster_id().to_owned(),
        );
        let bytes = estimated_entry_bytes(&message);
        let accepted = message.clone();
        room.push_back(Entry { message, bytes });
        self.metrics.retained_messages += 1;
        self.metrics.estimated_history_bytes += bytes;
        self.trim();
        Some(accepted)
    }
    fn history(&self, login: &str) -> Vec<ChatTimelineEntry> {
        self.logins
            .get(login)
            .and_then(|id| self.rooms.get(id))
            .map(|room| room.iter().map(|e| e.message.clone()).collect())
            .unwrap_or_default()
    }
    fn translation_candidates(
        &self,
        login: &str,
        after: u64,
        before: u64,
        newest: bool,
        limit: usize,
    ) -> Vec<ChannelMessage> {
        let Some(room) = self.logins.get(login).and_then(|id| self.rooms.get(id)) else {
            return vec![];
        };
        let eligible = |message: &&ChannelMessage| {
            message.index > after
                && message.index < before
                && message.translation.is_none()
                && crate::types::translation_input_from_fragments(&message.fragments).is_some()
        };
        if newest {
            room.iter()
                .rev()
                .filter_map(|entry| entry.message.as_chat())
                .filter(eligible)
                .take(limit)
                .cloned()
                .collect()
        } else {
            room.iter()
                .filter_map(|entry| entry.message.as_chat())
                .filter(eligible)
                .take(limit)
                .cloned()
                .collect()
        }
    }
    fn update(&mut self, login: &str, update: &ChannelMessageTranslationUpdate) -> bool {
        let Some(room) = self.logins.get(login).and_then(|id| self.rooms.get_mut(id)) else {
            return false;
        };
        let Some(entry) = room.iter_mut().find(|e| {
            e.message
                .as_chat()
                .is_some_and(|m| m.message_id == update.message_id)
        }) else {
            return false;
        };
        let ChatTimelineEntry::Chat(message) = &mut entry.message else {
            return false;
        };
        if message.translation.as_ref() == Some(&update.translation) {
            return false;
        }
        self.metrics.estimated_history_bytes -= entry.bytes;
        message.translation = Some(update.translation.clone());
        entry.bytes = estimated_entry_bytes(&entry.message);
        self.metrics.estimated_history_bytes += entry.bytes;
        true
    }
    fn set_limit(&mut self, limit: usize) {
        self.metrics.history_limit_per_channel = limit.max(1);
        self.trim();
    }
    fn metrics(&self) -> StoreMetrics {
        self.metrics.clone()
    }
    fn clear(&mut self) {
        self.rooms.clear();
        self.logins.clear();
        self.next_chat_sequences.clear();
        self.metrics = StoreMetrics {
            history_limit_per_channel: self.metrics.history_limit_per_channel,
            ..Default::default()
        };
    }
}

pub fn estimated_entry_bytes(entry: &ChatTimelineEntry) -> usize {
    let ChatTimelineEntry::System(m) = entry else {
        return estimated_message_bytes(entry.as_chat().unwrap())
            + std::mem::size_of::<ChatTimelineEntry>()
            - std::mem::size_of::<ChannelMessage>();
    };
    std::mem::size_of::<ChatTimelineEntry>()
        + strings(&[
            &m.ts,
            &m.message_id,
            &m.broadcaster_user_id,
            &m.broadcaster_user_login,
            &m.broadcaster_user_name,
        ])
        + m.segments.capacity() * std::mem::size_of::<SystemMessageSegment>()
        + m.segments
            .iter()
            .map(|segment| match segment {
                SystemMessageSegment::Text { text } | SystemMessageSegment::Emphasis { text } => {
                    text.capacity()
                }
                SystemMessageSegment::User {
                    user_id,
                    login,
                    display_name,
                } => strings(&[user_id, login, display_name]),
                SystemMessageSegment::Link { text, url } => strings(&[text, url]),
            })
            .sum::<usize>()
}

fn strings(values: &[&String]) -> usize {
    values.iter().map(|s| s.capacity()).sum()
}
fn string_vec(values: &Vec<String>) -> usize {
    values.capacity() * std::mem::size_of::<String>()
        + values.iter().map(String::capacity).sum::<usize>()
}
pub fn estimated_message_bytes(m: &ChannelMessage) -> usize {
    use crate::message::Fragment;
    let mut bytes = std::mem::size_of::<ChannelMessage>()
        + strings(&[
            &m.ts,
            &m.broadcaster_user_id,
            &m.broadcaster_user_name,
            &m.broadcaster_user_login,
            &m.chatter_user_id,
            &m.chatter_user_name,
            &m.message_id,
            &m.text,
            &m.color,
        ])
        + m.fragments.capacity() * std::mem::size_of::<Fragment>()
        + m.badges.capacity() * std::mem::size_of::<crate::types::BadgeRef>();
    for fragment in &m.fragments {
        bytes += match fragment {
            Fragment::Text(f) => f.text.capacity(),
            Fragment::Cheer(f) => f.text.capacity(),
            Fragment::Emote(f) => {
                let e = &f.emote;
                strings(&[
                    &e.id,
                    &e.name,
                    &e.tier,
                    &e.emote_type,
                    &e.emote_set_id,
                    &e.url,
                    &e.provider,
                    &e.scope,
                ]) + string_vec(&e.format)
                    + string_vec(&e.scale)
                    + string_vec(&e.theme_mode)
            }
        };
    }
    for badge in &m.badges {
        bytes += strings(&[&badge.set_id, &badge.id, &badge.info]);
        // Badge metadata is owned by the history record as well.
        let b = &badge.badge;
        bytes += strings(&[
            &b.set_id,
            &b.id,
            &b.image_url_1x,
            &b.image_url_2x,
            &b.image_url_4x,
            &b.title,
            &b.description,
        ]);
    }
    if let Some(t) = &m.translation {
        bytes += strings(&[&t.source_language, &t.target_language, &t.translated_text]);
    }
    bytes
}

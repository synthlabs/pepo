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
    fn insert(&mut self, message: ChannelMessage) -> Option<ChannelMessage>;
    fn history(&self, login: &str) -> Vec<ChannelMessage>;
    fn update(&mut self, login: &str, update: &ChannelMessageTranslationUpdate) -> bool;
    fn set_limit(&mut self, limit: usize);
    fn metrics(&self) -> StoreMetrics;
    fn clear(&mut self);
}

struct Entry {
    message: ChannelMessage,
    bytes: usize,
}

pub struct MemoryRepository {
    rooms: HashMap<String, VecDeque<Entry>>,
    logins: HashMap<String, String>,
    metrics: StoreMetrics,
}

impl MemoryRepository {
    pub fn new(limit: usize) -> Self {
        Self {
            rooms: HashMap::new(),
            logins: HashMap::new(),
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
    fn insert(&mut self, mut message: ChannelMessage) -> Option<ChannelMessage> {
        let room = self
            .rooms
            .entry(message.broadcaster_user_id.clone())
            .or_default();
        if room
            .iter()
            .any(|entry| entry.message.message_id == message.message_id)
        {
            return None;
        }
        message.channel_sequence = room
            .back()
            .map_or(0, |entry| entry.message.channel_sequence + 1);
        if room.is_empty() {
            self.metrics.channels_with_history += 1;
        }
        self.logins.insert(
            message.broadcaster_user_login.to_lowercase(),
            message.broadcaster_user_id.clone(),
        );
        let bytes = estimated_message_bytes(&message);
        let accepted = message.clone();
        room.push_back(Entry { message, bytes });
        self.metrics.retained_messages += 1;
        self.metrics.estimated_history_bytes += bytes;
        self.trim();
        Some(accepted)
    }
    fn history(&self, login: &str) -> Vec<ChannelMessage> {
        self.logins
            .get(login)
            .and_then(|id| self.rooms.get(id))
            .map(|room| room.iter().map(|e| e.message.clone()).collect())
            .unwrap_or_default()
    }
    fn update(&mut self, login: &str, update: &ChannelMessageTranslationUpdate) -> bool {
        let Some(room) = self.logins.get(login).and_then(|id| self.rooms.get_mut(id)) else {
            return false;
        };
        let Some(entry) = room
            .iter_mut()
            .find(|e| e.message.message_id == update.message_id)
        else {
            return false;
        };
        if entry.message.translation.as_ref() == Some(&update.translation) {
            return false;
        }
        self.metrics.estimated_history_bytes -= entry.bytes;
        entry.message.translation = Some(update.translation.clone());
        entry.bytes = estimated_message_bytes(&entry.message);
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
        self.metrics = StoreMetrics {
            history_limit_per_channel: self.metrics.history_limit_per_channel,
            ..Default::default()
        };
    }
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

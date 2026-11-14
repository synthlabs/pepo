use crate::types::{ChannelMessage, ChatRoomInfo};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemMessageSegment {
    Text {
        text: String,
    },
    Emphasis {
        text: String,
    },
    User {
        user_id: String,
        login: String,
        display_name: String,
    },
    Link {
        text: String,
        url: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SystemMessageIcon {
    Info,
    Shield,
    Clock,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct SystemMessageInput {
    pub message_id: String,
    pub ts: String,
    pub segments: Vec<SystemMessageSegment>,
    pub icon: Option<SystemMessageIcon>,
}

impl SystemMessageInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.message_id.trim().is_empty() {
            return Err("System messages require an ID".into());
        }
        if self.ts.parse::<twitch_api::types::Timestamp>().is_err() {
            return Err("System messages require a valid timestamp".into());
        }
        let mut has_content = false;
        for segment in &self.segments {
            let text = match segment {
                SystemMessageSegment::Text { text } | SystemMessageSegment::Emphasis { text } => {
                    text
                }
                SystemMessageSegment::User {
                    user_id,
                    login,
                    display_name,
                } => {
                    if user_id.trim().is_empty() || login.trim().is_empty() {
                        return Err("User references require an ID and login".into());
                    }
                    display_name
                }
                SystemMessageSegment::Link { text, url } => {
                    let parsed =
                        reqwest::Url::parse(url).map_err(|_| "Invalid system message link")?;
                    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                        return Err("System message links must use HTTP or HTTPS".into());
                    }
                    text
                }
            };
            has_content |= !text.trim().is_empty();
        }
        if !has_content {
            return Err("System messages require content".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct SystemMessage {
    pub index: u64,
    pub ts: String,
    pub message_id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub segments: Vec<SystemMessageSegment>,
    pub icon: Option<SystemMessageIcon>,
}

impl SystemMessage {
    pub fn new(room: ChatRoomInfo, input: SystemMessageInput) -> Result<Self, String> {
        input.validate()?;
        if room.broadcaster_id.trim().is_empty() || room.broadcaster_login.trim().is_empty() {
            return Err("System messages require a channel ID and login".into());
        }
        Ok(Self {
            index: 0,
            ts: input.ts,
            message_id: input.message_id,
            broadcaster_user_id: room.broadcaster_id,
            broadcaster_user_login: room.broadcaster_login.to_lowercase(),
            broadcaster_user_name: room.broadcaster_name,
            segments: input.segments,
            icon: input.icon,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatTimelineEntry {
    Chat(ChannelMessage),
    System(SystemMessage),
}

impl ChatTimelineEntry {
    pub fn index(&self) -> u64 {
        match self {
            Self::Chat(m) => m.index,
            Self::System(m) => m.index,
        }
    }

    pub fn set_index(&mut self, index: u64) {
        match self {
            Self::Chat(m) => m.index = index,
            Self::System(m) => m.index = index,
        }
    }

    pub fn message_id(&self) -> &str {
        match self {
            Self::Chat(m) => &m.message_id,
            Self::System(m) => &m.message_id,
        }
    }

    pub fn broadcaster_id(&self) -> &str {
        match self {
            Self::Chat(m) => &m.broadcaster_user_id,
            Self::System(m) => &m.broadcaster_user_id,
        }
    }

    pub fn login(&self) -> &str {
        match self {
            Self::Chat(m) => &m.broadcaster_user_login,
            Self::System(m) => &m.broadcaster_user_login,
        }
    }

    pub fn as_chat(&self) -> Option<&ChannelMessage> {
        match self {
            Self::Chat(m) => Some(m),
            Self::System(_) => None,
        }
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && self.message_id() == other.message_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatEngine, FeedEvent, FeedScope, FEED_CAPACITY};
    use crate::types::{ChannelMessageTranslation, ChannelMessageTranslationUpdate, ChatSettings};

    fn room() -> ChatRoomInfo {
        ChatRoomInfo {
            broadcaster_id: "1".into(),
            broadcaster_login: "room".into(),
            broadcaster_name: "Room".into(),
        }
    }

    fn input(id: &str) -> SystemMessageInput {
        SystemMessageInput {
            message_id: id.into(),
            ts: "2026-11-14T19:00:00Z".into(),
            segments: vec![SystemMessageSegment::Text {
                text: "A notice".into(),
            }],
            icon: Some(SystemMessageIcon::Info),
        }
    }

    fn chat(id: &str) -> ChannelMessage {
        serde_json::from_value(serde_json::json!({
            "ts": "2026-11-14T19:00:00Z", "broadcaster_user_id": "1", "broadcaster_user_login": "room", "broadcaster_user_name": "Room",
            "chatter_user_id": "2", "chatter_user_name": "Viewer", "message_id": id,
            "text": "Hello", "fragments": [{ "Text": { "index": 0, "text": "Hello" } }],
            "message_type": "text", "badges": [], "color": "", "translation": null, "index": 0, "channel_sequence": 0
        })).unwrap()
    }

    #[test]
    fn mixed_entries_share_canonical_history_and_live_delivery() {
        let engine = ChatEngine::new(ChatSettings::default());
        let feed = engine.subscribe(FeedScope::Channel("room".into()));
        let other = engine.subscribe(FeedScope::Channel("other".into()));
        assert!(engine.insert(chat("same")));
        assert!(engine.insert_system(room(), input("same")).unwrap());
        assert!(!engine.insert_system(room(), input("same")).unwrap());
        assert!(!engine.insert(chat("same")));
        assert!(engine.insert(chat("next")));
        let history = engine.history("room");
        assert_eq!(
            history
                .iter()
                .map(ChatTimelineEntry::index)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(history[2].as_chat().unwrap().channel_sequence, 1);
        assert!(matches!(history[1], ChatTimelineEntry::System(_)));
        assert_eq!(other.poll().len(), 1);
        let frames = feed.poll();
        assert_eq!(frames.len(), 4);
        for (entry, frame) in history.iter().zip(frames.iter().skip(1)) {
            let FeedEvent::Message { message } = &frame.event else {
                panic!("expected entry");
            };
            assert_eq!(entry.index(), frame.cursor);
            assert_eq!(
                serde_json::to_value(entry).unwrap(),
                serde_json::to_value(message).unwrap()
            );
        }
        let reopened = engine.subscribe(FeedScope::Channel("room".into()));
        let frames = reopened.poll();
        let FeedEvent::Snapshot { messages } = &frames[0].event else {
            panic!("expected snapshot");
        };
        assert_eq!(
            serde_json::to_value(messages).unwrap(),
            serde_json::to_value(&history).unwrap()
        );
        let json = serde_json::to_value(&history).unwrap();
        assert_eq!(json[0]["kind"], "chat");
        assert_eq!(json[1]["kind"], "system");
        assert!(!json[1].as_object().unwrap().contains_key("chatter_user_id"));
    }

    #[test]
    fn ordinary_sequence_survives_notices_only_trimming_and_login_changes() {
        let engine = ChatEngine::new(ChatSettings {
            message_limit: 1,
            ..Default::default()
        });
        engine.insert(chat("first"));
        engine.insert_system(room(), input("one")).unwrap();
        engine.insert_system(room(), input("two")).unwrap();
        assert_eq!(engine.history("room").len(), 1);
        let bytes = engine.metrics().estimated_history_bytes;
        assert!(bytes > 0);
        let mut renamed = chat("second");
        renamed.broadcaster_user_login = "renamed".into();
        engine.insert(renamed);
        assert_eq!(
            engine.history("room")[0]
                .as_chat()
                .unwrap()
                .channel_sequence,
            1
        );
        assert_eq!(
            engine.history("renamed")[0]
                .as_chat()
                .unwrap()
                .channel_sequence,
            1
        );
        assert_eq!(engine.metrics().retained_messages, 1);
        engine.shutdown();
        assert_eq!(engine.metrics().estimated_history_bytes, 0);
        assert!(engine.insert_system(room(), input("ended")).is_err());
    }

    #[test]
    fn translations_ignore_system_entries_even_with_the_same_producer_id() {
        let engine = ChatEngine::new(ChatSettings::default());
        engine.insert_system(room(), input("shared")).unwrap();
        engine.insert(chat("shared"));
        engine.insert_system(room(), input("tail")).unwrap();
        let candidates = engine
            .state
            .lock()
            .unwrap()
            .repository
            .translation_candidates("room", 0, 10, true, 1);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].message_id, "shared");
        let before = serde_json::to_value(&engine.history("room")[0]).unwrap();
        engine.translate(
            "room",
            ChannelMessageTranslationUpdate {
                message_id: "shared".into(),
                translation: ChannelMessageTranslation {
                    source_language: "en".into(),
                    target_language: "ru".into(),
                    translated_text: "Привет".into(),
                },
            },
        );
        let history = engine.history("room");
        assert_eq!(serde_json::to_value(&history[0]).unwrap(), before);
        assert!(history[1].as_chat().unwrap().translation.is_some());
    }

    #[test]
    fn slow_subscribers_reset_to_mixed_retained_history() {
        let engine = ChatEngine::new(ChatSettings {
            message_limit: 3,
            ..Default::default()
        });
        let feed = engine.subscribe(FeedScope::Channel("room".into()));
        for id in 0..=FEED_CAPACITY {
            engine
                .insert_system(room(), input(&id.to_string()))
                .unwrap();
        }
        engine.insert(chat("last"));
        let frames = feed.poll();
        let FeedEvent::Reset { messages } = &frames[0].event else {
            panic!("expected reset");
        };
        assert_eq!(messages.len(), 3);
        assert!(matches!(messages[0], ChatTimelineEntry::System(_)));
        assert_eq!(messages[2].as_chat().unwrap().channel_sequence, 0);
        assert_eq!(engine.metrics().retained_messages, 3);
    }

    #[test]
    fn malformed_notices_are_rejected_before_insertion() {
        let engine = ChatEngine::new(ChatSettings::default());
        let mut notice = input("invalid");
        notice.ts = "bad date".into();
        assert!(engine.insert_system(room(), notice).is_err());
        let mut notice = input("invalid");
        notice.segments.clear();
        assert!(engine.insert_system(room(), notice).is_err());
        for url in ["javascript:alert(1)", "file:///tmp/test", "broken"] {
            let mut notice = input("invalid");
            notice.segments = vec![SystemMessageSegment::Link {
                text: "Details".into(),
                url: url.into(),
            }];
            assert!(engine.insert_system(room(), notice).is_err());
        }
        assert!(engine.history("room").is_empty());
        let settings: ChatSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.center_system_messages);
    }
}

use super::timeline::{ChatTimelineEntry, SystemMessageSegment};
use crate::message::Fragment;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

const USER_LIMIT: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
pub struct ObservedUser {
    pub user_id: String,
    pub login: String,
    pub display_name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserDirectoryUpdate {
    Snapshot {
        users: Vec<ObservedUser>,
    },
    Delta {
        upserts: Vec<ObservedUser>,
        removed: Vec<String>,
    },
}

struct Entry {
    user: ObservedUser,
    observed: u64,
}

pub struct UserDirectory {
    users: HashMap<String, Entry>,
    logins: HashMap<String, String>,
    recency: BTreeMap<u64, String>,
    clock: u64,
    limit: usize,
}

impl Default for UserDirectory {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
            logins: HashMap::new(),
            recency: BTreeMap::new(),
            clock: 0,
            limit: USER_LIMIT,
        }
    }
}

impl UserDirectory {
    pub fn snapshot(&self) -> UserDirectoryUpdate {
        UserDirectoryUpdate::Snapshot {
            users: self
                .users
                .values()
                .map(|entry| entry.user.clone())
                .collect(),
        }
    }

    fn by_login(&self, login: &str) -> Option<&ObservedUser> {
        self.logins
            .get(&login.to_ascii_lowercase())
            .and_then(|id| self.users.get(id))
            .map(|entry| &entry.user)
    }

    pub fn enrich(&self, entry: &mut ChatTimelineEntry) {
        let ChatTimelineEntry::Chat(message) = entry else {
            return;
        };
        for fragment in &mut message.fragments {
            let Fragment::Mention(mention) = fragment else {
                continue;
            };
            if mention.user_id.is_none() {
                if !message.chatter_user_login.is_empty()
                    && mention
                        .login
                        .eq_ignore_ascii_case(&message.chatter_user_login)
                {
                    mention.user_id = Some(message.chatter_user_id.clone());
                    mention.display_name = Some(message.chatter_user_name.clone());
                } else if let Some(user) = self.by_login(&mention.login) {
                    mention.user_id = Some(user.user_id.clone());
                    mention.display_name = Some(user.display_name.clone());
                }
            }
        }
    }

    pub fn observe(&mut self, entry: &ChatTimelineEntry) -> Option<UserDirectoryUpdate> {
        let mut changed = HashMap::new();
        let mut removed = Vec::new();
        let mut record =
            |id: &str, login: &str, name: &str, color: Option<&str>, authoritative: bool| {
                if let Some(user) = self.record(id, login, name, color, authoritative, &mut removed)
                {
                    changed.insert(user.user_id.clone(), user);
                }
            };
        match entry {
            ChatTimelineEntry::Chat(message) => {
                record(
                    &message.broadcaster_user_id,
                    &message.broadcaster_user_login,
                    &message.broadcaster_user_name,
                    None,
                    true,
                );
                record(
                    &message.chatter_user_id,
                    &message.chatter_user_login,
                    &message.chatter_user_name,
                    Some(&message.color),
                    true,
                );
                for fragment in &message.fragments {
                    if let Fragment::Mention(mention) = fragment {
                        if let Some(id) = &mention.user_id {
                            record(
                                id,
                                &mention.login,
                                mention.display_name.as_deref().unwrap_or(""),
                                None,
                                true,
                            );
                        }
                    }
                }
            }
            ChatTimelineEntry::System(message) => {
                record(
                    &message.broadcaster_user_id,
                    &message.broadcaster_user_login,
                    &message.broadcaster_user_name,
                    None,
                    true,
                );
                for segment in &message.segments {
                    if let SystemMessageSegment::User {
                        user_id,
                        login,
                        display_name,
                    } = segment
                    {
                        record(user_id, login, display_name, None, false);
                    }
                }
            }
        }
        changed.retain(|id, _| self.users.contains_key(id));
        removed.retain(|id| !self.users.contains_key(id));
        if changed.is_empty() && removed.is_empty() {
            return None;
        }
        Some(UserDirectoryUpdate::Delta {
            upserts: changed.into_values().collect(),
            removed,
        })
    }

    fn record(
        &mut self,
        id: &str,
        login: &str,
        name: &str,
        color: Option<&str>,
        authoritative: bool,
        removed: &mut Vec<String>,
    ) -> Option<ObservedUser> {
        if id.is_empty() || login.is_empty() {
            return None;
        }
        let login = login.to_ascii_lowercase();
        if let Some(owner) = self
            .logins
            .get(&login)
            .filter(|owner| owner.as_str() != id)
            .cloned()
        {
            if let Some(entry) = self.users.remove(&owner) {
                self.recency.remove(&entry.observed);
                removed.push(owner);
            }
            self.logins.remove(&login);
        }
        let previous = self.users.remove(id);
        let old_user = previous.as_ref().map(|entry| entry.user.clone());
        if let Some(entry) = &previous {
            self.recency.remove(&entry.observed);
            if self
                .logins
                .get(&entry.user.login)
                .is_some_and(|owner| owner == id)
            {
                self.logins.remove(&entry.user.login);
            }
        }
        let mut user = old_user.clone().unwrap_or_else(|| ObservedUser {
            user_id: id.to_owned(),
            login: login.clone(),
            display_name: login.clone(),
            color: None,
        });
        user.login = login.clone();
        if !name.trim().is_empty()
            && (authoritative
                || !name.eq_ignore_ascii_case(&login)
                || user.display_name.eq_ignore_ascii_case(&login)
                || previous.is_none())
        {
            user.display_name = name.to_owned();
        }
        if let Some(color) = color {
            user.color = Some(color.to_owned());
        }
        self.clock += 1;
        self.recency.insert(self.clock, id.to_owned());
        self.logins.insert(login, id.to_owned());
        self.users.insert(
            id.to_owned(),
            Entry {
                user: user.clone(),
                observed: self.clock,
            },
        );
        while self.users.len() > self.limit {
            let Some((_, evicted)) = self.recency.pop_first() else {
                break;
            };
            if let Some(entry) = self.users.remove(&evicted) {
                if self.logins.get(&entry.user.login) == Some(&evicted) {
                    self.logins.remove(&entry.user.login);
                }
                removed.push(evicted);
            }
        }
        (old_user.as_ref() != Some(&user)).then_some(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(channel: &str, id: &str, author: &str, name: &str) -> crate::types::ChannelMessage {
        serde_json::from_value(serde_json::json!({
            "ts": "2026-11-21T00:00:00Z", "broadcaster_user_id": channel,
            "broadcaster_user_login": channel, "broadcaster_user_name": channel,
            "chatter_user_id": author, "chatter_user_login": author, "chatter_user_name": name,
            "message_id": id, "text": "Hi", "fragments": [], "message_type": "text",
            "badges": [], "color": "#123456", "translation": null, "index": 0, "channel_sequence": 0
        }))
        .unwrap()
    }

    #[test]
    fn background_metadata_shares_the_message_cursor_without_adding_timeline_entries() {
        use crate::chat::{ChatEngine, FeedEvent, FeedScope};
        let engine = ChatEngine::new(Default::default());
        let foreground = engine.subscribe(FeedScope::Channel("rooma".into()));
        let background = engine.subscribe(FeedScope::Channel("roomb".into()));
        foreground.poll();
        background.poll();
        let incoming = message("roomb", "one", "maya", "마야");
        assert!(engine.insert(incoming.clone()));
        let foreground_frames = foreground.poll();
        let background_frames = background.poll();
        assert_eq!(foreground_frames.len(), 1);
        assert_eq!(background_frames.len(), 1);
        assert!(matches!(foreground_frames[0].event, FeedEvent::Users));
        assert!(matches!(
            background_frames[0].event,
            FeedEvent::Message { .. }
        ));
        assert_eq!(foreground_frames[0].cursor, background_frames[0].cursor);
        assert_eq!(
            foreground_frames[0].cursor,
            engine.history("roomb")[0].index()
        );
        assert!(engine.history("rooma").is_empty());
        let Some(UserDirectoryUpdate::Delta { upserts, .. }) = &foreground_frames[0].users else {
            panic!("missing delta")
        };
        assert!(upserts
            .iter()
            .any(|u| u.user_id == "maya" && u.display_name == "마야"));
        let mut duplicate = incoming;
        duplicate.color = "#654321".into();
        assert!(!engine.insert(duplicate));
        assert!(foreground.poll().is_empty());
        assert_eq!(
            engine
                .state
                .lock()
                .unwrap()
                .users
                .by_login("maya")
                .unwrap()
                .color
                .as_deref(),
            Some("#123456")
        );

        let mut mention = message("rooma", "two", "viewer", "Viewer");
        mention.fragments = vec![
            Fragment::Text(crate::message::TextFragment {
                index: 0,
                text: "Hello ".into(),
            }),
            Fragment::Mention(crate::message::MentionFragment {
                index: 1,
                text: "@MAYA".into(),
                login: "maya".into(),
                user_id: None,
                display_name: None,
            }),
        ];
        assert!(engine.insert(mention));
        let history = engine.history("rooma");
        let Fragment::Mention(mention) = &history[0].as_chat().unwrap().fragments[1] else {
            panic!("missing mention")
        };
        assert_eq!(mention.user_id.as_deref(), Some("maya"));
        assert_eq!(mention.display_name.as_deref(), Some("마야"));
        engine.shutdown();
        assert!(engine.state.lock().unwrap().users.users.is_empty());
    }

    #[test]
    fn resets_restore_the_complete_user_directory_after_background_metadata_lag() {
        use crate::chat::{ChatEngine, FeedEvent, FeedScope, FEED_CAPACITY};
        let engine = ChatEngine::new(Default::default());
        let feed = engine.subscribe(FeedScope::Channel("rooma".into()));
        feed.poll();
        for index in 0..=FEED_CAPACITY {
            engine.insert(message(
                "roomb",
                &index.to_string(),
                &format!("viewer{index}"),
                "Viewer",
            ));
        }
        let frames = feed.poll();
        assert_eq!(frames.len(), 1);
        assert!(matches!(&frames[0].event, FeedEvent::Reset { messages } if messages.is_empty()));
        let Some(UserDirectoryUpdate::Snapshot { users }) = &frames[0].users else {
            panic!("missing user snapshot")
        };
        assert_eq!(users.len(), FEED_CAPACITY + 2);
        assert!(users
            .iter()
            .any(|u| u.login == format!("viewer{FEED_CAPACITY}")));
    }

    #[test]
    fn merges_partial_metadata_and_explicit_color_removal() {
        let mut cache = UserDirectory::default();
        let mut removed = vec![];
        cache.record("1", "maya", "마야", Some("#123456"), true, &mut removed);
        cache.record("1", "maya", "maya", None, false, &mut removed);
        assert_eq!(cache.by_login("MAYA").unwrap().display_name, "마야");
        assert_eq!(
            cache.by_login("maya").unwrap().color.as_deref(),
            Some("#123456")
        );
        cache.record("1", "maya", "Maya", Some(""), true, &mut removed);
        assert_eq!(cache.by_login("maya").unwrap().display_name, "Maya");
        assert_eq!(cache.by_login("maya").unwrap().color.as_deref(), Some(""));
    }

    #[test]
    fn evicts_least_recently_observed_and_updates_login_ownership() {
        let mut cache = UserDirectory {
            limit: 2,
            ..Default::default()
        };
        let mut removed = vec![];
        cache.record("1", "one", "One", None, true, &mut removed);
        cache.record("2", "two", "Two", None, true, &mut removed);
        cache.record("1", "renamed", "Renamed", None, true, &mut removed);
        assert!(cache.by_login("one").is_none());
        cache.record("3", "three", "Three", None, true, &mut removed);
        assert_eq!(removed, ["2"]);
        assert!(cache.by_login("two").is_none());
        assert_eq!(cache.by_login("RENAMED").unwrap().user_id, "1");
        cache.record("4", "renamed", "Other", None, true, &mut removed);
        assert_eq!(cache.by_login("renamed").unwrap().user_id, "4");
    }
}

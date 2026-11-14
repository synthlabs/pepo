use crate::eventsub::EventNotification;
use serde::{Deserialize, Serialize};
use twitch_api::eventsub::{Event, Message, Payload};

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct ModerationUser {
    pub user_id: String,
    pub login: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModerationAction {
    MessageDeleted {
        target: ModerationUser,
        message_id: String,
    },
    // The reader stream does not identify whether this was a ban or a timeout.
    UserMessagesCleared {
        target: ModerationUser,
    },
    ChatCleared,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct ModerationEvent {
    pub notification_id: String,
    pub ts: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub action: ModerationAction,
}

impl ModerationEvent {
    pub fn from_notification(notification: &EventNotification) -> Option<Self> {
        let (id, login, name, action) = match &notification.event {
            Event::ChannelChatMessageDeleteV1(Payload {
                message: Message::Notification(event),
                ..
            }) => (
                event.broadcaster_user_id.to_string(),
                event.broadcaster_user_login.to_string(),
                event.broadcaster_user_name.to_string(),
                ModerationAction::MessageDeleted {
                    target: ModerationUser {
                        user_id: event.target_user_id.to_string(),
                        login: event.target_user_login.to_string(),
                        display_name: event.target_user_name.to_string(),
                    },
                    message_id: event.message_id.to_string(),
                },
            ),
            Event::ChannelChatClearUserMessagesV1(Payload {
                message: Message::Notification(event),
                ..
            }) => (
                event.broadcaster_user_id.to_string(),
                event.broadcaster_user_login.to_string(),
                event.broadcaster_user_name.to_string(),
                ModerationAction::UserMessagesCleared {
                    target: ModerationUser {
                        user_id: event.target_user_id.to_string(),
                        login: event.target_user_login.to_string(),
                        display_name: event.target_user_name.to_string(),
                    },
                },
            ),
            Event::ChannelChatClearV1(Payload {
                message: Message::Notification(event),
                ..
            }) => (
                event.broadcaster_user_id.to_string(),
                event.broadcaster_user_login.to_string(),
                event.broadcaster_user_name.to_string(),
                ModerationAction::ChatCleared,
            ),
            _ => return None,
        };
        Some(Self {
            notification_id: notification.id.clone(),
            ts: notification.ts.to_string(),
            broadcaster_user_id: id,
            broadcaster_user_login: login,
            broadcaster_user_name: name,
            action,
        })
    }
}

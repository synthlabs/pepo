use super::*;
use compatibility::{decode, DecodedFrame};
use diagnostics_tests::chat_frame;
use serde_json::{json, Value};
use std::fmt::{self, Write};
use std::sync::mpsc::sync_channel;
use tracing::field::Visit;
use tracing_subscriber::{layer::Context, prelude::*, Layer};
use twitch_api::eventsub::{channel::ChannelChatMessageV1Payload, Message, Payload};

fn deliver(frame: &Value) -> EventNotification {
    let (sender, receiver) = sync_channel(1);
    assert!(
        process_frame(&frame.to_string(), &sender, EventSubSettings::default())
            .unwrap()
            .is_none()
    );
    match receiver.try_recv().expect("chat notification delivered") {
        EventSubMessage::Notification(notification) => notification,
        other => panic!("unexpected event: {other:?}"),
    }
}

fn chat(notification: &EventNotification) -> &ChannelChatMessageV1Payload {
    match &notification.event {
        Event::ChannelChatMessageV1(Payload {
            message: Message::Notification(chat),
            ..
        }) => chat,
        other => panic!("unexpected notification: {other:?}"),
    }
}

fn ignored(frame: &Value) {
    assert!(matches!(
        decode(&frame.to_string()),
        Ok(DecodedFrame::Ignored)
    ));
    let (sender, receiver) = sync_channel(1);
    process_frame(&frame.to_string(), &sender, EventSubSettings::default()).unwrap();
    assert!(receiver.try_recv().is_err());
}

fn failure(frame: &Value) -> Value {
    let text = frame.to_string();
    let diagnostic = decode(&text).err().expect("invalid frame rejected");
    serde_json::to_value(diagnostic).unwrap()
}

fn emote_fragment() -> Value {
    json!({"type": "emote", "text": "Kappa", "emote": {
        "id": "25", "emote_set_id": "0", "owner_id": "123", "format": ["static"]
    }})
}

#[test]
fn reported_gif_falls_back_without_losing_text_emotes_or_identity() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message"] = json!({
        "text": "before Kappa GIF after",
        "fragments": [
            {"type": "text", "text": "before "},
            emote_fragment(),
            {"type": "gif", "text": " GIF", "gif": {
                "id": "private-gif-id", "url": "https://example.invalid/private-gif"
            }, "emote": null, "cheermote": null, "mention": null},
            {"type": "text", "text": " after"}
        ]
    });
    let notification = deliver(&frame);
    assert!(notification.message_type_override.is_none());
    let payload = chat(&notification);
    assert_eq!(payload.message.text, "before Kappa GIF after");
    assert_eq!(payload.message_id.as_str(), "private-chat-id");
    assert_eq!(payload.broadcaster_user_id.as_str(), "123");
    assert_eq!(payload.chatter_user_id.as_str(), "456");
    assert!(matches!(
        payload.message.fragments[1],
        eventsub::channel::chat::Fragment::Emote { .. }
    ));
    assert!(
        matches!(&payload.message.fragments[2], eventsub::channel::chat::Fragment::Text { text } if text == " GIF")
    );
}

#[test]
fn future_fragments_use_available_text_and_omit_fragments_without_text() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message"] = json!({"text": "", "fragments": [
        {"type": "text", "text": "before "},
        {"type": "future_content", "text": "readable"},
        {"type": "future_without_text"},
        {"type": "gif", "text": null},
        {"type": "future_empty_text", "text": ""},
        {"type": "text", "text": " after"}
    ]});
    let notification = deliver(&frame);
    assert_eq!(chat(&notification).message.text, "before readable after");
    assert_eq!(chat(&notification).message.fragments.len(), 3);
}

#[test]
fn future_message_types_preserve_chat_with_an_unknown_type_override() {
    for kind in ["future_message", "unknown", "future_highlight"] {
        let mut frame = chat_frame();
        frame["payload"]["event"]["message_type"] = json!(kind);
        let notification = deliver(&frame);
        assert!(matches!(
            notification.message_type_override,
            Some(ChannelMessageType::Unknown)
        ));
        assert_eq!(chat(&notification).message.text, "private chat text");
    }
}

#[test]
fn known_message_types_keep_their_original_meaning() {
    for kind in [
        "text",
        "channel_points_highlighted",
        "channel_points_sub_only",
        "user_intro",
        "power_ups_gigantified_emote",
        "power_ups_message_effect",
    ] {
        let mut frame = chat_frame();
        frame["payload"]["event"]["message_type"] = json!(kind);
        frame["payload"]["event"]["message"]["fragments"][0]["type"] = json!("future_content");
        let notification = deliver(&frame);
        assert!(notification.message_type_override.is_none());
        assert_eq!(
            serde_json::to_value(&chat(&notification).message_type).unwrap(),
            kind
        );
    }
}

#[test]
fn unsupported_content_without_readable_text_is_omitted() {
    for text in ["", " \t"] {
        let mut frame = chat_frame();
        frame["payload"]["event"]["message"] = json!({"text": text, "fragments": [
            {"type": "gif", "gif": {"id": "private-gif"}},
            {"type": "future_content", "text": ""}
        ]});
        ignored(&frame);
    }
    let mut known = chat_frame();
    known["payload"]["event"]["message"] = json!({"text": "", "fragments": []});
    assert_eq!(chat(&deliver(&known)).message.text, "");
}

fn unhandled_frame(event_type: &str, version: &str, notice: &str) -> Value {
    let mut frame = chat_frame();
    frame["metadata"]["subscription_type"] = json!(event_type);
    frame["metadata"]["subscription_version"] = json!(version);
    frame["payload"]["subscription"]["type"] = json!(event_type);
    frame["payload"]["subscription"]["version"] = json!(version);
    frame["payload"]["event"] = json!({"notice_type": notice,
        "watch_streak": {"channel_points_awarded": 350, "streak_count": 3},
        "message": {"text": "private notice", "fragments": []}
    });
    frame
}

#[test]
fn reported_watch_streak_and_other_unhandled_categories_are_ignored() {
    for (event_type, version, notice) in [
        ("channel.chat.notification", "1", "watch_streak"),
        ("channel.chat.notification", "1", "sub"),
        ("channel.chat.notification", "1", "future_notice"),
        ("channel.future.event", "1", "future_notice"),
        ("channel.chat.message", "999", ""),
        ("user.update", "1", ""),
    ] {
        ignored(&unhandled_frame(event_type, version, notice));
    }
}

#[test]
fn invalid_notification_envelopes_are_errors_even_for_unhandled_content() {
    let valid = unhandled_frame("channel.future.event", "1", "future_notice");
    for pointer in [
        "/metadata/message_id",
        "/metadata/message_timestamp",
        "/metadata/subscription_type",
        "/metadata/subscription_version",
        "/payload/subscription/type",
        "/payload/subscription/version",
        "/payload/event",
    ] {
        let mut frame = valid.clone();
        *frame.pointer_mut(pointer).unwrap() = json!(123);
        assert_eq!(failure(&frame)["error_kind"], "schema", "{pointer}");
    }
    let mut missing = valid.clone();
    missing["metadata"]
        .as_object_mut()
        .unwrap()
        .remove("subscription_type");
    assert_eq!(
        failure(&missing)["reason"],
        "missing field `subscription_type`"
    );
    let mut mismatch = valid;
    mismatch["payload"]["subscription"]["version"] = json!("2");
    assert_eq!(failure(&mismatch)["field_path"], "payload.subscription");
}

#[test]
fn malformed_known_fields_are_not_hidden_by_compatibility_fallback() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message"]["fragments"] = json!([
        {"type": "gif", "text": null}, emote_fragment()
    ]);
    frame["payload"]["event"]["message"]["fragments"][1]["emote"]
        .as_object_mut()
        .unwrap()
        .remove("owner_id");
    let diagnostic = failure(&frame);
    assert_eq!(diagnostic["reason"], "missing field `owner_id`");
    assert!(diagnostic["field_path"]
        .as_str()
        .unwrap()
        .contains("fragments[1]"));
    assert!(diagnostic["line"].is_null());
    assert!(diagnostic["column"].is_null());
    let sample: Value = serde_json::from_str(diagnostic["sample"].as_str().unwrap()).unwrap();
    assert_eq!(
        sample["payload"]["event"]["message"]["fragments"][0]["type"],
        "gif"
    );
    for pointer in [
        "/payload/event/message",
        "/payload/event/message_type",
        "/payload/event/badges",
        "/payload/event/message/text",
    ] {
        let mut invalid = chat_frame();
        invalid["payload"]["event"]["message_type"] = json!("future_message");
        *invalid.pointer_mut(pointer).unwrap() = json!(123);
        assert!(
            matches!(
                failure(&invalid)["error_kind"].as_str(),
                Some("schema" | "syntax")
            ),
            "{pointer}"
        );
    }
    let mut bad_text = chat_frame();
    bad_text["payload"]["event"]["message"]["fragments"][0] = json!({"type": "gif", "text": 123});
    assert_eq!(failure(&bad_text)["error_kind"], "schema");
}

#[test]
fn invalid_json_and_transport_types_remain_errors() {
    for frame in [
        "{",
        r#"{"metadata":{"message_type":"notification"}} trailing"#,
        "null",
    ] {
        assert!(decode(frame).is_err());
    }
    let mut frame = chat_frame();
    frame["metadata"]["message_type"] = json!("future_transport_message");
    assert!(matches!(
        failure(&frame)["error_kind"].as_str(),
        Some("schema" | "syntax")
    ));
}

#[test]
fn revocation_still_reaches_control_handling() {
    let mut frame = chat_frame();
    frame["metadata"]["message_type"] = json!("revocation");
    frame["payload"].as_object_mut().unwrap().remove("event");
    frame["payload"]["subscription"]["status"] = json!("authorization_revoked");
    let text = frame.to_string();
    assert!(matches!(
        decode(&text),
        Ok(DecodedFrame::Parsed(
            EventsubWebsocketData::Revocation { .. }
        ))
    ));
    frame["payload"]["subscription"]["status"] = json!(123);
    assert!(matches!(
        failure(&frame)["error_kind"].as_str(),
        Some("schema" | "syntax")
    ));
    frame["metadata"]["subscription_type"] = json!("channel.future.event");
    assert!(decode(&frame.to_string()).is_err());
}

#[test]
fn compatibility_does_not_hide_duplicate_known_fields() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message"]["fragments"][0]["type"] = json!("gif");
    let text = frame.to_string().replacen(
        "\"source_badges\":null",
        "\"source_badges\":null,\"source_badges\":null",
        1,
    );
    let diagnostic = decode(&text)
        .err()
        .expect("duplicate field must remain an error");
    let diagnostic = serde_json::to_value(diagnostic).unwrap();
    assert_eq!(diagnostic["reason"], "duplicate field `source_badges`");
}

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<(tracing::Level, String)>>>);

#[derive(Default)]
struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        write!(self.0, "{field}={value:?} ").unwrap();
    }
}

impl<S: tracing::Subscriber> Layer<S> for Recorder {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        self.0
            .lock()
            .unwrap()
            .push((*event.metadata().level(), fields.0));
    }
}

#[test]
fn compatibility_emits_safe_debug_records_and_chat_continues_after_skipped_events() {
    if logging::run_log_test_in_child("eventsub::compatibility_tests::compatibility_emits_safe_debug_records_and_chat_continues_after_skipped_events") {
        return;
    }
    let recorder = Recorder::default();
    let subscriber = tracing_subscriber::registry()
        .with(recorder.clone())
        .with(tracing_subscriber::filter::LevelFilter::DEBUG);
    tracing::subscriber::with_default(subscriber, || {
        let (sender, receiver) = sync_channel(4);
        for frame in [
            unhandled_frame("channel.chat.notification", "1", "watch_streak"),
            unhandled_frame("private secret @ event", "1", "private secret @ notice"),
        ] {
            process_frame(&frame.to_string(), &sender, EventSubSettings::default()).unwrap();
        }
        let mut frame = chat_frame();
        frame["payload"]["event"]["message"]["fragments"][0]["type"] =
            json!("private secret @ fragment");
        frame["payload"]["event"]["message_type"] = json!("private secret @ message");
        process_frame(&frame.to_string(), &sender, EventSubSettings::default()).unwrap();
        process_frame(
            &chat_frame().to_string(),
            &sender,
            EventSubSettings::default(),
        )
        .unwrap();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            EventSubMessage::Notification(_)
        ));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            EventSubMessage::Notification(_)
        ));
        assert!(receiver.try_recv().is_err());
        let events = recorder.0.lock().unwrap();
        assert_eq!(events.len(), 3, "{events:?}");
        for (level, message) in events.iter() {
            assert_eq!(*level, tracing::Level::DEBUG, "{message}");
            for secret in [
                "private secret",
                "private chat text",
                "private notice",
                "private-envelope-id",
                "private-session-id",
                "private_chatter",
            ] {
                assert!(!message.contains(secret), "{message}");
            }
        }
        assert!(events[0].1.contains("watch_streak"));
        assert!(events[1].1.contains("<string>"));
    });
}

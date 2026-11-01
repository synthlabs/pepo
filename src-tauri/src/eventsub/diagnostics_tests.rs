use super::*;
use diagnostics::ParseFailure;
use serde_json::{json, Value};
use std::fmt::{self, Write};
use std::sync::mpsc::sync_channel;
use tracing::field::Visit;
use tracing_subscriber::{layer::Context, prelude::*, Layer};

fn chat_frame() -> Value {
    json!({
        "metadata": {
            "message_id": "private-envelope-id",
            "message_type": "notification",
            "message_timestamp": "2026-11-01T00:00:00Z",
            "subscription_type": "channel.chat.message",
            "subscription_version": "1"
        },
        "payload": {
            "subscription": {
                "id": "private-subscription-id", "status": "enabled",
                "type": "channel.chat.message", "version": "1", "cost": 0,
                "condition": {"broadcaster_user_id": "123", "user_id": "456"},
                "created_at": "2026-11-01T00:00:00Z",
                "transport": {"method": "websocket", "session_id": "private-session-id"}
            },
            "event": {
                "broadcaster_user_id": "123", "broadcaster_user_login": "private_streamer",
                "broadcaster_user_name": "Private Streamer", "chatter_user_id": "456",
                "chatter_user_login": "private_chatter", "chatter_user_name": "Private Chatter",
                "message_id": "private-chat-id", "message_type": "text",
                "message": {"text": "private chat text", "fragments": [
                    {"type": "text", "text": "private chat text"}
                ]},
                "color": "#123456", "badges": [], "source_badges": null
            }
        }
    })
}

fn failure(frame: &Value) -> ParseFailure {
    let frame = frame.to_string();
    ParseFailure::new(&frame, &Event::parse_websocket(&frame).unwrap_err())
}

#[test]
fn identifies_schema_path_and_preserves_unknown_discriminators() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message_type"] = json!("future_chat_type");
    let diagnostic = serde_json::to_value(failure(&frame)).unwrap();
    assert_eq!(diagnostic["event_type"], "channel.chat.message");
    assert_eq!(diagnostic["event_version"], "1");
    assert_eq!(diagnostic["error_kind"], "schema");
    assert_eq!(diagnostic["field_path"], "event.message_type");
    assert!(diagnostic["reason"]
        .as_str()
        .unwrap()
        .contains("future_chat_type"));
    assert!(diagnostic["reason"].as_str().unwrap().contains("expected"));
    assert!(diagnostic["line"].as_u64().unwrap() > 0);
}

#[test]
fn identifies_missing_fields_and_wrong_value_types() {
    let mut frame = chat_frame();
    frame["payload"]["event"]
        .as_object_mut()
        .unwrap()
        .remove("color");
    let diagnostic = serde_json::to_value(failure(&frame)).unwrap();
    assert_eq!(diagnostic["reason"], "missing field `color`");
    frame["payload"]["event"]["color"] = json!("#123456");
    frame["payload"]["event"]["badges"] = json!("private-secret-badges");
    let diagnostic = serde_json::to_value(failure(&frame)).unwrap();
    assert_eq!(diagnostic["field_path"], "event.badges");
    assert!(diagnostic["reason"]
        .as_str()
        .unwrap()
        .contains("expected a sequence"));
    assert!(!diagnostic.to_string().contains("private-secret-badges"));
}

#[test]
fn distinguishes_unsupported_versions_unknown_types_and_invalid_json() {
    let mut frame = chat_frame();
    frame["metadata"]["subscription_version"] = json!("999");
    let diagnostic = serde_json::to_value(failure(&frame)).unwrap();
    assert_eq!(diagnostic["error_kind"], "unimplemented_event");
    assert_eq!(diagnostic["event_version"], "999");
    frame["metadata"]["subscription_type"] = json!("channel.future.event");
    let diagnostic = serde_json::to_value(failure(&frame)).unwrap();
    assert_eq!(diagnostic["event_type"], "channel.future.event");
    let malformed = r#"{"metadata":{"access_token":"private-token"},"payload":"#;
    let diagnostic = ParseFailure::new(malformed, &Event::parse_websocket(malformed).unwrap_err());
    let value = serde_json::to_value(&diagnostic).unwrap();
    assert_eq!(value["error_kind"], "eof");
    assert!(value["sample"].is_null());
    assert_eq!(value["frame_bytes"], malformed.len());
    assert!(!diagnostic.message().contains("private-token"));
    let no_metadata = json!({"payload": {"access_token": "private-token"}});
    assert!(serde_json::to_value(failure(&no_metadata)).unwrap()["message_type"].is_null());
}

#[test]
fn removes_sensitive_values_from_samples_reasons_and_grouping_keys() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["access_token"] = json!("private-access-token");
    frame["payload"]["event"]["refresh_token"] = json!("private-refresh-token");
    frame["payload"]["event"]["badges"] = json!("private-rejected-chat-text");
    let diagnostic = failure(&frame);
    let output = format!("{}{}", diagnostic.message(), diagnostic.key());
    for secret in [
        "private-access-token",
        "private-refresh-token",
        "private-rejected-chat-text",
        "private-envelope-id",
        "private-subscription-id",
        "private-session-id",
        "private_streamer",
        "Private Streamer",
        "private_chatter",
        "Private Chatter",
        "private-chat-id",
        "private chat text",
        "#123456",
    ] {
        assert!(!output.contains(secret), "leaked {secret}");
    }
    let value = serde_json::to_value(diagnostic).unwrap();
    let sample: Value = serde_json::from_str(value["sample"].as_str().unwrap()).unwrap();
    assert_eq!(sample["payload"]["subscription"]["cost"], "<number>");
    assert_eq!(
        sample["payload"]["event"]["message"]["fragments"][0]["type"],
        "text"
    );
    assert_eq!(sample["payload"]["event"]["source_badges"], Value::Null);
}

#[test]
fn caps_unicode_samples_and_omits_unrecognized_free_text_errors() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message_type"] = json!("future_chat_type");
    frame["payload"]["event"]["extra"] = json!({"界": vec!["private-text"; 500]});
    let value = serde_json::to_value(failure(&frame)).unwrap();
    let sample = value["sample"].as_str().unwrap();
    assert_eq!(value["sample_truncated"], true);
    assert!(sample.len() <= 4096);
    assert!(sample.ends_with("...[truncated]"));
    assert!(!sample.contains("private-text"));
    let custom = <serde_json::Error as serde::de::Error>::custom("secret private-token");
    let error = eventsub::PayloadParseError::DeserializeError(twitch_api::DeserError::PathError {
        path: "event.color".into(),
        error: custom,
    });
    assert!(!ParseFailure::new(&frame.to_string(), &error)
        .message()
        .contains("private-token"));
}

#[test]
fn groups_without_samples_identities_timestamps_or_json_positions() {
    let mut frame = chat_frame();
    frame["payload"]["event"]["message_type"] = json!("future_chat_type");
    let first = failure(&frame);
    frame["metadata"]["message_id"] = json!("different-envelope");
    frame["metadata"]["message_timestamp"] = json!("2026-11-01T01:00:00Z");
    frame["payload"]["event"]["message"]["text"] = json!("different private text");
    let pretty = serde_json::to_string_pretty(&frame).unwrap();
    let second = ParseFailure::new(&pretty, &Event::parse_websocket(&pretty).unwrap_err());
    assert_eq!(first.key(), second.key());
    frame["payload"]["event"]["message_type"] = json!("another_future_type");
    assert_ne!(first.key(), failure(&frame).key());
    frame["payload"]["event"]["message_type"] = json!("text");
    frame["payload"]["event"]["badges"] = json!(true);
    assert_ne!(first.key(), failure(&frame).key());
    frame["metadata"]["subscription_version"] = json!("999");
    assert_ne!(first.key(), failure(&frame).key());
}

#[test]
fn skipped_frames_do_not_prevent_later_chat_delivery_or_control_handling() {
    let (sender, receiver) = sync_channel(4);
    let settings = EventSubSettings::default();
    let mut invalid = chat_frame();
    invalid["payload"]["event"]["message_type"] = json!("future_chat_type");
    assert!(process_frame(&invalid.to_string(), &sender, settings)
        .unwrap()
        .is_none());
    assert!(receiver.try_recv().is_err());
    let valid = chat_frame().to_string();
    assert!(process_frame(&valid, &sender, settings).unwrap().is_none());
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EventSubMessage::Notification(_)
    ));
    for kind in ["session_keepalive", "session_welcome", "session_reconnect"] {
        let frame = json!({
            "metadata": {"message_id": "control", "message_type": kind,
                "message_timestamp": "2026-11-01T00:00:00Z"},
            "payload": {"session": {"id": "session", "status": "connected",
                "connected_at": "2026-11-01T00:00:00Z", "keepalive_timeout_seconds": 10,
                "reconnect_url": "wss://eventsub.wss.twitch.tv/ws"}}
        })
        .to_string();
        assert!(
            process_frame(&frame, &sender, settings).unwrap().is_some(),
            "{kind}"
        );
    }
    drop(receiver);
    assert!(process_frame(&valid, &sender, settings).is_err());
}

#[derive(Clone, Default)]
struct ErrorRecorder(Arc<Mutex<Vec<String>>>);

#[derive(Default)]
struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        write!(self.0, "{field}={value:?} ").unwrap();
    }
}

impl<S: tracing::Subscriber> Layer<S> for ErrorRecorder {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        if event.metadata().level() == &tracing::Level::ERROR {
            let mut fields = Fields::default();
            event.record(&mut fields);
            self.0.lock().unwrap().push(fields.0);
        }
    }
}

#[test]
fn ordinary_errors_are_throttled_by_signature_and_can_be_unthrottled() {
    if logging::run_log_test_in_child(
        "eventsub::diagnostics_tests::ordinary_errors_are_throttled_by_signature_and_can_be_unthrottled",
    ) {
        return;
    }
    let recorder = ErrorRecorder::default();
    let subscriber = tracing_subscriber::registry()
        .with(recorder.clone())
        .with(tracing_subscriber::filter::LevelFilter::DEBUG);
    tracing::subscriber::with_default(subscriber, || {
        let (sender, _receiver) = sync_channel(1);
        let mut frame = chat_frame();
        frame["payload"]["event"]["message_type"] = json!("diagnostics_throttle_test");
        let text = frame.to_string();
        process_frame(&text, &sender, EventSubSettings::default()).unwrap();
        process_frame(&text, &sender, EventSubSettings::default()).unwrap();
        assert_eq!(
            recorder.0.lock().unwrap().len(),
            1,
            "{}",
            failure(&frame).message()
        );
        frame["payload"]["event"]["message_type"] = json!("diagnostics_distinct_test");
        process_frame(&frame.to_string(), &sender, EventSubSettings::default()).unwrap();
        assert_eq!(recorder.0.lock().unwrap().len(), 2);
        let settings = EventSubSettings {
            repeated_log_throttle_enabled: false,
            ..Default::default()
        };
        process_frame(&text, &sender, settings).unwrap();
        process_frame(&text, &sender, settings).unwrap();
        let events = recorder.0.lock().unwrap();
        assert_eq!(events.len(), 4);
        for message in events.iter() {
            assert!(message.contains("EventSub frame skipped"));
            assert!(message.contains("event.message_type"));
            assert!(message.contains("sample"));
            assert!(!message.contains("private chat text"));
        }
    });
}

use std::collections::BTreeSet;
use std::fmt;

use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{value::RawValue, Value};
use tracing::debug;
use twitch_api::eventsub::{
    channel::chat::{message::MessageType, ChannelChatMessageV1},
    Event, EventSubscription, EventsubWebsocketData, Message, PayloadParseError,
};

use super::{
    diagnostics::{protocol_value, ParseFailure},
    EventNotification,
};
use crate::types::ChannelMessageType;

pub(super) enum DecodedFrame<'a> {
    Parsed(EventsubWebsocketData<'a>),
    Recovered(EventNotification),
    Ignored,
}

#[derive(Deserialize)]
struct Route {
    metadata: RouteMetadata,
}

#[derive(Deserialize)]
struct RouteMetadata {
    message_type: String,
}

#[derive(Deserialize)]
struct NotificationEnvelope<'a> {
    metadata: NotificationMetadata,
    #[serde(borrow)]
    payload: NotificationPayload<'a>,
}

#[derive(Deserialize)]
struct NotificationMetadata {
    #[serde(rename = "message_id")]
    _message_id: String,
    #[serde(rename = "message_timestamp")]
    _message_timestamp: twitch_api::types::Timestamp,
    subscription_type: String,
    subscription_version: String,
}

#[derive(Deserialize)]
struct NotificationPayload<'a> {
    subscription: SubscriptionRoute,
    #[serde(borrow)]
    event: &'a RawValue,
}

#[derive(Deserialize)]
struct SubscriptionRoute {
    #[serde(rename = "type")]
    event_type: String,
    version: String,
}

pub(super) fn decode(frame: &str) -> Result<DecodedFrame<'_>, ParseFailure> {
    serde_json::from_str::<&RawValue>(frame).map_err(|error| {
        ParseFailure::new(
            frame,
            &PayloadParseError::DeserializeError(twitch_api::DeserError::PathError {
                path: ".".into(),
                error,
            }),
        )
    })?;
    let route: Route = parse_envelope(frame)?;
    if route.metadata.message_type != "notification" {
        return Event::parse_websocket(frame)
            .map(DecodedFrame::Parsed)
            .map_err(|error| ParseFailure::new(frame, &error));
    }

    let envelope: NotificationEnvelope<'_> = parse_envelope(frame)?;
    let event_type = envelope.metadata.subscription_type.as_str();
    let version = envelope.metadata.subscription_version.as_str();
    if event_type != envelope.payload.subscription.event_type
        || version != envelope.payload.subscription.version
    {
        return Err(ParseFailure::invalid_envelope(
            frame,
            "payload.subscription",
            "subscription type/version does not match notification metadata",
        ));
    }
    if !envelope.payload.event.get().starts_with('{') {
        return Err(ParseFailure::invalid_envelope(
            frame,
            "payload.event",
            "invalid type: expected an event object",
        ));
    }
    if event_type != ChannelChatMessageV1::EVENT_TYPE.to_str()
        || version != ChannelChatMessageV1::VERSION
    {
        #[derive(Deserialize)]
        struct Notice {
            notice_type: Option<String>,
        }
        let notice = serde_json::from_str::<Notice>(envelope.payload.event.get())
            .ok()
            .and_then(|notice| notice.notice_type.as_deref().map(protocol_value));
        debug!(
            event_type = %protocol_value(event_type),
            event_version = %protocol_value(version),
            notice_type = ?notice,
            "EventSub notification ignored: no application handler"
        );
        return Ok(DecodedFrame::Ignored);
    }

    match Event::parse_websocket(frame) {
        Ok(parsed) => Ok(DecodedFrame::Parsed(parsed)),
        Err(error) => recover_chat(frame, &error),
    }
}

fn parse_envelope<'a, T: Deserialize<'a>>(frame: &'a str) -> Result<T, ParseFailure> {
    twitch_api::parse_json(frame, false)
        .map_err(|error| ParseFailure::new(frame, &PayloadParseError::DeserializeError(error)))
}

fn recover_chat<'a>(
    frame: &'a str,
    original_error: &PayloadParseError,
) -> Result<DecodedFrame<'a>, ParseFailure> {
    let UniqueValue(mut raw) = parse_envelope(frame)?;
    let event = &mut raw["payload"]["event"];
    let mut fragment_types = BTreeSet::new();
    let mut omitted_fragments = BTreeSet::new();
    if let Some(fragments) = event
        .pointer_mut("/message/fragments")
        .and_then(Value::as_array_mut)
    {
        for (index, fragment) in fragments.iter_mut().enumerate() {
            let Some(kind) = fragment.get("type").and_then(Value::as_str) else {
                continue;
            };
            if matches!(kind, "text" | "emote" | "cheermote" | "mention") {
                continue;
            }
            fragment_types.insert(protocol_value(kind));
            fragment["type"] = Value::String("text".into());
            if fragment.get("text").is_none_or(Value::is_null) {
                fragment["text"] = Value::String(String::new());
            }
            if fragment["text"].as_str() == Some("") {
                omitted_fragments.insert(index);
            }
        }
    }
    let unknown_message_type = event["message_type"]
        .as_str()
        .filter(|kind| {
            serde_json::from_value::<MessageType>(Value::String((*kind).into())).is_err()
        })
        .map(protocol_value);
    if unknown_message_type.is_some() {
        event["message_type"] = Value::String("text".into());
    }
    if fragment_types.is_empty() && unknown_message_type.is_none() {
        return Err(ParseFailure::new(frame, original_error));
    }

    // Keep fragment indexes stable until validation so failure paths match the original frame.
    if event["message"]["text"]
        .as_str()
        .is_some_and(|text| text.trim().is_empty())
    {
        if let Some(fragments) = event["message"]["fragments"].as_array() {
            let text: String = fragments
                .iter()
                .filter_map(|fragment| fragment.get("text").and_then(Value::as_str))
                .collect();
            event["message"]["text"] = Value::String(text);
        }
    }
    let normalized = raw.to_string();
    let parsed = Event::parse_websocket(&normalized)
        .map_err(|error| ParseFailure::new(frame, &error).without_position())?;
    let EventsubWebsocketData::Notification {
        metadata,
        mut payload,
    } = parsed
    else {
        unreachable!("normalization only changes chat content");
    };
    if let Event::ChannelChatMessageV1(chat) = &mut payload {
        if let Message::Notification(message) = &mut chat.message {
            if message.message.text.trim().is_empty() {
                debug!(
                    fragment_types = ?fragment_types,
                    message_type = ?unknown_message_type,
                    "EventSub chat message omitted: unsupported content has no readable text"
                );
                return Ok(DecodedFrame::Ignored);
            }
            let mut index = 0;
            message.message.fragments.retain(|_| {
                let keep = !omitted_fragments.contains(&index);
                index += 1;
                keep
            });
        }
    }
    debug!(
        fragment_types = ?fragment_types,
        message_type = ?unknown_message_type,
        "EventSub unsupported chat content handled as text"
    );
    Ok(DecodedFrame::Recovered(EventNotification {
        ts: metadata.message_timestamp.into_owned(),
        event: payload,
        message_type_override: unknown_message_type.map(|_| ChannelMessageType::Unknown),
    }))
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor).map(Self)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object fields")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("expected a finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(UniqueValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate field `{key}`")));
            }
            let UniqueValue(value) = object.next_value()?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

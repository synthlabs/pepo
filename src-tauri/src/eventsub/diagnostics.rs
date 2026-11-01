use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;
use twitch_api::{eventsub::PayloadParseError, DeserError};

const SAMPLE_LIMIT: usize = 4 * 1024;

#[derive(Debug, Serialize)]
pub(super) struct ParseFailure {
    message_type: Option<String>,
    event_type: Option<String>,
    event_version: Option<String>,
    error_kind: &'static str,
    field_path: Option<String>,
    reason: String,
    line: Option<usize>,
    column: Option<usize>,
    frame_bytes: usize,
    sample: Option<String>,
    sample_truncated: bool,
}

impl ParseFailure {
    pub(super) fn new(frame: &str, error: &PayloadParseError) -> Self {
        let (raw, syntax_error) = match serde_json::from_str::<Value>(frame) {
            Ok(raw) => (Some(raw), None),
            Err(error) => (None, Some(error)),
        };
        let tag = |pointer| {
            raw.as_ref()
                .and_then(|raw| raw.pointer(pointer))
                .and_then(Value::as_str)
                .map(protocol_value)
        };
        let mut diagnostic = Self {
            message_type: tag("/metadata/message_type"),
            event_type: tag("/metadata/subscription_type")
                .or_else(|| tag("/payload/subscription/type")),
            event_version: tag("/metadata/subscription_version")
                .or_else(|| tag("/payload/subscription/version")),
            error_kind: "other",
            field_path: None,
            reason: "unrecognized parser error".into(),
            line: None,
            column: None,
            frame_bytes: frame.len(),
            sample: None,
            sample_truncated: false,
        };
        let mut protocol_values = BTreeSet::new();
        if let Some(mut raw) = raw {
            scrub_sample(&mut raw, "", &mut protocol_values);
            let mut sample = raw.to_string();
            diagnostic.sample_truncated = truncate(&mut sample, SAMPLE_LIMIT);
            diagnostic.sample = Some(sample);
        }
        if let Some(error) = syntax_error {
            diagnostic.set_json_error(None, &error, &protocol_values);
            return diagnostic;
        }
        match error {
            PayloadParseError::DeserializeError(DeserError::PathError { path, error }) => {
                diagnostic.set_json_error(Some(path), error, &protocol_values);
            }
            PayloadParseError::UnknownMessageType(_) => {
                diagnostic.error_kind = "unknown_message_type";
                diagnostic.reason = "unsupported message type".into();
            }
            PayloadParseError::UnknownEventType(_) => {
                diagnostic.error_kind = "unknown_event_type";
                diagnostic.reason = "unsupported event type".into();
            }
            PayloadParseError::UnimplementedEvent { .. } => {
                diagnostic.error_kind = "unimplemented_event";
                diagnostic.reason = "unsupported event type/version".into();
            }
            PayloadParseError::MalformedEvent => {
                diagnostic.error_kind = "malformed_event";
                diagnostic.reason = "missing event context".into();
            }
            PayloadParseError::Utf8Error(_) => {
                diagnostic.error_kind = "utf8";
                diagnostic.reason = "invalid UTF-8".into();
            }
            _ => {}
        }
        truncate(&mut diagnostic.reason, 512);
        diagnostic
    }

    fn set_json_error(
        &mut self,
        path: Option<&str>,
        error: &serde_json::Error,
        protocol_values: &BTreeSet<String>,
    ) {
        self.error_kind = match error.classify() {
            serde_json::error::Category::Data => "schema",
            serde_json::error::Category::Syntax => "syntax",
            serde_json::error::Category::Eof => "eof",
            serde_json::error::Category::Io => "io",
        };
        self.field_path = path.map(str::to_owned);
        self.reason = safe_reason(error, protocol_values);
        self.line = Some(error.line());
        self.column = Some(error.column());
    }

    pub(super) fn key(&self) -> String {
        format!(
            "eventsub_unparseable:{}",
            serde_json::json!([
                self.message_type,
                self.event_type,
                self.event_version,
                self.error_kind,
                self.field_path,
                self.reason,
            ])
        )
    }

    pub(super) fn message(&self) -> String {
        format!(
            "EventSub frame skipped after parsing failed: {}",
            serde_json::to_string(self).expect("diagnostic contains only JSON values")
        )
    }
}

fn protocol_path(path: &str) -> bool {
    matches!(
        path,
        "metadata.message_type"
            | "metadata.subscription_type"
            | "metadata.subscription_version"
            | "payload.subscription.type"
            | "payload.subscription.version"
            | "payload.subscription.status"
            | "payload.subscription.transport.method"
            | "payload.event.message_type"
            | "payload.event.notice_type"
            | "payload.event.message.fragments[].type"
    )
}

fn protocol_value(value: &str) -> String {
    if value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        value.into()
    } else {
        "<string>".into()
    }
}

fn scrub_sample(value: &mut Value, path: &str, protocol_values: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                scrub_sample(value, &child, protocol_values);
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub_sample(value, &format!("{path}[]"), protocol_values);
            }
        }
        Value::String(text) if protocol_path(path) => {
            *text = protocol_value(text);
            protocol_values.insert(text.clone());
        }
        Value::String(_) => *value = Value::String("<string>".into()),
        Value::Number(_) => *value = Value::String("<number>".into()),
        Value::Bool(_) => *value = Value::String("<boolean>".into()),
        Value::Null => {}
    }
}

fn safe_reason(error: &serde_json::Error, protocol_values: &BTreeSet<String>) -> String {
    let message = error.to_string();
    let position = format!(" at line {} column {}", error.line(), error.column());
    let reason = message.strip_suffix(&position).unwrap_or(&message);
    if !error.is_data() {
        return reason.into();
    }
    // Serde can echo rejected input. Keep expected schema text, not arbitrary values.
    if let Some((actual, expected)) = reason.rsplit_once(", expected ") {
        if let Some(variant) = actual
            .strip_prefix("unknown variant `")
            .and_then(|variant| variant.strip_suffix('`'))
        {
            let variant = if protocol_values.contains(variant) {
                variant
            } else {
                "<string>"
            };
            return format!("unknown variant `{variant}`, expected {expected}");
        }
        for prefix in ["invalid type: ", "invalid value: "] {
            if let Some(actual) = actual.strip_prefix(prefix) {
                let kind = actual.split_whitespace().next().unwrap_or("value");
                let kind = match kind {
                    "string" | "integer" | "floating" | "boolean" | "map" | "sequence" | "null"
                    | "unit" => kind,
                    _ => "value",
                };
                return format!("{prefix}{kind} (value omitted), expected {expected}");
            }
        }
    }
    for prefix in ["missing field `", "duplicate field `"] {
        if let Some(field) = reason
            .strip_prefix(prefix)
            .and_then(|field| field.strip_suffix('`'))
        {
            if protocol_value(field) == field {
                return reason.into();
            }
        }
    }
    "data did not match expected schema (unrecognized detail omitted)".into()
}

fn truncate(text: &mut String, limit: usize) -> bool {
    if text.len() <= limit {
        return false;
    }
    const MARKER: &str = "...[truncated]";
    let mut end = limit - MARKER.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(MARKER);
    true
}

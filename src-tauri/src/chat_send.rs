use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use twitch_api::helix::{chat::SendChatMessageResponse, ClientRequestError, HelixRequestPostError};

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ChatSendErrorKind {
    Authentication,
    PermissionDenied,
    RateLimited,
    Rejected,
    Connection,
    Timeout,
    Unexpected,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
pub struct ChatSendError {
    pub kind: ChatSendErrorKind,
    pub message: String,
    pub details: String,
    pub attempt_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, specta::Type)]
pub struct ChatSendDiagnostics {
    pub attempt_id: String,
    pub channel_login: String,
    pub last_completion: Option<EmoteCompletionDiagnostics>,
    pub key_events: Vec<ChatSendKeyEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct EmoteCompletionDiagnostics {
    pub mode: String,
    pub method: String,
    pub emote_id: String,
    pub emote_name: String,
    pub provider: String,
    pub scope: String,
    pub edited_since_selection: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct ChatSendKeyEvent {
    pub key: String,
    pub shift: bool,
    pub repeat: bool,
    pub is_composing: bool,
    pub picker_visible: bool,
    pub result_count: usize,
    pub selected_index: Option<usize>,
    #[specta(type = specta_typescript::Number)]
    pub age_ms: f64,
}

pub struct SendAttempt<'a> {
    diagnostics: ChatSendDiagnostics,
    broadcaster_id: &'a str,
    message: &'a str,
    started: Instant,
}

impl<'a> SendAttempt<'a> {
    pub fn new(
        broadcaster_id: &'a str,
        message: &'a str,
        diagnostics: Option<ChatSendDiagnostics>,
    ) -> Self {
        static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);
        let mut diagnostics = diagnostics.unwrap_or_default();
        if diagnostics.attempt_id.is_empty() {
            diagnostics.attempt_id = format!(
                "native-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT_ATTEMPT.fetch_add(1, Ordering::Relaxed),
            );
        }
        if diagnostics.key_events.len() > 16 {
            diagnostics
                .key_events
                .drain(..diagnostics.key_events.len() - 16);
        }
        let attempt = Self {
            diagnostics,
            broadcaster_id,
            message,
            started: Instant::now(),
        };
        info!("chat.send.native_attempt {}", attempt.record(None));
        attempt
    }

    fn record(&self, sender_id: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "attempt_id": self.diagnostics.attempt_id,
            "channel_login": self.diagnostics.channel_login,
            "broadcaster_id": self.broadcaster_id,
            "sender_id": sender_id,
            "utf8_bytes": self.message.len(),
            "utf16_length": self.message.encode_utf16().count(),
            "scalar_length": self.message.chars().count(),
            "elapsed_ms": self.started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    fn failure(
        &self,
        sender_id: Option<&str>,
        mut error: ChatSendError,
        outcome: serde_json::Value,
    ) -> ChatSendError {
        error.attempt_id = Some(self.diagnostics.attempt_id.clone());
        let mut record = self.record(sender_id);
        record["kind"] = serde_json::json!(error.kind);
        record["outcome"] = outcome;
        record["message"] = serde_json::json!(self.message);
        record["unusual_codepoints"] = serde_json::json!(self.message.chars().enumerate()
            .filter(|(_, ch)| !ch.is_ascii_graphic())
            .map(|(index, ch)| serde_json::json!({"index": index, "codepoint": format!("U+{:04X}", ch as u32)}))
            .collect::<Vec<_>>());
        record["composer"] = serde_json::json!(self.diagnostics);
        warn!("chat.send.native_failure {record}");
        error
    }

    pub fn missing_token(&self) -> ChatSendError {
        self.failure(
            None,
            ChatSendError::missing_token(),
            serde_json::json!({"stage": "authentication"}),
        )
    }

    pub fn response(
        &self,
        sender_id: &str,
        response: SendChatMessageResponse,
    ) -> Result<(), ChatSendError> {
        let outcome = serde_json::json!({
            "is_sent": response.is_sent,
            "message_id": response.message_id,
            "drop_reason": response.drop_reason,
        });
        match check_response(response) {
            Ok(()) => {
                let mut record = self.record(Some(sender_id));
                record["outcome"] = outcome;
                info!("chat.send.native_result {record}");
                Ok(())
            }
            Err(error) => Err(self.failure(Some(sender_id), error, outcome)),
        }
    }

    pub fn request_error(&self, sender_id: &str, error: RequestError) -> ChatSendError {
        let outcome =
            serde_json::json!({"stage": "request", "http_status": response_status(&error)});
        self.failure(Some(sender_id), ChatSendError::from_request(error), outcome)
    }
}

const UNEXPECTED_MESSAGE: &str = "Could not send this message.";

type RequestError = ClientRequestError<reqwest::Error>;

impl ChatSendError {
    fn new(kind: ChatSendErrorKind, message: impl Into<String>, details: String) -> Self {
        Self {
            kind,
            message: message.into(),
            details,
            attempt_id: None,
        }
    }

    pub fn missing_token() -> Self {
        Self::new(
            ChatSendErrorKind::Authentication,
            "Sign in to send messages.",
            "No active Twitch token".to_owned(),
        )
    }

    pub fn from_request(error: RequestError) -> Self {
        use ChatSendErrorKind::*;
        let (kind, message) = match (response_status(&error), &error) {
            (Some(401), _) => (
                Authentication,
                "Twitch could not authenticate this message. Try signing in again.",
            ),
            (Some(403), _) => (
                PermissionDenied,
                "Twitch denied permission to send messages in this channel.",
            ),
            (Some(429), _) => (
                RateLimited,
                "Twitch is limiting your messages. Wait before sending again.",
            ),
            (_, ClientRequestError::RequestError(source)) if source.is_timeout() => (
                Timeout,
                "Sending timed out. Your message may or may not have been delivered.",
            ),
            (_, ClientRequestError::RequestError(source)) if source.is_connect() => (
                Connection,
                "Could not connect to Twitch. Check your connection and try again.",
            ),
            _ => (Unexpected, UNEXPECTED_MESSAGE),
        };
        Self::new(kind, message, request_details(&error))
    }
}

fn response_status(error: &RequestError) -> Option<u16> {
    match error {
        ClientRequestError::HelixRequestPostError(
            HelixRequestPostError::Error { status, .. }
            | HelixRequestPostError::InvalidResponse { status, .. }
            | HelixRequestPostError::DeserializeError(_, _, _, status),
        ) => Some(status.as_u16()),
        _ => None,
    }
}

// The Error variant carries the request body, which holds the chat text; keep it out of reports.
fn request_details(error: &RequestError) -> String {
    match error {
        ClientRequestError::HelixRequestPostError(HelixRequestPostError::Error {
            error: reason,
            status,
            message,
            uri,
            ..
        }) => format!("{} {reason}: {message} ({uri})", status.as_u16()),
        _ => format!("{error:?}"),
    }
}

pub fn check_response(response: SendChatMessageResponse) -> Result<(), ChatSendError> {
    if response.is_sent {
        return Ok(());
    }
    let details = format!("{response:?}");
    if let Some(reason) = response.drop_reason {
        if !reason.message.trim().is_empty() {
            return Err(ChatSendError::new(
                ChatSendErrorKind::Rejected,
                reason.message,
                details,
            ));
        }
    }
    Err(ChatSendError::new(
        ChatSendErrorKind::Unexpected,
        UNEXPECTED_MESSAGE,
        details,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Duration;

    fn response(is_sent: bool, reason: serde_json::Value) -> SendChatMessageResponse {
        serde_json::from_value(serde_json::json!({
            "message_id": "", "is_sent": is_sent, "drop_reason": reason,
        }))
        .unwrap()
    }

    fn request_error(status: u16) -> RequestError {
        HelixRequestPostError::Error {
            error: "Failure".to_owned(),
            status: status.try_into().unwrap(),
            message: "private diagnostic".to_owned(),
            uri: "https://api.twitch.tv/helix/chat/messages".parse().unwrap(),
            body: "secret request body".into(),
        }
        .into()
    }

    #[test]
    fn confirms_delivery_and_preserves_known_or_unknown_refusals() {
        assert!(check_response(response(true, serde_json::Value::Null)).is_ok());
        for code in ["msg_rejected", "future_code"] {
            let error = check_response(response(
                false,
                serde_json::json!({
                    "code": code, "message": "Your message is being checked by mods.",
                }),
            ))
            .unwrap_err();
            assert_eq!(error.kind, ChatSendErrorKind::Rejected);
            assert_eq!(error.message, "Your message is being checked by mods.");
            assert!(error.details.contains("code"));
        }
    }

    #[test]
    fn unexplained_refusals_are_technical_failures() {
        for reason in [
            serde_json::Value::Null,
            serde_json::json!({"code":"future_code", "message":"  "}),
        ] {
            assert_eq!(
                check_response(response(false, reason)).unwrap_err().kind,
                ChatSendErrorKind::Unexpected
            );
        }
    }

    #[test]
    fn classifies_only_supported_statuses_and_keeps_diagnostics_separate() {
        for (status, expected) in [
            (401, ChatSendErrorKind::Authentication),
            (403, ChatSendErrorKind::PermissionDenied),
            (429, ChatSendErrorKind::RateLimited),
            (400, ChatSendErrorKind::Unexpected),
            (422, ChatSendErrorKind::Unexpected),
            (500, ChatSendErrorKind::Unexpected),
        ] {
            let error = ChatSendError::from_request(request_error(status));
            assert_eq!(error.kind, expected);
            assert!(!error.message.contains("private diagnostic"));
            assert!(error.details.contains("private diagnostic"));
            assert!(error.details.contains(&status.to_string()));
            assert!(error.details.contains("helix/chat/messages"));
            assert!(!error.details.contains("secret request body"));
        }
        assert_eq!(
            ChatSendError::missing_token().kind,
            ChatSendErrorKind::Authentication
        );
        let error = ChatSendError::from_request(ClientRequestError::Custom("401 timeout".into()));
        assert_eq!(error.kind, ChatSendErrorKind::Unexpected);
        assert!(error.details.contains("401 timeout"));
    }

    #[test]
    fn send_diagnostics_are_correlated_lossless_and_visible_at_info() {
        if crate::logging::run_log_test_in_child(
            "chat_send::tests::send_diagnostics_are_correlated_lossless_and_visible_at_info",
        ) {
            return;
        }
        use std::sync::{Arc, Mutex};
        #[derive(Clone)]
        struct Output(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Output {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let captured = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || Output(captured.clone()))
            .finish();
        let payload = "Kappa \u{200b}\n🥳";
        tracing::subscriber::with_default(subscriber, || {
            let completion = EmoteCompletionDiagnostics {
                mode: "colon".into(),
                method: "keyboard".into(),
                emote_id: "25".into(),
                emote_name: "private-success-emote".into(),
                provider: "Twitch".into(),
                scope: "Global".into(),
                edited_since_selection: false,
            };
            let success = SendAttempt::new(
                "1",
                "private-success-message",
                Some(ChatSendDiagnostics {
                    attempt_id: "success-id".into(),
                    channel_login: "maya".into(),
                    last_completion: Some(completion.clone()),
                    key_events: Vec::new(),
                }),
            );
            success
                .response("2", response(true, serde_json::Value::Null))
                .unwrap();
            let rejection = SendAttempt::new(
                "1",
                payload,
                Some(ChatSendDiagnostics {
                    attempt_id: "rejection-id".into(),
                    channel_login: "maya".into(),
                    last_completion: Some(EmoteCompletionDiagnostics {
                        emote_name: "Kappa".into(),
                        ..completion
                    }),
                    key_events: Vec::new(),
                }),
            );
            let error = rejection
                .response(
                    "2",
                    response(
                        false,
                        serde_json::json!({
                            "code": "msg_rejected", "message": "Please try again later."
                        }),
                    ),
                )
                .unwrap_err();
            assert_eq!(error.attempt_id.as_deref(), Some("rejection-id"));
            assert_eq!(error.kind, ChatSendErrorKind::Rejected);
            assert!(!error.details.contains(payload));
            let transport = SendAttempt::new(
                "1",
                "failed request",
                Some(ChatSendDiagnostics {
                    attempt_id: "request-id".into(),
                    ..Default::default()
                }),
            );
            assert_eq!(
                transport
                    .request_error("2", request_error(401))
                    .attempt_id
                    .as_deref(),
                Some("request-id")
            );
            let absent = SendAttempt::new("1", "no token", None);
            let next = SendAttempt::new("1", "no token", None);
            assert_ne!(absent.diagnostics.attempt_id, next.diagnostics.attempt_id);
            assert!(!absent.diagnostics.attempt_id.is_empty());
            assert_eq!(
                absent.missing_token().attempt_id.as_deref(),
                Some(absent.diagnostics.attempt_id.as_str())
            );
        });
        let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("private-success-message"));
        assert!(!output.contains("private-success-emote"));
        assert!(!output.contains("secret request body"));
        assert!(!output.contains("private diagnostic"));
        let records: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(&line[line.find('{').unwrap()..]).unwrap())
            .collect();
        let rejection = records
            .iter()
            .find(|record| {
                record["attempt_id"] == "rejection-id" && record.get("message").is_some()
            })
            .unwrap();
        assert_eq!(rejection["message"], payload);
        assert_eq!(rejection["utf8_bytes"], 14);
        assert_eq!(rejection["utf16_length"], 10);
        assert_eq!(rejection["scalar_length"], 9);
        assert_eq!(
            rejection["unusual_codepoints"],
            serde_json::json!([
                {"index":5,"codepoint":"U+0020"}, {"index":6,"codepoint":"U+200B"},
                {"index":7,"codepoint":"U+000A"}, {"index":8,"codepoint":"U+1F973"},
            ])
        );
        assert_eq!(rejection["outcome"]["drop_reason"]["code"], "msg_rejected");
        assert_eq!(rejection["sender_id"], "2");
        assert_eq!(
            rejection["composer"]["last_completion"]["emote_name"],
            "Kappa"
        );
        assert!(records
            .iter()
            .any(|record| record["attempt_id"] == "request-id"
                && record["outcome"]["http_status"] == 401));
        assert_eq!(output.matches("WARN").count(), 3);
    }

    #[tokio::test]
    async fn classifies_timeout_from_existing_reqwest_flag() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let timeout = client
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap_err();
        assert!(timeout.is_timeout());
        assert_eq!(
            ChatSendError::from_request(ClientRequestError::RequestError(timeout)).kind,
            ChatSendErrorKind::Timeout
        );
    }
}

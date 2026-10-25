use serde::Serialize;
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
}

const UNEXPECTED_MESSAGE: &str = "Could not send this message.";

type RequestError = ClientRequestError<reqwest::Error>;

impl ChatSendError {
    fn new(kind: ChatSendErrorKind, message: impl Into<String>, details: String) -> Self {
        Self {
            kind,
            message: message.into(),
            details,
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

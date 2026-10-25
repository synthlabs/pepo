#[cfg(internal_enabled)]
mod local {
    pub use pepo_internal::{apply_plugins, detect_language, setup, InternalBuildInfo};

    #[cfg(debug_assertions)]
    pub use pepo_internal::specta_builder;
}

#[cfg(not(internal_enabled))]
mod local {
    #[allow(dead_code)]
    pub struct InternalBuildInfo {
        pub app_version: String,
        pub app_commit: String,
        pub build_time: String,
    }

    pub fn apply_plugins<R: tauri::Runtime>(
        builder: tauri::Builder<R>,
        _build_info: InternalBuildInfo,
    ) -> tauri::Builder<R> {
        builder
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn detect_language(
        _app_handle: tauri::AppHandle,
        _channel_login: &str,
        _message_id: &str,
        _text: &str,
    ) {
    }
}

pub use local::{apply_plugins, detect_language, setup, InternalBuildInfo};

#[cfg(all(debug_assertions, internal_enabled))]
pub use local::specta_builder;

pub struct ChatTranslationRequest {
    login: String,
    message_id: String,
    text: String,
}

pub fn ingest_chat_message(
    engine: &crate::chat::ChatEngine,
    message: crate::types::ChannelMessage,
    dispatch: impl FnOnce(ChatTranslationRequest),
) -> bool {
    let translation = ChatTranslationRequest::from_message(&message);
    if !engine.insert(message) {
        return false;
    }
    dispatch(translation);
    true
}

impl ChatTranslationRequest {
    pub fn from_message(message: &crate::types::ChannelMessage) -> Self {
        Self {
            login: message.broadcaster_user_login.clone(),
            message_id: message.message_id.clone(),
            text: crate::types::translation_input_from_fragments(&message.fragments)
                .unwrap_or_default(),
        }
    }

    pub fn dispatch(self, app: tauri::AppHandle) {
        detect_language(app, &self.login, &self.message_id, &self.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translation_is_dispatched_only_for_accepted_messages() {
        let engine = crate::chat::ChatEngine::new(crate::types::ChatSettings::default());
        let message: crate::types::ChannelMessage = serde_json::from_value(json!({
            "ts":"now", "broadcaster_user_id":"room", "broadcaster_user_login":"room", "broadcaster_user_name":"Room",
            "chatter_user_id":"user", "chatter_user_name":"User", "message_id":"message", "text":"hello",
            "fragments":[{"Text":{"index":0,"text":"hello"}}], "message_type":"text", "badges":[],
            "color":"", "translation":null, "index":0, "channel_sequence":0
        })).unwrap();
        let mut sent = Vec::new();
        assert!(ingest_chat_message(&engine, message.clone(), |request| {
            sent.push((request.login, request.message_id, request.text))
        }));
        assert!(!ingest_chat_message(&engine, message.clone(), |_| panic!(
            "duplicate translated"
        )));
        engine.shutdown();
        assert!(!ingest_chat_message(&engine, message, |_| panic!(
            "ended session translated"
        )));
        assert_eq!(sent, [("room".into(), "message".into(), "hello".into())]);
    }
}

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

    #[allow(dead_code)]
    pub fn detect_language(
        _app_handle: tauri::AppHandle,
        _channel_login: &str,
        _message_id: &str,
        _text: &str,
    ) {
    }
}

pub use local::{apply_plugins, setup, InternalBuildInfo};

#[cfg(all(debug_assertions, internal_enabled))]
pub use local::specta_builder;

pub fn ingest_chat_message(
    engine: &crate::chat::ChatEngine,
    message: crate::types::ChannelMessage,
) -> bool {
    engine.insert(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ingestion_accepts_only_unique_messages_in_an_active_session() {
        let engine = crate::chat::ChatEngine::new(crate::types::ChatSettings::default());
        let message: crate::types::ChannelMessage = serde_json::from_value(json!({
            "ts":"now", "broadcaster_user_id":"room", "broadcaster_user_login":"room", "broadcaster_user_name":"Room",
            "chatter_user_id":"user", "chatter_user_name":"User", "message_id":"message", "text":"hello",
            "fragments":[{"Text":{"index":0,"text":"hello"}}], "message_type":"text", "badges":[],
            "color":"", "translation":null, "index":0, "channel_sequence":0
        })).unwrap();
        assert!(ingest_chat_message(&engine, message.clone()));
        assert!(!ingest_chat_message(&engine, message.clone()));
        engine.shutdown();
        assert!(!ingest_chat_message(&engine, message));
    }
}

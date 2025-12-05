use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::emote::{cache::EmoteCacheTrait, Emote};

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub enum Fragment {
    Text(TextFragment),
    Emote(EmoteFragment),
    Cheer(CheerFragment),
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct TextFragment {
    pub index: u64,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct EmoteFragment {
    pub index: u64,
    pub emote: Emote,
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct CheerFragment {
    pub index: u64,
    pub text: String,
}

pub struct Parser {}

impl Parser {
    pub fn parse(message: String, cache: &dyn EmoteCacheTrait) -> Vec<Fragment> {
        let _start_time = std::time::Instant::now();

        let result = vec![Fragment::Text(TextFragment {
            index: 0,
            text: message.clone(),
        })];

        debug!(
            providers = ?cache.providers(),
            fragments = result.len(),
            duration = ?_start_time.elapsed(),
            "parsed message into fragments",
        );

        return result;
    }
}

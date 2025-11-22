use serde::{Deserialize, Serialize};

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
        vec![Fragment::Text(TextFragment {
            index: 0,
            text: message.clone(),
        })]
    }
}

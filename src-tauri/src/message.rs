use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::emote::{cache::EmoteCacheTrait, Emote};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub enum Fragment {
    Text(TextFragment),
    Emote(EmoteFragment),
    Cheer(CheerFragment),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub struct TextFragment {
    pub index: u64,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub struct EmoteFragment {
    pub index: u64,
    pub emote: Emote,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub struct CheerFragment {
    pub index: u64,
    pub text: String,
}

pub struct Parser {}

impl Parser {
    // TODO: clean this up, maybe with an insert_emote_fragment! macro?
    pub fn parse(message: String, cache: &dyn EmoteCacheTrait) -> Vec<Fragment> {
        let _start_time = std::time::Instant::now();

        let mut result: Vec<Fragment> = vec![];
        let mut current = String::new();
        let mut chars = message.chars().peekable();
        let mut index = 0;

        loop {
            let mut word = String::new();
            while let Some(c) = chars.next() {
                if !c.is_alphanumeric() {
                    if let Some(emote) = cache.get_emote(word.clone()) {
                        debug!("found emote: {}", word);
                        if !current.is_empty() {
                            // terminate the currently building text fragment
                            result.push(Fragment::Text(TextFragment {
                                index: index,
                                text: current.clone(),
                            }));
                            current.clear();
                            index = index + 1;
                        }
                        word.clear();

                        // insert emote fragment
                        result.push(Fragment::Emote(EmoteFragment {
                            index: index,
                            emote: emote,
                        }));
                        index = index + 1;
                    }
                }
                word.push(c);
                if !c.is_alphanumeric() {
                    current.push_str(&word);
                    break;
                }
            }
            println!("{}", word);

            if let None = chars.peek() {
                if let Some(emote) = cache.get_emote(word.clone()) {
                    debug!("found emote: {}", word);
                    if !current.is_empty() {
                        // terminate the currently building text fragment
                        result.push(Fragment::Text(TextFragment {
                            index: index,
                            text: current.clone(),
                        }));
                        current.clear();
                        index = index + 1;
                    }
                    word.clear();

                    // insert emote fragment
                    result.push(Fragment::Emote(EmoteFragment {
                        index: index,
                        emote: emote,
                    }));
                    index = index + 1;
                } else {
                    current.push_str(&word);
                }
                break;
            }
        }

        if !current.is_empty() {
            result.push(Fragment::Text(TextFragment {
                index: index,
                text: current.clone(),
            }));
        }

        debug!(
            providers = ?cache.providers(),
            fragments = result.len(),
            msg_len = message.len(),
            duration = ?_start_time.elapsed(),
            "parsed message into fragments",
        );

        return result;
    }
}

#[cfg(test)]
mod tests {
    use tracing::warn;

    use crate::emote::cache::EmoteCache;

    use super::*;

    pub struct NoneCache {
        pub scope: String,
        pub provider: String,
    }

    impl NoneCache {
        pub fn new(scope: String) -> Self {
            NoneCache {
                scope,
                provider: "NoneCache".to_owned(),
            }
        }
    }

    impl EmoteCacheTrait for NoneCache {
        fn name(&self) -> String {
            format!("{}:{}", self.provider.clone(), self.scope.clone())
        }

        fn providers(&self) -> Vec<String> {
            vec![]
        }

        fn set_emote(&self, name: String, _emote: Emote) {
            warn!(name, "setting a value in none cache");
        }

        fn get_emote(&self, name: String) -> Option<Emote> {
            warn!(name, "getting value in a none cache");
            None
        }

        fn has_emote(&self, _name: String) -> bool {
            false
        }
    }

    #[test]
    fn test_parse_message_empty_cache() {
        let emote_cache = NoneCache::new("test_parse_message_empty_cache".to_string());

        let msg = "this is a test message LUL with only twitch emotes".to_string();
        assert_eq!(
            Parser::parse(msg.clone(), &emote_cache),
            vec![Fragment::Text(TextFragment {
                index: 0,
                text: msg.clone()
            })]
        );
    }

    #[test]
    fn test_parse_single_word_empty_cache() {
        let emote_cache = NoneCache::new("test_parse_single_word_empty_cache".to_string());

        let msg = "test".to_string();
        assert_eq!(
            Parser::parse(msg.clone(), &emote_cache),
            vec![Fragment::Text(TextFragment {
                index: 0,
                text: msg.clone()
            })]
        );
    }

    #[test]
    fn test_parse_single_emote_with_cache() {
        let emote_cache = EmoteCache::new(
            "test_simple_emote_cache".to_string(),
            "TestProvider".to_string(),
        );

        let msg = "LUL".to_string();
        let emote = Emote {
            id: "1234".to_string(),
            name: "LUL".to_string(),
            ..Default::default()
        };

        emote_cache.set_emote(emote.name.clone(), emote.clone());

        assert_eq!(
            Parser::parse(msg.clone(), &emote_cache),
            vec![Fragment::Emote(EmoteFragment {
                index: 0,
                emote: emote.clone()
            })]
        );
    }

    #[test]
    fn test_simple_emote_cache() {
        let emote_cache = EmoteCache::new(
            "test_simple_emote_cache".to_string(),
            "TestProvider".to_string(),
        );

        let msg = "this is a test message LUL with only twitch emotes".to_string();
        let emote = Emote {
            id: "1234".to_string(),
            name: "LUL".to_string(),
            ..Default::default()
        };

        emote_cache.set_emote(emote.name.clone(), emote.clone());

        assert_eq!(
            Parser::parse(msg.clone(), &emote_cache),
            vec![
                Fragment::Text(TextFragment {
                    index: 0,
                    text: "this is a test message ".to_string(),
                }),
                Fragment::Emote(EmoteFragment {
                    index: 1,
                    emote: emote.clone()
                }),
                Fragment::Text(TextFragment {
                    index: 2,
                    text: " with only twitch emotes".to_string(),
                })
            ]
        );
    }
}

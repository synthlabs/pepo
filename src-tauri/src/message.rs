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
    pub fn parse(message: String, cache: &dyn EmoteCacheTrait) -> Vec<Fragment> {
        let _start_time = std::time::Instant::now();

        let mut result: Vec<Fragment> = Vec::new();
        let mut current = String::new();
        let mut word = String::new();
        let mut index: u64 = 0;

        let resolve_word = |word: &mut String,
                            current: &mut String,
                            result: &mut Vec<Fragment>,
                            index: &mut u64| {
            if word.is_empty() {
                return;
            }
            if let Some(emote) = cache.get_emote(word.clone()) {
                debug!("found emote: {}", word);
                if !current.is_empty() {
                    result.push(Fragment::Text(TextFragment {
                        index: *index,
                        text: std::mem::take(current),
                    }));
                    *index += 1;
                }
                result.push(Fragment::Emote(EmoteFragment {
                    index: *index,
                    emote,
                }));
                *index += 1;
            } else {
                current.push_str(word);
            }
            word.clear();
        };

        for c in message.chars() {
            if c.is_alphanumeric() {
                word.push(c);
                continue;
            }
            // word boundary
            resolve_word(&mut word, &mut current, &mut result, &mut index);
            current.push(c);
        }
        // trailing word at end-of-input
        resolve_word(&mut word, &mut current, &mut result, &mut index);

        if !current.is_empty() {
            result.push(Fragment::Text(TextFragment {
                index,
                text: current,
            }));
        }

        debug!(
            msg = message,
            msg_len = message.len(),
            providers = ?cache.providers(),
            fragments = result.len(),
            duration = ?_start_time.elapsed(),
            "parsed message into fragments",
        );

        result
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

        fn search_emotes(&self, _query: &str, _limit: usize) -> Vec<Emote> {
            vec![]
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

    fn cache_with(name: &str) -> (EmoteCache, Emote) {
        let cache = EmoteCache::new("test".to_string(), "TestProvider".to_string());
        let emote = Emote {
            id: format!("id-{}", name),
            name: name.to_string(),
            ..Default::default()
        };
        cache.set_emote(emote.name.clone(), emote.clone());
        (cache, emote)
    }

    #[test]
    fn test_parse_empty_message() {
        let cache = NoneCache::new("test".to_string());
        assert_eq!(Parser::parse(String::new(), &cache), vec![]);
    }

    #[test]
    fn test_parse_emote_followed_by_punctuation() {
        let (cache, emote) = cache_with("LUL");
        assert_eq!(
            Parser::parse("LUL!".to_string(), &cache),
            vec![
                Fragment::Emote(EmoteFragment {
                    index: 0,
                    emote: emote.clone(),
                }),
                Fragment::Text(TextFragment {
                    index: 1,
                    text: "!".to_string(),
                }),
            ]
        );
    }

    #[test]
    fn test_parse_two_adjacent_emotes() {
        let (cache, emote) = cache_with("LUL");
        assert_eq!(
            Parser::parse("LUL LUL".to_string(), &cache),
            vec![
                Fragment::Emote(EmoteFragment {
                    index: 0,
                    emote: emote.clone(),
                }),
                Fragment::Text(TextFragment {
                    index: 1,
                    text: " ".to_string(),
                }),
                Fragment::Emote(EmoteFragment {
                    index: 2,
                    emote: emote.clone(),
                }),
            ]
        );
    }

    #[test]
    fn test_parse_unicode_message_without_emotes() {
        // is_alphanumeric is Unicode-aware in Rust; CJK + accents stay in the text fragment.
        let cache = NoneCache::new("test".to_string());
        let msg = "你好 héllo wörld".to_string();
        assert_eq!(
            Parser::parse(msg.clone(), &cache),
            vec![Fragment::Text(TextFragment {
                index: 0,
                text: msg,
            })]
        );
    }
}

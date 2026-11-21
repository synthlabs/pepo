use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::emote::{cache::EmoteCacheTrait, Emote};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub enum Fragment {
    Text(TextFragment),
    Emote(EmoteFragment),
    Cheer(CheerFragment),
    Mention(MentionFragment),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, specta::Type)]
pub struct MentionFragment {
    pub index: u64,
    pub text: String,
    pub login: String,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
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
    #[cfg(test)]
    pub fn parse(message: String, cache: &dyn EmoteCacheTrait) -> Vec<Fragment> {
        Self::parse_message(&message, &[], cache)
    }

    pub fn parse_message(
        message: &str,
        upstream: &[twitch_api::eventsub::channel::chat::Fragment],
        cache: &dyn EmoteCacheTrait,
    ) -> Vec<Fragment> {
        let start_time = std::time::Instant::now();
        use twitch_api::eventsub::channel::chat::Fragment as TwitchFragment;
        let mut result = Vec::new();
        let reconstructed: String = upstream.iter().map(TwitchFragment::text).collect();
        let mut start = 0;
        if reconstructed == message {
            let mut offset = 0;
            for fragment in upstream {
                let end = offset + fragment.text().len();
                if let TwitchFragment::Mention { text, mention } = fragment {
                    Self::parse_mentions(&message[start..offset], cache, &mut result);
                    result.push(Fragment::Mention(MentionFragment {
                        index: 0,
                        text: text.clone(),
                        login: mention.user_login.to_string().to_ascii_lowercase(),
                        user_id: Some(mention.user_id.to_string()),
                        display_name: Some(mention.user_name.to_string()),
                    }));
                    start = end;
                }
                offset = end;
            }
        }
        Self::parse_mentions(&message[start..], cache, &mut result);
        for (index, fragment) in result.iter_mut().enumerate() {
            let position = match fragment {
                Fragment::Text(f) => &mut f.index,
                Fragment::Emote(f) => &mut f.index,
                Fragment::Cheer(f) => &mut f.index,
                Fragment::Mention(f) => &mut f.index,
            };
            *position = index as u64;
        }
        let emote_hits = result
            .iter()
            .filter(|fragment| matches!(fragment, Fragment::Emote(_)))
            .count();
        let duration = start_time.elapsed();
        let providers = cache.providers();
        trace!(
            msg = %message,
            msg_len = message.len(),
            providers = ?providers,
            fragments = result.len(),
            emote_hits,
            duration_ms = crate::logging::duration_ms(duration),
            "parsed message into fragments",
        );
        crate::logging::record_chat_parse(
            message.len(),
            providers,
            result.len(),
            duration,
            emote_hits,
        );
        result
    }

    fn parse_mentions(message: &str, cache: &dyn EmoteCacheTrait, result: &mut Vec<Fragment>) {
        let mut start = 0;
        for (offset, ch) in message.char_indices() {
            if ch != '@' || offset < start {
                continue;
            }
            let before = message[..offset].chars().next_back();
            if before.is_some_and(|c| !c.is_whitespace() && !"([{<\"',;:!?".contains(c)) {
                continue;
            }
            let token = message[..offset]
                .rsplit(char::is_whitespace)
                .next()
                .unwrap_or("");
            let token = token.trim_start_matches(['(', '[', '{', '<', '\'', '"']);
            if token.contains("://") || token.to_ascii_lowercase().starts_with("www.") {
                continue;
            }
            let login_len = message[offset + 1..]
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            let end = offset + 1 + login_len;
            if login_len == 0
                || message[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric())
            {
                continue;
            }
            result.extend(Self::parse_emotes(message[start..offset].to_owned(), cache));
            result.push(Fragment::Mention(MentionFragment {
                index: 0,
                text: message[offset..end].to_owned(),
                login: message[offset + 1..end].to_ascii_lowercase(),
                user_id: None,
                display_name: None,
            }));
            start = end;
        }
        result.extend(Self::parse_emotes(message[start..].to_owned(), cache));
    }

    fn parse_emotes(message: String, cache: &dyn EmoteCacheTrait) -> Vec<Fragment> {
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
                trace!("found emote: {}", word);
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

    fn fragment_text(fragment: &Fragment) -> &str {
        match fragment {
            Fragment::Text(f) => &f.text,
            Fragment::Mention(f) => &f.text,
            Fragment::Cheer(f) => &f.text,
            Fragment::Emote(f) => &f.emote.name,
        }
    }

    #[test]
    fn mentions_preserve_text_boundaries_and_coexist_with_emotes() {
        let (cache, _) = cache_with("Maya");
        let text = "你好 (@MaYa_2), @Maya! Maya @MaYa_2\nname@example.com https://example.com/(@url) www.example.com/(@web) @@bad @é @valid世界";
        let fragments = Parser::parse(text.into(), &cache);
        assert_eq!(
            fragments.iter().map(fragment_text).collect::<String>(),
            text
        );
        let mentions: Vec<_> = fragments
            .iter()
            .filter_map(|f| {
                if let Fragment::Mention(f) = f {
                    Some((f.text.as_str(), f.login.as_str()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            mentions,
            [
                ("@MaYa_2", "maya_2"),
                ("@Maya", "maya"),
                ("@MaYa_2", "maya_2")
            ]
        );
        assert_eq!(
            fragments
                .iter()
                .filter(|f| matches!(f, Fragment::Emote(_)))
                .count(),
            1
        );
    }

    #[test]
    fn twitch_mentions_keep_identity_and_invalid_fragment_spans_fall_back_to_text() {
        use twitch_api::eventsub::channel::chat::Message;
        let cache = NoneCache::new("mentions".into());
        let message: Message = serde_json::from_value(serde_json::json!({
            "text": "Hi @MaYa!", "fragments": [
                { "type": "text", "text": "Hi " },
                { "type": "mention", "text": "@MaYa", "mention": {
                    "user_id": "123", "user_login": "maya", "user_name": "마야"
                }},
                { "type": "text", "text": "!" }
            ]
        }))
        .unwrap();
        let fragments = Parser::parse_message(&message.text, &message.fragments, &cache);
        let Fragment::Mention(mention) = &fragments[1] else {
            panic!("expected mention")
        };
        assert_eq!(mention.user_id.as_deref(), Some("123"));
        assert_eq!(mention.display_name.as_deref(), Some("마야"));
        assert_eq!(mention.text, "@MaYa");
        assert_eq!(mention.index, 1);
        let fallback = Parser::parse_message("Other @MaYa", &message.fragments, &cache);
        assert_eq!(
            fallback.iter().map(fragment_text).collect::<String>(),
            "Other @MaYa"
        );
        assert!(matches!(&fallback[1], Fragment::Mention(f) if f.user_id.is_none()));
        assert_eq!(
            crate::types::translation_input_from_fragments(&fragments).as_deref(),
            Some("Hi @MaYa!")
        );
    }
}

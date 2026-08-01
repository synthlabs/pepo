pub mod translation;

use std::sync::OnceLock;
use std::time::Instant;

#[cfg(feature = "runtime")]
use tauri::Emitter;
#[cfg(feature = "runtime")]
use translation::{Language, OneOrMany, SourceLanguage, TranslateRequest, TranslationClient};
use whatlang::Detector;

pub const COMMANDS: &[&str] = &["internal_build_info"];

static LANGUAGE_DETECTOR: OnceLock<Detector> = OnceLock::new();

#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "runtime", derive(specta::Type))]
pub struct InternalBuildInfo {
    pub app_version: String,
    pub app_commit: String,
    pub build_time: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanguageClassification {
    pub lang: Option<&'static str>,
    pub script: Option<String>,
    pub confidence: Option<f64>,
    pub reliable: bool,
    pub duration_ms: f64,
    pub text_len: usize,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
struct ChannelMessageTranslation {
    source_language: String,
    target_language: String,
    translated_text: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
struct ChannelMessageTranslationUpdate {
    message_id: String,
    translation: ChannelMessageTranslation,
}

#[cfg(any(feature = "runtime", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TranslationDecision {
    Skip {
        reason: &'static str,
        language: ChatLanguage,
    },
    Translate {
        source_language: ChatLanguage,
    },
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct TranslationSkip {
    reason: &'static str,
    language: ChatLanguage,
    raw_lang: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatLanguage {
    En,
    Zh,
    Hi,
    Es,
    Ar,
    Fr,
    Bn,
    Pt,
    Id,
    Ur,
    Ru,
    De,
    Ja,
    Mr,
    Vi,
    Te,
    Tr,
    Other,
}

impl ChatLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
            Self::Hi => "hi",
            Self::Es => "es",
            Self::Ar => "ar",
            Self::Fr => "fr",
            Self::Bn => "bn",
            Self::Pt => "pt",
            Self::Id => "id",
            Self::Ur => "ur",
            Self::Ru => "ru",
            Self::De => "de",
            Self::Ja => "ja",
            Self::Mr => "mr",
            Self::Vi => "vi",
            Self::Te => "te",
            Self::Tr => "tr",
            Self::Other => "other",
        }
    }
}

pub fn classify_language(text: &str) -> LanguageClassification {
    let start = Instant::now();
    let info = detector().detect(text);
    let duration_ms = duration_ms(start.elapsed());

    LanguageClassification {
        lang: info.as_ref().map(|info| info.lang().code()),
        script: info.as_ref().map(|info| info.script().name().to_owned()),
        confidence: info.as_ref().map(|info| info.confidence()),
        reliable: info.as_ref().is_some_and(|info| info.is_reliable()),
        duration_ms,
        text_len: text.len(),
    }
}

pub fn detect_chat_language(text: &str) -> (LanguageClassification, ChatLanguage) {
    let text = clean_chat_translation_text(text);
    let classification = classify_language(&text);
    let language = resolved_chat_language(&text, &classification);

    (classification, language)
}

pub fn log_chat_language(channel_login: &str, message_id: &str, text: &str) {
    let (classification, language) = detect_chat_language(text);

    log_language_classification(channel_login, message_id, &classification, language);
}

#[cfg(feature = "runtime")]
pub fn detect_language(
    app_handle: tauri::AppHandle,
    channel_login: &str,
    message_id: &str,
    text: &str,
) {
    let text = clean_chat_translation_text(text);
    if text.is_empty() {
        log_translation_skip(
            channel_login,
            message_id,
            "empty_input",
            ChatLanguage::Other,
            None,
        );
        return;
    }

    let classification = classify_language(&text);
    let language = resolved_chat_language(&text, &classification);
    log_language_classification(channel_login, message_id, &classification, language);

    let decision = translation_decision(&classification, language);
    let TranslationDecision::Translate { source_language } = decision else {
        if let TranslationDecision::Skip { reason, language } = decision {
            log_translation_skip(
                channel_login,
                message_id,
                reason,
                language,
                classification.lang,
            );
        }
        return;
    };

    let Some((fixed_source, target)) = translation_pair(source_language) else {
        log_translation_skip(
            channel_login,
            message_id,
            "unsupported_language",
            source_language,
            classification.lang,
        );
        return;
    };

    let channel_login = channel_login.to_owned();
    let message_id = message_id.to_owned();
    let translation_event = format!("chat_translation:{channel_login}");

    tauri::async_runtime::spawn(async move {
        let client = match TranslationClient::from_env() {
            Ok(client) => client,
            Err(error) => {
                tracing::debug!(
                    target: "pepo_internal::translation",
                    channel_login,
                    message_id,
                    source = fixed_source.to_string(),
                    target = target.to_string(),
                    error = %error,
                    "failed to create translation client"
                );
                return;
            }
        };

        let request = TranslateRequest::text(text.clone(), target).source(fixed_source);

        match client.translate(request).await {
            Ok(response) => {
                let translation =
                    match translation_from_response(source_language, target, response.clone()) {
                        Ok(translation) => translation,
                        Err(skip) => {
                            log_translation_skip(
                                &channel_login,
                                &message_id,
                                skip.reason,
                                skip.language,
                                skip.raw_lang.as_deref(),
                            );
                            return;
                        }
                    };

                let update = ChannelMessageTranslationUpdate {
                    message_id: message_id.clone(),
                    translation,
                };

                if let Err(error) = app_handle.emit(&translation_event, update) {
                    tracing::debug!(
                        target: "pepo_internal::translation",
                        channel_login,
                        message_id,
                        source = fixed_source.to_string(),
                        target = target.to_string(),
                        error = %error,
                        "failed to emit translated chat message"
                    );
                    return;
                }

                tracing::debug!(
                    target: "pepo_internal::translation",
                    channel_login,
                    message_id,
                    source = fixed_source.to_string(),
                    target = target.to_string(),
                    translated_text = ?response.translated_text,
                    detected_language = ?response.detected_language,
                    model = ?response.model,
                    latency_ms = response.latency_ms,
                    cached = ?response.cached,
                    "translated chat message"
                );
            }
            Err(error) => {
                tracing::debug!(
                    target: "pepo_internal::translation",
                    channel_login,
                    message_id,
                    source = fixed_source.to_string(),
                    target = target.to_string(),
                    error = %error,
                    "failed to translate chat message"
                );
            }
        }
    });
}

fn resolved_chat_language(text: &str, classification: &LanguageClassification) -> ChatLanguage {
    if classification.reliable {
        chat_language(classification.lang)
    } else if accepts_unreliable_russian(text, classification) {
        ChatLanguage::Ru
    } else {
        ChatLanguage::Other
    }
}

#[cfg(any(feature = "runtime", test))]
fn translation_decision(
    classification: &LanguageClassification,
    language: ChatLanguage,
) -> TranslationDecision {
    let raw_language = chat_language(classification.lang);
    if raw_language == ChatLanguage::En {
        return TranslationDecision::Skip {
            reason: "english_source",
            language: ChatLanguage::En,
        };
    }

    match language {
        ChatLanguage::Other if raw_language == ChatLanguage::Other => TranslationDecision::Skip {
            reason: "unsupported_language",
            language: ChatLanguage::Other,
        },
        ChatLanguage::Other => TranslationDecision::Skip {
            reason: "unreliable_language",
            language: raw_language,
        },
        language => TranslationDecision::Translate {
            source_language: language,
        },
    }
}

fn accepts_unreliable_russian(text: &str, classification: &LanguageClassification) -> bool {
    classification.lang == Some("rus")
        && classification.script.as_deref() == Some("Cyrillic")
        && has_minimum_cyrillic_letters(text)
}

fn has_minimum_cyrillic_letters(text: &str) -> bool {
    text.chars().filter(|c| is_cyrillic_letter(*c)).take(3).count() >= 3
}

fn is_cyrillic_letter(c: char) -> bool {
    c.is_alphabetic()
        && matches!(
            c as u32,
            0x0400..=0x04ff
                | 0x0500..=0x052f
                | 0x1c80..=0x1c8f
                | 0x2de0..=0x2dff
                | 0xa640..=0xa69f
        )
}

#[cfg(feature = "runtime")]
fn single_translation_text(value: OneOrMany<String>) -> Option<String> {
    match value {
        OneOrMany::One(text) => Some(text),
        OneOrMany::Many(texts) => texts.into_iter().next(),
    }
}

#[cfg(feature = "runtime")]
fn translation_from_response(
    source_language: ChatLanguage,
    target: Language,
    response: translation::TranslateResponse,
) -> Result<ChannelMessageTranslation, TranslationSkip> {
    let translated_text = single_translation_text(response.translated_text).ok_or_else(|| {
        translation_skip(
            "empty_response",
            source_language,
            Some(source_language.as_str()),
        )
    })?;

    if translated_text.trim().is_empty() {
        return Err(translation_skip(
            "empty_response",
            source_language,
            Some(source_language.as_str()),
        ));
    }

    let target_language = target.to_string();

    Ok(ChannelMessageTranslation {
        source_language: source_language.as_str().to_owned(),
        target_language,
        translated_text,
    })
}

#[cfg(feature = "runtime")]
fn translation_skip(
    reason: &'static str,
    language: ChatLanguage,
    raw_lang: Option<&str>,
) -> TranslationSkip {
    TranslationSkip {
        reason,
        language,
        raw_lang: raw_lang.map(ToOwned::to_owned),
    }
}

fn clean_chat_translation_text(text: &str) -> String {
    let mut tokens = text.split_whitespace().peekable();
    while tokens
        .peek()
        .is_some_and(|token| token.starts_with('@') && token.chars().count() > 1)
    {
        tokens.next();
    }
    tokens
        .filter(|token| !is_chat_url_token(token))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_chat_url_token(token: &str) -> bool {
    let token = token
        .trim_start_matches(|c| matches!(c, '<' | '(' | '[' | '{' | '"' | '\''))
        .trim_end_matches(|c| matches!(c, '>' | ')' | ']' | '}' | '"' | '\'' | ',' | '.'));

    token.starts_with("http://") || token.starts_with("https://") || token.starts_with("www.")
}

fn log_language_classification(
    channel_login: &str,
    message_id: &str,
    classification: &LanguageClassification,
    language: ChatLanguage,
) {
    tracing::debug!(
        target: "pepo_internal::language",
        channel_login,
        message_id,
        text_len = classification.text_len,
        lang = language.as_str(),
        raw_lang = classification.lang.unwrap_or("unknown"),
        script = classification.script.as_deref().unwrap_or("unknown"),
        confidence = classification.confidence.unwrap_or(0.0),
        reliable = classification.reliable,
        duration_ms = classification.duration_ms,
        "classified chat message language"
    );
}

#[cfg(feature = "runtime")]
fn log_translation_skip(
    channel_login: &str,
    message_id: &str,
    reason: &'static str,
    language: ChatLanguage,
    raw_lang: Option<&str>,
) {
    tracing::debug!(
        target: "pepo_internal::translation",
        channel_login,
        message_id,
        reason,
        lang = language.as_str(),
        raw_lang = raw_lang.unwrap_or("unknown"),
        "skipping chat translation"
    );
}

fn duration_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn chat_language(lang: Option<&str>) -> ChatLanguage {
    match lang {
        Some("eng") => ChatLanguage::En,
        Some("cmn") => ChatLanguage::Zh,
        Some("hin") => ChatLanguage::Hi,
        Some("spa") => ChatLanguage::Es,
        Some("ara") => ChatLanguage::Ar,
        Some("fra") => ChatLanguage::Fr,
        Some("ben") => ChatLanguage::Bn,
        Some("por") => ChatLanguage::Pt,
        Some("ind") => ChatLanguage::Id,
        Some("urd") => ChatLanguage::Ur,
        Some("rus") => ChatLanguage::Ru,
        Some("deu") => ChatLanguage::De,
        Some("jpn") => ChatLanguage::Ja,
        Some("mar") => ChatLanguage::Mr,
        Some("vie") => ChatLanguage::Vi,
        Some("tel") => ChatLanguage::Te,
        Some("tur") => ChatLanguage::Tr,
        _ => ChatLanguage::Other,
    }
}

#[cfg(feature = "runtime")]
fn translation_pair(language: ChatLanguage) -> Option<(SourceLanguage, Language)> {
    match language {
        ChatLanguage::En => None,
        ChatLanguage::Zh => Some((SourceLanguage::Zh, Language::En)),
        ChatLanguage::Hi => Some((SourceLanguage::Hi, Language::En)),
        ChatLanguage::Es => Some((SourceLanguage::Es, Language::En)),
        ChatLanguage::Ar => Some((SourceLanguage::Ar, Language::En)),
        ChatLanguage::Fr => Some((SourceLanguage::Fr, Language::En)),
        ChatLanguage::Bn => Some((SourceLanguage::Bn, Language::En)),
        ChatLanguage::Pt => Some((SourceLanguage::Pt, Language::En)),
        ChatLanguage::Id => Some((SourceLanguage::Id, Language::En)),
        ChatLanguage::Ur => Some((SourceLanguage::Ur, Language::En)),
        ChatLanguage::Ru => Some((SourceLanguage::Ru, Language::En)),
        ChatLanguage::De => Some((SourceLanguage::De, Language::En)),
        ChatLanguage::Ja => Some((SourceLanguage::Ja, Language::En)),
        ChatLanguage::Mr => Some((SourceLanguage::Mr, Language::En)),
        ChatLanguage::Vi => Some((SourceLanguage::Vi, Language::En)),
        ChatLanguage::Te => Some((SourceLanguage::Te, Language::En)),
        ChatLanguage::Tr => Some((SourceLanguage::Tr, Language::En)),
        ChatLanguage::Other => None,
    }
}

fn detector() -> &'static Detector {
    LANGUAGE_DETECTOR.get_or_init(Detector::new)
}

#[cfg(feature = "runtime")]
mod runtime {
    use super::InternalBuildInfo;
    use tauri::State;
    use tauri::{plugin::TauriPlugin, Manager, Runtime};

    #[cfg(debug_assertions)]
    use tauri_specta::collect_commands;

    #[tauri::command]
    #[specta::specta]
    fn internal_build_info(build_info: State<'_, InternalBuildInfo>) -> InternalBuildInfo {
        build_info.inner().clone()
    }

    fn init<R: Runtime>(build_info: InternalBuildInfo) -> TauriPlugin<R> {
        tauri::plugin::Builder::<R>::new("internal")
            .invoke_handler(tauri::generate_handler![internal_build_info])
            .setup(move |app, _api| {
                app.manage(build_info);
                Ok(())
            })
            .build()
    }

    pub fn apply_plugins<R: Runtime>(
        builder: tauri::Builder<R>,
        build_info: InternalBuildInfo,
    ) -> tauri::Builder<R> {
        builder.plugin(init(build_info))
    }

    #[cfg(debug_assertions)]
    pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
        tauri_specta::Builder::<tauri::Wry>::new()
            .plugin_name("internal")
            .commands(collect_commands![internal_build_info])
    }

    pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(feature = "runtime")]
pub use runtime::{apply_plugins, setup};

#[cfg(all(debug_assertions, feature = "runtime"))]
pub use runtime::specta_builder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_english_text() {
        let text = "This is a short English message about today and tomorrow.";
        let classification = classify_language(text);

        assert_eq!(classification.lang, Some("eng"));
        assert_eq!(classification.script.as_deref(), Some("Latin"));
        assert!(classification.confidence.is_some());
        assert_eq!(classification.text_len, text.len());
    }

    #[test]
    fn classifies_russian_text() {
        let classification =
            classify_language("Это короткое русское сообщение для проверки языка.");

        assert_eq!(classification.lang, Some("rus"));
        assert_eq!(classification.script.as_deref(), Some("Cyrillic"));
        assert!(classification.confidence.is_some());
    }

    #[test]
    fn handles_empty_text() {
        let classification = classify_language("");

        assert_eq!(classification.lang, None);
        assert_eq!(classification.script, None);
        assert_eq!(classification.confidence, None);
        assert!(!classification.reliable);
        assert_eq!(classification.text_len, 0);
    }

    #[test]
    fn maps_supported_chat_languages() {
        for (raw_lang, language) in [
            ("eng", ChatLanguage::En),
            ("cmn", ChatLanguage::Zh),
            ("hin", ChatLanguage::Hi),
            ("spa", ChatLanguage::Es),
            ("ara", ChatLanguage::Ar),
            ("fra", ChatLanguage::Fr),
            ("ben", ChatLanguage::Bn),
            ("por", ChatLanguage::Pt),
            ("ind", ChatLanguage::Id),
            ("urd", ChatLanguage::Ur),
            ("rus", ChatLanguage::Ru),
            ("deu", ChatLanguage::De),
            ("jpn", ChatLanguage::Ja),
            ("mar", ChatLanguage::Mr),
            ("vie", ChatLanguage::Vi),
            ("tel", ChatLanguage::Te),
            ("tur", ChatLanguage::Tr),
        ] {
            assert_eq!(chat_language(Some(raw_lang)), language);
        }
    }

    #[test]
    fn formats_chat_language_codes() {
        for (language, code) in [
            (ChatLanguage::En, "en"),
            (ChatLanguage::Zh, "zh"),
            (ChatLanguage::Hi, "hi"),
            (ChatLanguage::Es, "es"),
            (ChatLanguage::Ar, "ar"),
            (ChatLanguage::Fr, "fr"),
            (ChatLanguage::Bn, "bn"),
            (ChatLanguage::Pt, "pt"),
            (ChatLanguage::Id, "id"),
            (ChatLanguage::Ur, "ur"),
            (ChatLanguage::Ru, "ru"),
            (ChatLanguage::De, "de"),
            (ChatLanguage::Ja, "ja"),
            (ChatLanguage::Mr, "mr"),
            (ChatLanguage::Vi, "vi"),
            (ChatLanguage::Te, "te"),
            (ChatLanguage::Tr, "tr"),
            (ChatLanguage::Other, "other"),
        ] {
            assert_eq!(language.as_str(), code);
        }
    }

    #[test]
    fn maps_unknown_chat_languages_to_other() {
        assert_eq!(chat_language(Some("epo")), ChatLanguage::Other);
        assert_eq!(chat_language(None), ChatLanguage::Other);
    }

    #[test]
    fn cleans_leading_mentions_and_collapses_whitespace() {
        assert_eq!(
            clean_chat_translation_text("@fasoollka   муха заяви"),
            "муха заяви"
        );
        assert_eq!(
            clean_chat_translation_text("@one @two\tсколько   Летела?"),
            "сколько Летела?"
        );
        assert_eq!(
            clean_chat_translation_text(
                "rosevnFire Donate - 1) DonationAlerts: https://www.donationalerts.com/r/rosevnv 2) DonatePay: https://donatepay.eu/don/25308"
            ),
            "rosevnFire Donate - 1) DonationAlerts: 2) DonatePay:"
        );
    }

    #[test]
    fn ignores_unreliable_chat_language_results() {
        let (classification, language) = detect_chat_language("It works better when it's caked on");

        assert_eq!(classification.lang, Some("deu"));
        assert!(!classification.reliable);
        assert_eq!(language, ChatLanguage::Other);
    }

    #[test]
    fn skips_unreliable_supported_language_candidates() {
        for (text, raw_lang, language) in [
            (
                "da musst du den bösen Wolf anlocken",
                Some("deu"),
                ChatLanguage::De,
            ),
            (
                "Gracias, es lo mas lindo que me dijiste hasta ahora",
                Some("spa"),
                ChatLanguage::Es,
            ),
            (
                "It works better when it's caked on",
                Some("deu"),
                ChatLanguage::De,
            ),
            (
                "rosevnFire Donate - 1) DonationAlerts: https://www.donationalerts.com/r/rosevnv 2) DonatePay: https://donatepay.eu/don/25308",
                Some("fra"),
                ChatLanguage::Fr,
            ),
        ] {
            let (classification, strict_language) = detect_chat_language(text);

            assert_eq!(classification.lang, raw_lang);
            assert!(!classification.reliable);
            assert_eq!(strict_language, ChatLanguage::Other);
            assert_eq!(
                translation_decision(&classification, strict_language),
                TranslationDecision::Skip {
                    reason: "unreliable_language",
                    language,
                }
            );
        }
    }

    #[test]
    fn accepts_short_raw_russian_chat_messages() {
        for text in [
            "Да обычный теплоход",
            "че случилось то кому в рехаб от чего",
        ] {
            let (classification, language) = detect_chat_language(text);

            assert_eq!(classification.lang, Some("rus"), "{text}");
            assert_eq!(classification.script.as_deref(), Some("Cyrillic"), "{text}");
            assert_eq!(language, ChatLanguage::Ru, "{text}: {classification:?}");
        }
    }

    #[test]
    fn accepts_unreliable_russian_cyrillic_candidates() {
        let text = "Что думаете?";
        let classification = LanguageClassification {
            lang: Some("rus"),
            script: Some("Cyrillic".to_owned()),
            confidence: Some(0.0),
            reliable: false,
            duration_ms: 0.0,
            text_len: text.len(),
        };

        let language = resolved_chat_language(text, &classification);

        assert_eq!(language, ChatLanguage::Ru);
        assert_eq!(
            translation_decision(&classification, language),
            TranslationDecision::Translate {
                source_language: ChatLanguage::Ru,
            }
        );
    }

    #[test]
    fn rejects_tiny_unreliable_russian_cyrillic_candidates() {
        let text = "я?";
        let classification = LanguageClassification {
            lang: Some("rus"),
            script: Some("Cyrillic".to_owned()),
            confidence: Some(0.0),
            reliable: false,
            duration_ms: 0.0,
            text_len: text.len(),
        };

        let language = resolved_chat_language(text, &classification);

        assert_eq!(language, ChatLanguage::Other);
        assert_eq!(
            translation_decision(&classification, language),
            TranslationDecision::Skip {
                reason: "unreliable_language",
                language: ChatLanguage::Ru,
            }
        );
    }

    #[test]
    fn keeps_reliable_classifier_language_when_supported() {
        let (classification, language) =
            detect_chat_language("Это короткое русское сообщение для проверки языка.");

        assert_eq!(classification.lang, Some("rus"));
        assert!(classification.reliable);
        assert_eq!(language, ChatLanguage::Ru);
    }

    #[test]
    fn keeps_reliable_translation_decisions_fixed_source() {
        let text = "Это короткое русское сообщение для проверки языка.";
        let classification = classify_language(text);
        let language = resolved_chat_language(text, &classification);

        assert_eq!(
            translation_decision(&classification, language),
            TranslationDecision::Translate {
                source_language: ChatLanguage::Ru,
            }
        );
    }

    #[test]
    fn skips_raw_english_translation_candidates() {
        let classification = LanguageClassification {
            lang: Some("eng"),
            script: Some("Latin".to_owned()),
            confidence: Some(0.25),
            reliable: false,
            duration_ms: 0.0,
            text_len: 12,
        };

        assert_eq!(
            translation_decision(&classification, ChatLanguage::Other),
            TranslationDecision::Skip {
                reason: "english_source",
                language: ChatLanguage::En,
            }
        );
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn fixed_source_emits_changed_non_english_translation() {
        let translation = translation_from_response(
            ChatLanguage::De,
            Language::En,
            translation_response("you have to lure the evil wolf", Some("de")),
        )
        .unwrap();

        assert_eq!(translation.source_language, "de");
        assert_eq!(translation.target_language, "en");
        assert_eq!(
            translation.translated_text,
            "you have to lure the evil wolf"
        );
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn fixed_source_ignores_missing_detection() {
        let translation = translation_from_response(
            ChatLanguage::Es,
            Language::En,
            translation_response(
                "Thank you, it is the nicest thing you have said to me so far",
                None,
            ),
        )
        .unwrap();

        assert_eq!(translation.source_language, "es");
        assert_eq!(translation.target_language, "en");
        assert_eq!(
            translation.translated_text,
            "Thank you, it is the nicest thing you have said to me so far"
        );
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn maps_supported_non_english_languages_to_english() {
        for (language, source) in [
            (ChatLanguage::Zh, SourceLanguage::Zh),
            (ChatLanguage::Hi, SourceLanguage::Hi),
            (ChatLanguage::Es, SourceLanguage::Es),
            (ChatLanguage::Ar, SourceLanguage::Ar),
            (ChatLanguage::Fr, SourceLanguage::Fr),
            (ChatLanguage::Bn, SourceLanguage::Bn),
            (ChatLanguage::Pt, SourceLanguage::Pt),
            (ChatLanguage::Id, SourceLanguage::Id),
            (ChatLanguage::Ur, SourceLanguage::Ur),
            (ChatLanguage::Ru, SourceLanguage::Ru),
            (ChatLanguage::De, SourceLanguage::De),
            (ChatLanguage::Ja, SourceLanguage::Ja),
            (ChatLanguage::Mr, SourceLanguage::Mr),
            (ChatLanguage::Vi, SourceLanguage::Vi),
            (ChatLanguage::Te, SourceLanguage::Te),
            (ChatLanguage::Tr, SourceLanguage::Tr),
        ] {
            assert_eq!(translation_pair(language), Some((source, Language::En)));
        }

        assert_eq!(translation_pair(ChatLanguage::En), None);
        assert_eq!(translation_pair(ChatLanguage::Other), None);
    }

    #[cfg(feature = "runtime")]
    fn translation_response(
        translated_text: impl Into<String>,
        detected_language: Option<&str>,
    ) -> translation::TranslateResponse {
        translation::TranslateResponse {
            translated_text: OneOrMany::One(translated_text.into()),
            detected_language: detected_language
                .map(|language| OneOrMany::One(language.to_owned())),
            model: OneOrMany::One("test-model".to_owned()),
            latency_ms: 1.0,
            cached: OneOrMany::One(false),
        }
    }
}

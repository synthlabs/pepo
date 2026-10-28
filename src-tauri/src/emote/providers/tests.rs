use std::{
    fmt::{self, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    time::Duration,
};

use httpmock::prelude::*;
use tracing::{field::Visit, Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, prelude::*, Layer};

use super::{
    bttv::BttvProvider, ffz::FfzProvider, seventv::SeventvProvider, EmoteProvider, GLOBAL_SCOPE_KEY,
};
use crate::{
    emote::{
        cache::{EmoteCacheTrait, MultiCache},
        persist::{EmoteMetadataStore, MemoryEmoteMetadataStore, SharedEmoteMetadataStore},
        Emote,
    },
    types::{EmoteProviderId, ProviderSettings},
};

const CHANNEL: &str = "429837742";
const NOW: u64 = 1_800_000_000;

struct ProviderCase {
    id: EmoteProviderId,
    module: &'static str,
    channel_path: &'static str,
    global_path: &'static str,
    missing_body: &'static str,
    empty_body: &'static str,
    populated_body: &'static str,
}

const PROVIDERS: [ProviderCase; 3] = [
    ProviderCase {
        id: EmoteProviderId::Ffz,
        module: "ffz",
        channel_path: "/room/id/429837742",
        global_path: "/set/global",
        missing_body: r#"{"status":404,"error":"Not Found","message":"No such room"}"#,
        empty_body: r#"{"sets":{}}"#,
        populated_body: r#"{"sets":{"1":{"emoticons":[{"id":1,"name":"FreshEmote"}]}}}"#,
    },
    ProviderCase {
        id: EmoteProviderId::Bttv,
        module: "bttv",
        channel_path: "/users/twitch/429837742",
        global_path: "/emotes/global",
        missing_body: r#"{"message":"user not found"}"#,
        empty_body: r#"{"channelEmotes":[],"sharedEmotes":[]}"#,
        populated_body: r#"{"channelEmotes":[{"id":"1","code":"FreshEmote","imageType":"png","animated":false}],"sharedEmotes":[]}"#,
    },
    ProviderCase {
        id: EmoteProviderId::Seventv,
        module: "seventv",
        channel_path: "/users/twitch/429837742",
        global_path: "/emote-sets/global",
        missing_body: r#"{"status":"Not Found","error_code":12000,"error":"user not found"}"#,
        empty_body: r#"{"emote_set":{"emotes":[]}}"#,
        populated_body: r#"{"emote_set":{"emotes":[{"id":"1","name":"FreshEmote","data":{"animated":false,"host":{"url":"//cdn.7tv.app/emote/1"}}}]}}"#,
    },
];

impl ProviderCase {
    fn provider(
        &self,
        persistence: SharedEmoteMetadataStore,
        api_base: String,
    ) -> Box<dyn EmoteProvider<MultiCache>> {
        let provider: Box<dyn EmoteProvider<MultiCache>> = match self.id {
            EmoteProviderId::Ffz => Box::new(FfzProvider::with_api_base(persistence, api_base)),
            EmoteProviderId::Bttv => Box::new(BttvProvider::with_api_base(persistence, api_base)),
            EmoteProviderId::Seventv => {
                Box::new(SeventvProvider::with_api_base(persistence, api_base))
            }
            EmoteProviderId::Twitch => unreachable!(),
        };
        assert!(provider.hydrate_cache(GLOBAL_SCOPE_KEY, &ProviderSettings::default()));
        provider
    }

    fn persistence(&self, cached_channel: bool) -> Arc<MemoryEmoteMetadataStore> {
        let persistence = MemoryEmoteMetadataStore::new(NOW);
        for (scope, name) in [
            (GLOBAL_SCOPE_KEY, "CachedGlobal"),
            (CHANNEL, "CachedChannel"),
        ] {
            if scope == CHANNEL && !cached_channel {
                continue;
            }
            persistence.insert(
                self.id,
                scope,
                vec![Emote {
                    id: name.to_owned(),
                    name: name.to_owned(),
                    provider: self.id.provider_name().to_owned(),
                    ..Default::default()
                }],
                NOW,
            );
        }
        persistence
    }

    fn assert_error(&self, events: &[RecordedEvent], text: &str) {
        assert!(
            events.iter().any(|event| event.level == Level::ERROR
                && event.target.ends_with(self.module)
                && event.fields.contains(text)),
            "{}: expected error containing {text:?}, got {events:?}",
            self.module
        );
    }
}

#[derive(Debug)]
struct RecordedEvent {
    level: Level,
    target: String,
    fields: String,
}

#[derive(Default)]
struct RecordedFields(String);

impl Visit for RecordedFields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        write!(self.0, "{field}={value:?} ").unwrap();
    }
}

#[derive(Clone, Default)]
struct EventRecorder(Arc<Mutex<Vec<RecordedEvent>>>);

impl<S: Subscriber> Layer<S> for EventRecorder {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut fields = RecordedFields::default();
        event.record(&mut fields);
        self.0.lock().unwrap().push(RecordedEvent {
            level: *event.metadata().level(),
            target: event.metadata().target().to_owned(),
            fields: fields.0,
        });
    }
}

fn capture_events(action: impl FnOnce()) -> Vec<RecordedEvent> {
    let recorder = EventRecorder::default();
    let subscriber = tracing_subscriber::registry()
        .with(recorder.clone())
        .with(tracing_subscriber::filter::LevelFilter::DEBUG);
    tracing::subscriber::with_default(subscriber, action);
    let events = std::mem::take(&mut *recorder.0.lock().unwrap());
    events
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_channel_responses_are_debug_only_and_preserve_existing_caches() {
    let server = MockServer::start();
    let client = reqwest::Client::new();
    let settings = ProviderSettings::default();

    for case in &PROVIDERS {
        for body in [case.missing_body, "<html>unfamiliar response</html>", ""] {
            for cached_channel in [false, true] {
                let mut mock = server.mock(|when, then| {
                    when.method(GET).path(case.channel_path);
                    then.status(404).body(body);
                });
                let persistence = case.persistence(cached_channel);
                let provider = case.provider(persistence.clone(), server.base_url());

                let events = capture_events(|| {
                    provider.load_channel_emotes(CHANNEL.to_owned(), &client, &settings);
                });

                assert!(
                    events.iter().all(|event| event.level != Level::ERROR),
                    "{events:?}"
                );
                let diagnostic = events
                    .iter()
                    .find(|event| event.fields.contains("404 Not Found"))
                    .unwrap_or_else(|| {
                        panic!("{}: missing debug diagnostic: {events:?}", case.module)
                    });
                assert_eq!(diagnostic.level, Level::DEBUG);
                assert!(diagnostic.target.ends_with(case.module));
                for context in [
                    "provider=",
                    "scope=channel:",
                    CHANNEL,
                    case.channel_path,
                    "after ",
                    "ms",
                ] {
                    assert!(diagnostic.fields.contains(context), "{diagnostic:?}");
                }
                assert!(
                    diagnostic
                        .fields
                        .contains(&format!("body_preview={body:?}")),
                    "{diagnostic:?}"
                );

                let cache = provider.get_emote_cache(CHANNEL.to_owned());
                assert!(cache.has_emote("CachedGlobal".to_owned()));
                assert_eq!(cache.has_emote("CachedChannel".to_owned()), cached_channel);
                let persisted =
                    persistence.load_cache(case.id, CHANNEL, &provider.get_name(), &settings);
                assert_eq!(persisted.is_some(), cached_channel);
                if let Some(persisted) = persisted {
                    assert!(persisted.cache.has_emote("CachedChannel".to_owned()));
                    assert_eq!(persisted.saved_at_unix_secs, NOW);
                }
                mock.assert();
                mock.delete();
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn global_not_found_responses_remain_errors() {
    let server = MockServer::start();
    for case in &PROVIDERS {
        let mut mock = server.mock(|when, then| {
            when.method(GET).path(case.global_path);
            then.status(404).body(case.missing_body);
        });
        let provider = case.provider(case.persistence(false), server.base_url());

        let events = capture_events(|| {
            provider.load_global_emotes(&reqwest::Client::new(), &ProviderSettings::default());
        });

        case.assert_error(&events, "404 Not Found");
        assert!(provider
            .get_emote_cache(CHANNEL.to_owned())
            .has_emote("CachedGlobal".to_owned()));
        mock.assert();
        mock.delete();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn other_channel_statuses_remain_errors_even_with_a_missing_user_body() {
    let server = MockServer::start();
    for case in &PROVIDERS {
        for status in [401, 403, 429, 503] {
            let mut mock = server.mock(|when, then| {
                when.method(GET).path(case.channel_path);
                then.status(status).body(case.missing_body);
            });
            let provider = case.provider(case.persistence(true), server.base_url());

            let events = capture_events(|| {
                provider.load_channel_emotes(
                    CHANNEL.to_owned(),
                    &reqwest::Client::new(),
                    &ProviderSettings::default(),
                );
            });

            case.assert_error(&events, &format!("status={status}"));
            assert!(provider
                .get_emote_cache(CHANNEL.to_owned())
                .has_emote("CachedChannel".to_owned()));
            mock.assert();
            mock.delete();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_successful_channel_responses_remain_errors() {
    let server = MockServer::start();
    for case in &PROVIDERS {
        for body in ["not json", "{}"] {
            let mut mock = server.mock(|when, then| {
                when.method(GET).path(case.channel_path);
                then.status(200).body(body);
            });
            let provider = case.provider(case.persistence(false), server.base_url());

            let events = capture_events(|| {
                provider.load_channel_emotes(
                    CHANNEL.to_owned(),
                    &reqwest::Client::new(),
                    &ProviderSettings::default(),
                );
            });

            case.assert_error(&events, "decode failed");
            mock.assert();
            mock.delete();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn channel_request_timeouts_remain_errors() {
    let server = MockServer::start();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .unwrap();
    for case in &PROVIDERS {
        let mut mock = server.mock(|when, then| {
            when.method(GET).path(case.channel_path);
            then.status(404)
                .body(case.missing_body)
                .delay(Duration::from_secs(1));
        });
        let provider = case.provider(case.persistence(false), server.base_url());

        let events = capture_events(|| {
            provider.load_channel_emotes(CHANNEL.to_owned(), &client, &ProviderSettings::default());
        });

        case.assert_error(&events, "request failed");
        mock.assert();
        mock.delete();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn channel_connection_failures_remain_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api_base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    for case in &PROVIDERS {
        let provider = case.provider(case.persistence(false), api_base.clone());
        let events = capture_events(|| {
            provider.load_channel_emotes(CHANNEL.to_owned(), &client, &ProviderSettings::default());
        });

        case.assert_error(&events, "request failed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn successful_channel_loads_replace_channel_cache_and_preserve_global_emotes() {
    let server = MockServer::start();
    let settings = ProviderSettings::default();
    for case in &PROVIDERS {
        for (body, populated) in [(case.empty_body, false), (case.populated_body, true)] {
            let mut mock = server.mock(|when, then| {
                when.method(GET).path(case.channel_path);
                then.status(200).body(body);
            });
            let persistence = case.persistence(true);
            let provider = case.provider(persistence.clone(), server.base_url());

            let events = capture_events(|| {
                provider.load_channel_emotes(
                    CHANNEL.to_owned(),
                    &reqwest::Client::new(),
                    &settings,
                );
            });

            assert!(
                events.iter().all(|event| event.level != Level::ERROR),
                "{events:?}"
            );
            let cache = provider.get_emote_cache(CHANNEL.to_owned());
            assert!(cache.has_emote("CachedGlobal".to_owned()));
            assert!(!cache.has_emote("CachedChannel".to_owned()));
            assert_eq!(cache.has_emote("FreshEmote".to_owned()), populated);
            let persisted = persistence
                .load_cache(case.id, CHANNEL, &provider.get_name(), &settings)
                .unwrap();
            assert_eq!(persisted.cache.len(), usize::from(populated));
            assert_eq!(
                persisted.cache.has_emote("FreshEmote".to_owned()),
                populated
            );
            mock.assert();
            mock.delete();
        }
    }
}

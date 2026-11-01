use super::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use twitch_api::client::{BoxedFuture, Request, Response};

const FOLLOWED_PATH: &str = "/helix/channels/followed";
const USERS_PATH: &str = "/helix/users";
const STREAMS_PATH: &str = "/helix/streams";
const FOLLOWED: &str = r#"{"total":1,"data":[{"broadcaster_id":"1","broadcaster_login":"maya","broadcaster_name":"Maya","followed_at":"2026-01-01T00:00:00Z"}],"pagination":{}}"#;
const EMPTY: &str = r#"{"data":[],"pagination":{}}"#;
const LIVE: &str = r#"{"data":[{"id":"stream-1","user_id":"1","user_login":"maya","user_name":"Maya","game_id":"game","game_name":"Game","type":"live","title":"Recovered","viewer_count":42,"started_at":"2026-01-01T00:00:00Z","language":"en","thumbnail_url":"https://example.test/thumb","tag_ids":[],"tags":[],"is_mature":false}],"pagination":{}}"#;
const SENSITIVE: &str =
    "https://id.twitch.tv/oauth2/token?refresh_token=secret-refresh&access_token=secret-access";

enum Step {
    Response(&'static str, u16, Vec<u8>, Duration),
    Failure(&'static str, std::io::ErrorKind),
    Pending(&'static str, Arc<AtomicBool>),
}

fn reply(path: &'static str, status: u16, body: impl AsRef<[u8]>) -> Step {
    Step::Response(path, status, body.as_ref().to_vec(), Duration::ZERO)
}

fn success() -> Vec<Step> {
    vec![
        reply(FOLLOWED_PATH, 200, FOLLOWED),
        reply(USERS_PATH, 200, EMPTY),
        reply(STREAMS_PATH, 200, LIVE),
    ]
}

struct ScriptedClient {
    steps: StdMutex<VecDeque<Step>>,
    uris: StdMutex<Vec<String>>,
}

impl ScriptedClient {
    fn new(steps: impl IntoIterator<Item = Step>) -> HelixClient<'static, Self> {
        HelixClient::with_client(Self {
            steps: StdMutex::new(steps.into_iter().collect()),
            uris: StdMutex::new(Vec::new()),
        })
    }

    fn assert_finished(&self) {
        assert!(self.steps.lock().unwrap().is_empty(), "unused responses");
    }
}

impl twitch_api::HttpClient for ScriptedClient {
    type Error = std::io::Error;

    fn req(&self, request: Request) -> BoxedFuture<'_, Result<Response, Self::Error>> {
        let step = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected Helix request");
        let path = match &step {
            Step::Response(path, ..) | Step::Failure(path, _) | Step::Pending(path, _) => path,
        };
        assert_eq!(request.uri().path(), *path);
        self.uris.lock().unwrap().push(request.uri().to_string());
        Box::pin(async move {
            match step {
                Step::Response(_, status, body, delay) => {
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    Ok(tauri::http::Response::builder()
                        .status(status)
                        .body(body.into())
                        .unwrap())
                }
                Step::Failure(_, kind) => Err(std::io::Error::new(kind, SENSITIVE)),
                Step::Pending(_, dropped) => {
                    struct Cancelled(Arc<AtomicBool>);
                    impl Drop for Cancelled {
                        fn drop(&mut self) {
                            self.0.store(true, Ordering::SeqCst);
                        }
                    }
                    let _cancelled = Cancelled(dropped);
                    futures::future::pending().await
                }
            }
        })
    }
}

#[derive(Clone)]
struct Published {
    cache: types::ChannelCache,
    followed: Vec<String>,
    updates: usize,
}

impl Default for Published {
    fn default() -> Self {
        let mut cache = types::ChannelCache::default();
        cache.channels.insert(
            "retained".to_owned(),
            types::ChannelStatus {
                broadcaster_id: "old".to_owned(),
                login: "retained".to_owned(),
                display_name: "Retained".to_owned(),
                is_live: true,
                ..Default::default()
            },
        );
        Self {
            cache,
            followed: vec!["retained".to_owned()],
            updates: 0,
        }
    }
}

fn identity() -> token::TokenIdentity {
    token::TokenIdentity {
        user_id: "14012".to_owned(),
        session: 1,
        generation: 1,
    }
}

fn runtime_token() -> twitch_oauth2::UserToken {
    twitch_oauth2::UserToken::from_existing_unchecked(
        "secret-access",
        Some("secret-refresh".into()),
        "client",
        None,
        "viewer".into(),
        identity().user_id.into(),
        Some(vec![]),
        Some(Duration::from_secs(3600)),
    )
}

async fn poll_once(
    client: &HelixClient<'_, ScriptedClient>,
    published: &RefCell<Published>,
    settings: types::ChannelCacheSettings,
) -> Result<(), ChannelCacheFetchError> {
    let previous = published.borrow().cache.clone();
    complete_channel_cache_poll(
        fetch_channel_cache(client, &runtime_token(), settings, &previous, Vec::new()),
        |update| {
            let mut published = published.borrow_mut();
            published.cache = update.cache;
            published.followed = update
                .channels
                .into_iter()
                .map(|channel| channel.broadcaster_login.to_string())
                .collect();
            published.updates += 1;
        },
    )
    .await
}

fn assert_retained(published: &Published) {
    assert_eq!(published.updates, 0);
    assert_eq!(published.followed, ["retained"]);
    assert_eq!(published.cache.channels.len(), 1);
    assert!(published.cache.channels["retained"].is_live);
}

fn no_session() -> ChannelCachePollError {
    ChannelCachePollError {
        failure: ChannelCacheFetchError::session_unavailable(),
        identity: None,
    }
}

#[tokio::test(start_paused = true)]
async fn failures_keep_data_and_recurring_poll_resumes_updates() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let client = ScriptedClient::new(
        [
            reply(FOLLOWED_PATH, 500, "Unauthorized 401"),
            Step::Failure(FOLLOWED_PATH, std::io::ErrorKind::NotConnected),
            Step::Pending(FOLLOWED_PATH, cancelled.clone()),
        ]
        .into_iter()
        .chain(success()),
    );
    let published = RefCell::new(Published::default());
    let attempts = Cell::new(0);
    let errors = RefCell::new(Vec::new());
    let settings = types::ChannelCacheSettings {
        poll_interval_secs: 37,
        ..Default::default()
    };
    let started = tokio::time::Instant::now();
    run_channel_cache_poll_loop(
        || settings,
        || async {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 4 {
                return Err(no_session());
            }
            poll_once(&client, &published, settings)
                .await
                .map_err(|failure| ChannelCachePollError {
                    failure,
                    identity: Some(identity()),
                })
        },
        |context, failure| {
            if failure.identity.is_some() {
                assert_retained(&published.borrow());
            }
            errors.borrow_mut().push(context);
            async move {
                channel_cache_poll_should_stop(
                    &failure,
                    async { panic!("non-auth failure invoked credential recovery") },
                    |_| async { panic!("non-auth failure cleared credentials") },
                )
                .await
            }
        },
    )
    .await;
    assert_eq!(started.elapsed(), Duration::from_secs(178));
    assert_eq!(
        *errors.borrow(),
        ["initial", "recurring", "recurring", "recurring"]
    );
    assert!(cancelled.load(Ordering::SeqCst));
    let published = published.borrow();
    assert_eq!(published.updates, 1);
    assert_eq!(published.followed, ["maya"]);
    assert_eq!(published.cache.channels.len(), 1);
    assert!(published.cache.channels["maya"].is_live);
    assert_eq!(
        published.cache.channels["maya"]
            .stream
            .as_ref()
            .unwrap()
            .title,
        "Recovered"
    );
    assert!(client.get_client().uris.lock().unwrap()[0].contains("user_id=14012"));
    client.get_client().assert_finished();
}

#[tokio::test]
async fn failures_after_partial_fetch_never_publish() {
    let page_one = FOLLOWED.replace(r#""pagination":{}"#, r#""pagination":{"cursor":"next"}"#);
    let cases = [
        (
            "followed_channels",
            vec![
                reply(FOLLOWED_PATH, 200, page_one),
                reply(FOLLOWED_PATH, 500, ""),
            ],
        ),
        (
            "streams",
            vec![
                reply(FOLLOWED_PATH, 200, FOLLOWED),
                reply(USERS_PATH, 200, EMPTY),
                reply(
                    STREAMS_PATH,
                    200,
                    LIVE.replace(r#""pagination":{}"#, r#""pagination":{"cursor":"next"}"#),
                ),
                reply(STREAMS_PATH, 500, ""),
            ],
        ),
    ];
    for (operation, steps) in cases {
        let client = ScriptedClient::new(steps);
        let published = RefCell::new(Published::default());
        let failure = poll_once(&client, &published, Default::default())
            .await
            .unwrap_err();
        assert_eq!(failure.operation, operation);
        assert_eq!(failure.status, Some(500));
        assert_retained(&published.borrow());
        assert!(client
            .get_client()
            .uris
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .contains("after=next"));
        client.get_client().assert_finished();
    }
}

#[tokio::test(start_paused = true)]
async fn deadline_cancels_fetch_at_every_stage_and_allows_success() {
    for path in [FOLLOWED_PATH, USERS_PATH, STREAMS_PATH] {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut steps = Vec::new();
        if path != FOLLOWED_PATH {
            steps.push(reply(FOLLOWED_PATH, 200, FOLLOWED));
        }
        if path == STREAMS_PATH {
            steps.push(reply(USERS_PATH, 200, EMPTY));
        }
        steps.push(Step::Pending(path, cancelled.clone()));
        steps.extend(success());
        let client = ScriptedClient::new(steps);
        let published = RefCell::new(Published::default());
        let started = tokio::time::Instant::now();
        let mut poll = Box::pin(poll_once(&client, &published, Default::default()));
        tokio::select! {
            result = &mut poll => panic!("poll completed before deadline: {result:?}"),
            _ = tokio::time::sleep(Duration::from_secs(29)) => {}
        }
        assert!(!cancelled.load(Ordering::SeqCst));
        assert_retained(&published.borrow());
        let failure = poll.await.unwrap_err();
        assert_eq!(failure.operation, "poll");
        assert_eq!(failure.category, "timeout");
        assert!(!failure.is_auth_rejection());
        assert_eq!(started.elapsed(), CHANNEL_CACHE_POLL_TIMEOUT);
        assert!(cancelled.load(Ordering::SeqCst));
        assert_retained(&published.borrow());
        poll_once(&client, &published, Default::default())
            .await
            .unwrap();
        assert_eq!(published.borrow().updates, 1);
        client.get_client().assert_finished();
    }
}

#[tokio::test(start_paused = true)]
async fn deadline_covers_all_fetches_instead_of_resetting_per_request() {
    let client = ScriptedClient::new([
        Step::Response(
            FOLLOWED_PATH,
            200,
            FOLLOWED.as_bytes().to_vec(),
            Duration::from_secs(20),
        ),
        Step::Response(
            USERS_PATH,
            200,
            EMPTY.as_bytes().to_vec(),
            Duration::from_secs(20),
        ),
    ]);
    let published = RefCell::new(Published::default());
    let started = tokio::time::Instant::now();
    let failure = poll_once(&client, &published, Default::default())
        .await
        .unwrap_err();
    assert_eq!(failure.category, "timeout");
    assert_eq!(started.elapsed(), CHANNEL_CACHE_POLL_TIMEOUT);
    assert_retained(&published.borrow());
    client.get_client().assert_finished();
}

#[tokio::test]
async fn later_user_chunk_failure_keeps_cache_and_membership() {
    let followed = serde_json::from_str::<serde_json::Value>(FOLLOWED).unwrap();
    let mut second = followed["data"][0].clone();
    second["broadcaster_id"] = "2".into();
    second["broadcaster_login"] = "luna".into();
    second["broadcaster_name"] = "Luna".into();
    let body = serde_json::json!({
        "total": 2,
        "data": [followed["data"][0].clone(), second],
        "pagination": {}
    })
    .to_string();
    let client = ScriptedClient::new([
        reply(FOLLOWED_PATH, 200, body),
        reply(USERS_PATH, 200, EMPTY),
        Step::Failure(USERS_PATH, std::io::ErrorKind::ConnectionReset),
    ]);
    let published = RefCell::new(Published::default());
    let settings = types::ChannelCacheSettings {
        user_lookup_chunk_size: 1,
        ..Default::default()
    };
    let failure = poll_once(&client, &published, settings).await.unwrap_err();
    assert_eq!(failure.operation, "users");
    assert_eq!(failure.category, "connect");
    assert_retained(&published.borrow());
    let uris = client.get_client().uris.lock().unwrap();
    assert!(uris[1].contains("id=2"));
    assert!(uris[2].contains("id=1"));
    client.get_client().assert_finished();
}

#[tokio::test]
async fn authentication_classification_uses_http_status_at_every_stage() {
    let body_claims_401 =
        r#"{"error":"Unauthorized","status":401,"message":"Invalid OAuth token secret-refresh"}"#;
    for (status, body, auth) in [
        (500, body_claims_401.as_bytes(), false),
        (403, body_claims_401.as_bytes(), false),
        (200, body_claims_401.as_bytes(), false),
        (401, b"".as_slice(), true),
        (401, b"not JSON secret-access".as_slice(), true),
        (401, b"\xff".as_slice(), true),
        (401, FOLLOWED.as_bytes(), true),
    ] {
        for path in [FOLLOWED_PATH, USERS_PATH, STREAMS_PATH] {
            let mut steps = Vec::new();
            if path != FOLLOWED_PATH {
                steps.push(reply(FOLLOWED_PATH, 200, FOLLOWED));
            }
            if path == STREAMS_PATH {
                steps.push(reply(USERS_PATH, 200, EMPTY));
            }
            steps.push(reply(path, status, body));
            let client = ScriptedClient::new(steps);
            let published = RefCell::new(Published::default());
            let failure = poll_once(&client, &published, Default::default())
                .await
                .unwrap_err();
            assert_eq!(failure.is_auth_rejection(), auth, "HTTP {status} at {path}");
            if status != 200 {
                assert_eq!(failure.status, Some(status));
            }
            assert_retained(&published.borrow());
            client.get_client().assert_finished();
        }
    }
}

#[tokio::test]
async fn authentication_recovery_preserves_existing_stop_and_clear_policy() {
    let terminal = token::AuthFailure {
        identity: token::TokenIdentity {
            generation: 2,
            ..identity()
        },
        issue: token::AuthIssue::permanent("refresh_token_rejected"),
    };
    for (result, expected_stop, expected_clear) in [
        (Ok(()), false, None),
        (
            Err(token::RecoveryError::RetryLater(token::AuthIssue::retry(
                "connect",
            ))),
            false,
            None,
        ),
        (Err(token::RecoveryError::SessionChanged), true, None),
        (
            Err(token::RecoveryError::ReauthenticationRequired(
                terminal.clone(),
            )),
            true,
            Some(terminal),
        ),
    ] {
        let client = ScriptedClient::new([reply(FOLLOWED_PATH, 401, "")]);
        let published = RefCell::new(Published::default());
        let failure = ChannelCachePollError {
            failure: poll_once(&client, &published, Default::default())
                .await
                .unwrap_err(),
            identity: Some(identity()),
        };
        let recovered = Cell::new(false);
        let cleared = RefCell::new(None);
        let stop = channel_cache_poll_should_stop(
            &failure,
            async {
                recovered.set(true);
                result
            },
            |failure| {
                *cleared.borrow_mut() = Some(failure);
                std::future::ready(())
            },
        )
        .await;
        assert!(recovered.get());
        assert_eq!(stop, expected_stop);
        assert_eq!(*cleared.borrow(), expected_clear);
        assert_retained(&published.borrow());
        client.get_client().assert_finished();
    }
}

#[tokio::test(start_paused = true)]
async fn unavailable_session_stops_initial_poll_without_recovery_or_sleep() {
    let attempts = Cell::new(0);
    let started = tokio::time::Instant::now();
    run_channel_cache_poll_loop(
        Default::default,
        || {
            attempts.set(attempts.get() + 1);
            std::future::ready(Err(no_session()))
        },
        |context, failure| {
            assert_eq!(context, "initial");
            async move {
                channel_cache_poll_should_stop(
                    &failure,
                    async { panic!("unavailable session invoked recovery") },
                    |_| async { panic!("unavailable session cleared credentials") },
                )
                .await
            }
        },
    )
    .await;
    assert_eq!(attempts.get(), 1);
    assert_eq!(started.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn disabled_recurring_polling_keeps_initial_poll_and_rechecks_settings() {
    let settings = Cell::new(types::ChannelCacheSettings {
        recurring_poll_enabled: false,
        poll_interval_secs: 10,
        ..Default::default()
    });
    let attempts = Cell::new(0);
    let started = tokio::time::Instant::now();
    let poll_loop = run_channel_cache_poll_loop(
        || settings.get(),
        || {
            attempts.set(attempts.get() + 1);
            std::future::ready(if attempts.get() == 1 {
                Ok(())
            } else {
                Err(no_session())
            })
        },
        |context, failure| {
            assert_eq!(context, "recurring");
            async move {
                channel_cache_poll_should_stop(
                    &failure,
                    async { panic!("unavailable session invoked recovery") },
                    |_| async { panic!("unavailable session cleared credentials") },
                )
                .await
            }
        },
    );
    let reenable = async {
        tokio::time::sleep(Duration::from_secs(25)).await;
        assert_eq!(attempts.get(), 1);
        settings.set(types::ChannelCacheSettings {
            recurring_poll_enabled: true,
            ..settings.get()
        });
    };
    tokio::join!(poll_loop, reenable);
    assert_eq!(attempts.get(), 2);
    assert_eq!(started.elapsed(), Duration::from_secs(30));
}

#[tokio::test]
async fn diagnostics_exclude_credential_bearing_bodies_urls_and_error_chains() {
    for step in [
        reply(FOLLOWED_PATH, 500, SENSITIVE),
        reply(FOLLOWED_PATH, 200, SENSITIVE),
        Step::Failure(FOLLOWED_PATH, std::io::ErrorKind::NotConnected),
        Step::Failure(FOLLOWED_PATH, std::io::ErrorKind::TimedOut),
    ] {
        let client = ScriptedClient::new([step]);
        let published = RefCell::new(Published::default());
        let failure = poll_once(&client, &published, Default::default())
            .await
            .unwrap_err();
        let diagnostics = format!("{failure} {failure:?}");
        for sensitive in [
            "secret-access",
            "secret-refresh",
            "refresh_token",
            "access_token",
            "https://",
            "14012",
        ] {
            assert!(
                !diagnostics.contains(sensitive),
                "unsafe diagnostic: {diagnostics}"
            );
        }
        assert!(diagnostics.contains("followed_channels"));
        assert_retained(&published.borrow());
        client.get_client().assert_finished();
    }
}

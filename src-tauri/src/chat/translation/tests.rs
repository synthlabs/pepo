use super::*;
use crate::chat::{ChatFeedRequest, FeedEvent, FeedScope};
use crate::types::{ChatSettings, ChatTranslationLayout};
use serde_json::json;
use synth_translate::{Language, SkipReason, Translation};
use tokio::sync::{mpsc, oneshot};

fn message(login: &str, id: usize) -> ChannelMessage {
    serde_json::from_value(json!({
        "ts": "2026-11-08T00:00:00Z", "broadcaster_user_id": login,
        "broadcaster_user_login": login, "broadcaster_user_name": login,
        "chatter_user_id": "user", "chatter_user_name": "User", "message_id": format!("{login}-{id}"),
        "text": format!("message {id}"), "fragments": [{"Text":{"index":0,"text":format!("message {id}")}}],
        "message_type":"text", "badges":[], "color":"", "translation":null, "index":0, "channel_sequence":0
    })).unwrap()
}

fn engine(settings: TranslationSettings) -> Arc<ChatEngine> {
    let engine = ChatEngine::new(ChatSettings::default());
    engine.configure_translation(settings);
    engine
}

fn enabled() -> TranslationSettings {
    TranslationSettings {
        enabled: true,
        translate_history: true,
        ..Default::default()
    }
}

fn open(engine: &ChatEngine, owner: &str, login: &str, revision: u64) -> ChatFeedRequest {
    let request = ChatFeedRequest {
        epoch: engine.feed_context(owner).epoch,
        revision,
    };
    assert!(engine.begin_request(owner, &request));
    assert!(engine.attach(owner, &request, login.into(), 0));
    request
}

fn take(engine: &ChatEngine) -> (Job, watch::Receiver<bool>) {
    let mut state = engine.state.lock().unwrap();
    state.refill_translations();
    state.translations.take().expect("expected translation job")
}

fn result(text: &str) -> Result<TranslationOutcome, ServiceError> {
    Ok(TranslationOutcome::Translated(Translation {
        text: text.into(),
        source: Language::RUSSIAN,
        target: Language::ENGLISH,
        model_id: "fake".into(),
        detection: None,
    }))
}

fn skip() -> Result<TranslationOutcome, ServiceError> {
    Ok(TranslationOutcome::Skipped(
        SkipReason::LanguageUndetermined,
    ))
}

#[test]
fn enabled_defaults_translate_new_arrivals_without_retained_history() {
    let engine = engine(TranslationSettings {
        enabled: true,
        ..Default::default()
    });
    engine.insert(message("a", 1));
    open(&engine, "window", "a", 1);
    engine.state.lock().unwrap().refill_translations();
    assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 0);
    engine.insert(message("a", 2));
    let (job, _) = take(&engine);
    assert_eq!(job.message_id, "a-2");
    engine.finish_translation(job.id, result("new arrival"));
    engine.state.lock().unwrap().refill_translations();
    assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 0);
    assert!(engine.history("a")[0].translation.is_none());
}

#[test]
fn only_open_rooms_schedule_unique_accepted_messages_without_delaying_originals() {
    let engine = engine(enabled());
    let feed = engine.subscribe(FeedScope::Channel("a".into()));
    feed.poll();
    open(&engine, "window", "a", 1);
    assert!(crate::internal::ingest_chat_message(
        &engine,
        message("a", 1)
    ));
    assert!(!crate::internal::ingest_chat_message(
        &engine,
        message("a", 1)
    ));
    engine.insert(message("background", 1));
    let frames = feed.poll();
    assert!(
        matches!(&frames[0].event, FeedEvent::Message { message } if message.translation.is_none())
    );
    assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 1);
    let (job, _) = take(&engine);
    assert_eq!(job.login, "a");
    assert_eq!(job.request.detection_policy, DetectionPolicy::ReliableOnly);
    engine.finish_translation(job.id, result("translated"));
    assert!(
        matches!(&feed.poll()[0].event, FeedEvent::Translation { update } if update.translation.translated_text == "translated")
    );
    assert_eq!(
        engine.history("a")[0]
            .translation
            .as_ref()
            .unwrap()
            .target_language,
        "eng"
    );
}

#[test]
fn history_is_optional_and_completed_translations_keep_their_language() {
    for history in [true, false] {
        let mut settings = enabled();
        settings.translate_history = history;
        settings.detection_policy = ChatDetectionPolicy::BestGuess;
        let engine = engine(settings.clone());
        engine.insert(message("a", 1));
        let first = open(&engine, "window", "a", 1);
        engine.state.lock().unwrap().refill_translations();
        assert_eq!(
            engine.state.lock().unwrap().translations.unfinished(),
            usize::from(history)
        );
        if !history {
            engine.insert(message("a", 2));
        }
        let (job, _) = take(&engine);
        assert_eq!(job.request.detection_policy, DetectionPolicy::BestGuess);
        engine.finish_translation(job.id, result("saved"));
        engine.detach("window", &first);
        settings.target_language = "rus".into();
        engine.configure_translation(settings);
        open(&engine, "window", "a", 2);
        engine.state.lock().unwrap().refill_translations();
        assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 0);
        assert!(engine.history("a").iter().any(|m| m
            .translation
            .as_ref()
            .is_some_and(|t| t.target_language == "eng")));
    }
}

#[test]
fn queue_policies_apply_global_bounds_and_prioritize_live_work() {
    for policy in [
        TranslationQueuePolicy::NewestFirst,
        TranslationQueuePolicy::OldestFirst,
    ] {
        let mut settings = enabled();
        settings.queue_capacity = 3;
        settings.max_in_flight = 1;
        settings.queue_policy = policy;
        let engine = engine(settings);
        engine.insert(message("a", 0));
        open(&engine, "one", "a", 1);
        open(&engine, "two", "b", 1);
        let (history, _) = take(&engine);
        engine.insert(message("a", 1));
        engine.insert(message("b", 2));
        engine.insert(message("a", 3));
        assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 3);
        assert!(engine.state.lock().unwrap().translations.take().is_none());
        engine.finish_translation(history.id, skip());
        let (job, _) = take(&engine);
        assert_eq!(
            job.message_id,
            if policy == TranslationQueuePolicy::NewestFirst {
                "a-3"
            } else {
                "a-1"
            }
        );
        engine.finish_translation(job.id, skip());
        let (job, _) = take(&engine);
        assert_eq!(job.message_id, "b-2");
        engine.finish_translation(job.id, skip());
        engine.state.lock().unwrap().refill_translations();
        assert_eq!(engine.state.lock().unwrap().translations.unfinished(), 0);
    }
}

#[test]
fn history_drains_in_order_without_retrying_skips_or_unchanged_failures() {
    for policy in [
        TranslationQueuePolicy::NewestFirst,
        TranslationQueuePolicy::OldestFirst,
    ] {
        let mut settings = enabled();
        settings.queue_policy = policy;
        settings.queue_capacity = 1;
        settings.max_in_flight = 256;
        let engine = engine(settings);
        for id in 0..4 {
            engine.insert(message("a", id));
        }
        open(&engine, "window", "a", 1);
        let mut ids = Vec::new();
        for _ in 0..4 {
            let (job, _) = take(&engine);
            ids.push(job.message_id.clone());
            assert!(engine.state.lock().unwrap().translations.take().is_none());
            engine.finish_translation(job.id, skip());
        }
        assert_eq!(
            ids,
            if policy == TranslationQueuePolicy::NewestFirst {
                vec!["a-3", "a-2", "a-1", "a-0"]
            } else {
                vec!["a-0", "a-1", "a-2", "a-3"]
            }
        );
        for _ in 0..10 {
            engine.state.lock().unwrap().refill_translations();
        }
        assert!(engine.state.lock().unwrap().translations.take().is_none());
    }
}

#[test]
fn recovery_sweep_does_not_readmit_already_submitted_live_failures() {
    let engine = engine(TranslationSettings {
        queue_capacity: 1,
        ..enabled()
    });
    open(&engine, "window", "a", 1);
    engine.insert(message("a", 1));
    {
        let mut state = engine.state.lock().unwrap();
        let cursor = state.cursor;
        state.translations.seed_history(cursor);
    }
    let (job, _) = take(&engine);
    engine.finish_translation(
        job.id,
        Err(ServiceError::new(ErrorCode::InputTooLong, "too long")),
    );
    engine.state.lock().unwrap().refill_translations();
    assert!(engine.state.lock().unwrap().translations.take().is_none());
}

#[test]
fn blank_history_does_not_stop_a_bounded_sweep_before_translatable_messages() {
    for policy in [
        TranslationQueuePolicy::NewestFirst,
        TranslationQueuePolicy::OldestFirst,
    ] {
        let engine = engine(TranslationSettings {
            queue_capacity: 1,
            queue_policy: policy,
            ..enabled()
        });
        for id in 0..20 {
            let mut message = message("a", id);
            if id != 10 {
                message.fragments.clear();
            }
            engine.insert(message);
        }
        open(&engine, "window", "a", 1);
        let (job, _) = take(&engine);
        assert_eq!(job.message_id, "a-10");
        engine.finish_translation(job.id, skip());
        engine.state.lock().unwrap().refill_translations();
        assert!(engine.state.lock().unwrap().translations.take().is_none());
    }
}

#[test]
fn last_lease_and_session_end_cancel_work_and_reject_late_results() {
    let engine = engine(enabled());
    let first = open(&engine, "one", "a", 1);
    let second = open(&engine, "two", "a", 1);
    engine.insert(message("a", 1));
    let (job, cancel) = take(&engine);
    engine.detach("one", &first);
    assert!(!*cancel.borrow());
    engine.detach("two", &second);
    assert!(*cancel.borrow());
    open(&engine, "one", "a", 2);
    engine.finish_translation(job.id, result("stale"));
    assert!(engine.history("a")[0].translation.is_none());
    let (job, cancel) = take(&engine);
    engine.shutdown();
    assert!(*cancel.borrow());
    engine.finish_translation(job.id, result("after logout"));
    assert_eq!(engine.metrics().retained_messages, 0);
}

#[test]
fn processing_changes_cancel_atomically_but_layout_changes_preserve_work() {
    let variants: Vec<Box<dyn Fn(&mut TranslationSettings)>> = vec![
        Box::new(|s| s.enabled = false),
        Box::new(|s| s.target_language = "rus".into()),
        Box::new(|s| s.translate_history = false),
        Box::new(|s| s.detection_policy = ChatDetectionPolicy::BestGuess),
        Box::new(|s| s.queue_policy = TranslationQueuePolicy::OldestFirst),
        Box::new(|s| s.queue_capacity = 2),
        Box::new(|s| s.max_in_flight = 2),
    ];
    for change in variants {
        let mut settings = enabled();
        let engine = engine(settings.clone());
        open(&engine, "window", "a", 1);
        engine.insert(message("a", 1));
        let (job, cancel) = take(&engine);
        settings.layout = ChatTranslationLayout::Connector;
        engine.configure_translation(settings.clone());
        assert!(!*cancel.borrow());
        change(&mut settings);
        engine.configure_translation(settings.clone());
        assert!(*cancel.borrow());
        engine.finish_translation(job.id, result("stale"));
        assert!(engine.history("a")[0].translation.is_none());
        engine.state.lock().unwrap().refill_translations();
        assert_eq!(
            engine.state.lock().unwrap().translations.unfinished(),
            usize::from(settings.enabled && settings.translate_history)
        );
    }
}

#[test]
fn translations_for_evicted_messages_still_reach_a_paused_feed() {
    let engine = engine(enabled());
    engine.configure(ChatSettings {
        message_limit: 1,
        ..Default::default()
    });
    let feed = engine.subscribe(FeedScope::Channel("a".into()));
    feed.poll();
    open(&engine, "window", "a", 1);
    engine.insert(message("a", 1));
    let (job, _) = take(&engine);
    engine.insert(message("a", 2));
    feed.poll();
    engine.finish_translation(job.id, result("late"));
    assert_eq!(engine.history("a").len(), 1);
    assert!(
        matches!(&feed.poll()[0].event, FeedEvent::Translation { update } if update.message_id == "a-1")
    );
}

fn snapshot(phase: ServicePhase) -> ServiceSnapshot {
    ServiceSnapshot {
        revision: "1".into(),
        models_revision: "0".into(),
        phase,
        error: None,
        catalog: Default::default(),
        installation: None,
        recovery: Default::default(),
        workload: None,
    }
}

type FakeResult = oneshot::Sender<Result<TranslationOutcome, ServiceError>>;

fn installed(id: &str, target: &str) -> synth_translate::plugin::dto::InstallationDto {
    use synth_translate::plugin::dto::{
        InstallResultDto, InstallStatusDto, InstallationDto, ModelDto, PairDto,
    };
    let model = ModelDto {
        id: id.into(),
        pair: PairDto {
            source: "rus".into(),
            target: target.into(),
        },
        version: "1".into(),
        architecture: "fake".into(),
        catalog_sha256: "test".into(),
        download_bytes: "0".into(),
        installed_bytes: "0".into(),
    };
    InstallationDto {
        id: id.into(),
        model: model.clone(),
        progress: synth_translate::InstallControl::new().progress().into(),
        finished: true,
        result: Some(InstallResultDto {
            status: InstallStatusDto::Installed,
            model,
            cleanup_warnings: vec![],
        }),
        error: None,
    }
}

#[test]
fn recovery_ignores_catalog_ticks_and_failed_or_unrelated_installs() {
    let mut recovery = Recovery::default();
    let mut status = snapshot(ServicePhase::Ready);
    assert!(recovery.observe(&status, "eng"));
    assert!(!recovery.observe(&status, "eng"));
    status.models_revision = "1".into();
    assert!(!recovery.observe(&status, "eng"));
    status.installation = Some(installed("other-target", "rus"));
    assert!(!recovery.observe(&status, "eng"));
    let mut failed = installed("failed", "eng");
    failed.result = None;
    failed.error = Some(ServiceError::new(ErrorCode::Transport, "offline"));
    status.installation = Some(failed);
    assert!(!recovery.observe(&status, "eng"));
    status.installation = Some(installed("new-model", "eng"));
    assert!(recovery.observe(&status, "eng"));
    assert!(!recovery.observe(&status, "eng"));
}

#[tokio::test]
async fn installed_models_retry_retained_work_once_without_retranslating_successes() {
    let engine = engine(enabled());
    open(&engine, "window", "a", 1);
    engine.insert(message("a", 1));
    let (snapshots, rx) = watch::channel(snapshot(ServicePhase::Ready));
    let (calls, mut received) = mpsc::unbounded_channel::<(TranslationRequest, FakeResult)>();
    let worker = tokio::spawn(run_with(engine.clone(), rx, move |request| {
        let (sender, result) = oneshot::channel();
        calls.send((request, sender)).unwrap();
        async move { result.await.unwrap() }
    }));
    let (_, answer) = tokio::time::timeout(Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    answer
        .send(Ok(TranslationOutcome::Skipped(
            SkipReason::ModelNotInstalled(synth_translate::LanguagePair {
                source: Language::RUSSIAN,
                target: Language::ENGLISH,
            }),
        )))
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    let mut status = snapshot(ServicePhase::Ready);
    status.installation = Some(installed("installed", "eng"));
    snapshots.send_replace(status.clone());
    let (_, answer) = tokio::time::timeout(Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    answer.send(result("model is now installed")).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    status.installation = Some(installed("repaired", "eng"));
    snapshots.send_replace(status);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    assert!(engine.history("a")[0].translation.is_some());
    engine.shutdown();
    worker.await.unwrap();
}

#[test]
fn quiet_outcomes_and_oversized_inputs_do_not_raise_repeated_technical_alerts() {
    if crate::logging::run_log_test_in_child(
        "chat::translation::tests::quiet_outcomes_and_oversized_inputs_do_not_raise_repeated_technical_alerts",
    ) { return; }
    #[derive(Clone)]
    struct Output(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let output = Arc::new(std::sync::Mutex::new(Vec::new()));
    let capture = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || Output(capture.clone()))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        for code in [
            ErrorCode::Cancelled,
            ErrorCode::Capacity,
            ErrorCode::Evicted,
            ErrorCode::NotReady,
            ErrorCode::Paused,
            ErrorCode::Shutdown,
            ErrorCode::UnsupportedPair,
            ErrorCode::NotInstalled,
            ErrorCode::InputTooLong,
        ] {
            report_error(&ServiceError::new(code, "expected skip"));
        }
        for _ in 0..10 {
            report_error(&ServiceError::new(
                ErrorCode::Inference,
                "fake inference failed",
            ));
        }
    });
    let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert_eq!(output.matches("ERROR").count(), 1);
    assert!(output.contains("fake inference failed"));
    assert!(output.contains("exceeds model limit"));
    assert!(!output.contains("expected skip"));
}

#[tokio::test]
async fn native_runner_waits_for_readiness_recovers_and_cancels_inference() {
    let engine = engine(enabled());
    let request = open(&engine, "window", "a", 1);
    engine.insert(message("a", 1));
    let (snapshots, rx) = watch::channel(snapshot(ServicePhase::Initializing));
    let (calls, mut received) = mpsc::unbounded_channel::<(TranslationRequest, FakeResult)>();
    let worker = tokio::spawn(run_with(engine.clone(), rx, move |request| {
        let (sender, result) = oneshot::channel();
        calls.send((request, sender)).unwrap();
        async move { result.await.unwrap() }
    }));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    let mut offline = snapshot(ServicePhase::Ready);
    offline.catalog.error = Some(ServiceError::new(ErrorCode::CatalogHttp, "offline"));
    snapshots.send_replace(offline);
    let (request_body, answer) = tokio::time::timeout(Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request_body.text, "message 1");
    answer.send(skip()).unwrap();
    for _ in 0..5 {
        engine.wake();
        tokio::task::yield_now().await;
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    let mut catalog_only = snapshot(ServicePhase::Ready);
    catalog_only.models_revision = "10".into();
    snapshots.send_replace(catalog_only);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), received.recv())
            .await
            .is_err()
    );
    snapshots.send_replace(snapshot(ServicePhase::Paused));
    tokio::task::yield_now().await;
    snapshots.send_replace(snapshot(ServicePhase::Ready));
    let (_, mut answer) = tokio::time::timeout(Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    engine.detach("window", &request);
    tokio::time::timeout(Duration::from_secs(2), answer.closed())
        .await
        .unwrap();
    engine.shutdown();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn old_settings_receive_defaults_and_limits_round_trip_without_a_schema_bump() {
    let settings: crate::types::AppSettings = serde_json::from_value(json!({
        "schema_version":1, "translation":{"enabled":true,"target_language":"rus","layout":"connector"}
    })).unwrap();
    assert!(!settings.translation.translate_history);
    assert_eq!(settings.translation.queue_capacity, 256);
    assert_eq!(settings.translation.max_in_flight, 16);
    let settings = TranslationSettings {
        queue_capacity: 0,
        max_in_flight: 999,
        ..enabled()
    }
    .normalized();
    assert_eq!(settings.queue_capacity, 1);
    assert_eq!(settings.max_in_flight, 256);
    let saved = serde_json::to_string(&settings).unwrap();
    let restored: TranslationSettings = serde_json::from_str(&saved).unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), saved);
}

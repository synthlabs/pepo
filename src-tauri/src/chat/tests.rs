use super::*;
use serde_json::json;
use std::collections::HashSet;
use std::thread;

fn request(engine: &ChatEngine, revision: u64) -> ChatFeedRequest {
    ChatFeedRequest {
        epoch: engine.feed_context("window").epoch,
        revision,
    }
}

fn attach(engine: &ChatEngine, request: &ChatFeedRequest, login: &str, feed_id: u64) -> bool {
    engine.begin_request("window", request)
        && engine.attach("window", request, login.into(), feed_id)
}

fn message(login: &str, id: usize) -> ChannelMessage {
    serde_json::from_value(json!({
        "ts": "2026-10-25T00:00:00Z", "broadcaster_user_id": login, "broadcaster_user_name": login,
        "broadcaster_user_login": login, "chatter_user_id": "user", "chatter_user_name": "User",
        "message_id": format!("{login}-{id}"), "text": format!("message {id}"),
        "fragments": [{"Text": {"index": 0, "text": format!("message {id}")}}],
        "message_type": "text", "badges": [], "color": "#ffffff", "translation": null, "index": id, "channel_sequence": 0
    }))
    .unwrap()
}
fn engine(limit: usize) -> Arc<ChatEngine> {
    ChatEngine::new(ChatSettings {
        message_limit: limit,
        ..Default::default()
    })
}

#[test]
fn global_gap_excludes_delivered_frames_waiting_for_acknowledgement() {
    let e = engine(10);
    let feed = e.subscribe(FeedScope::All);
    for id in 0..DELIVERY_WINDOW {
        e.insert(message("a", id));
    }
    let delivered = feed.poll();
    feed.acknowledge(delivered[63].cursor);
    for id in DELIVERY_WINDOW..DELIVERY_WINDOW + FEED_CAPACITY + 1 {
        e.insert(message("a", id));
    }
    let recovery = feed.poll();
    assert!(matches!(
        recovery[0].event,
        FeedEvent::Gap {
            after: 128,
            through: 1153
        }
    ));
}

#[test]
fn failure_replays_per_feed_and_deduplicates_until_recovery() {
    let e = engine(2);
    let existing = e.subscribe(FeedScope::Channel("a".into()));
    existing.poll();
    let now = Instant::now();
    e.state
        .lock()
        .unwrap()
        .fail_room("a", "403 Forbidden".into(), now);
    assert_eq!(existing.poll().len(), 1);
    e.state
        .lock()
        .unwrap()
        .fail_room("a", "403 Forbidden".into(), now);
    assert!(existing.poll().is_empty());
    let reopened = e.subscribe(FeedScope::Channel("a".into()));
    let frames = reopened.poll();
    assert_eq!(frames.len(), 2);
    assert!(matches!(frames[1].event, FeedEvent::Unavailable { .. }));
    e.state.lock().unwrap().failures.remove("a");
    e.state
        .lock()
        .unwrap()
        .fail_room("a", "403 Forbidden".into(), now);
    assert_eq!(existing.poll().len(), 1);
    assert_eq!(e.state.lock().unwrap().failures["a"].attempts, 1);
}

#[test]
fn explicit_reopening_expedites_retry_without_resetting_failed_attempts() {
    let e = engine(2);
    let now = Instant::now();
    e.state
        .lock()
        .unwrap()
        .fail_room("a", "403 Forbidden".into(), now);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    let r = request(&e, 1);
    assert!(attach(&e, &r, "a", feed.id));
    let state = e.state.lock().unwrap();
    assert!(state.failures["a"].retry_at < now + std::time::Duration::from_secs(5));
    assert_eq!(state.failures["a"].attempts, 1);
}

#[test]
fn owner_tracking_is_bounded_and_stale_requests_cannot_reactivate() {
    let e = engine(2);
    let first = request(&e, 1);
    for revision in 1..=2000 {
        let r = request(&e, revision);
        let feed = e.subscribe(FeedScope::Channel("a".into()));
        assert!(attach(&e, &r, "a", feed.id));
        e.detach("window", &r);
        e.detach("window", &r);
    }
    assert!(!e.begin_request("window", &first));
    let state = e.state.lock().unwrap();
    assert_eq!(state.owners.len(), 1);
    assert!(state.leases.is_empty());
    assert!(state.subscribers.is_empty());
}

#[test]
fn renderer_and_session_epochs_reject_late_commands() {
    let e = engine(2);
    let old = request(&e, 500);
    e.close_owner("window");
    let current = request(&e, 1);
    assert_ne!(old.epoch, current.epoch);
    assert!(!e.begin_request("window", &old));
    assert!(e.begin_request("window", &current));
    e.detach("window", &old);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    assert!(e.attach("window", &current, "a".into(), feed.id));
    e.shutdown();
    assert!(!e.begin_request("window", &current));
    let next_session = engine(2);
    assert_ne!(next_session.feed_context("window").epoch, current.epoch);
    assert!(!next_session.begin_request("window", &current));
}

#[test]
fn acknowledgements_and_cancellation_are_bound_to_the_issuing_window() {
    let e = engine(2);
    let r = request(&e, 1);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    assert!(attach(&e, &r, "a", feed.id));
    feed.poll();
    e.feed_context("other");
    e.acknowledge("other", &r, u64::MAX);
    e.detach("other", &r);
    assert!(e.request_active("window", &r));
    assert_eq!(
        e.state.lock().unwrap().subscribers[&feed.id]
            .in_flight
            .len(),
        1
    );
}

#[tokio::test]
async fn failed_resolution_returns_one_error_without_a_lease_or_subscriber() {
    let e = engine(2);
    let r = request(&e, 1);
    let result = runtime::prepare_feed(&e, "window", &r, "unknown".into(), async {
        Err("channel not found".into())
    })
    .await;
    assert!(matches!(result, Err(error) if error == "channel not found"));
    let state = e.state.lock().unwrap();
    assert!(state.leases.is_empty());
    assert!(state.subscribers.is_empty());
    assert!(state.membership.foreground.is_empty());
    assert!(state.membership.lingering.is_empty());
    assert!(!state.owners["window"].pending);
}

#[tokio::test]
async fn cancellation_while_resolution_is_pending_cannot_create_a_feed() {
    let e = engine(2);
    let r = request(&e, 1);
    let resolve = async {
        e.detach("window", &r);
        Ok(ChatRoomInfo::default())
    };
    let result = runtime::prepare_feed(&e, "window", &r, "a".into(), resolve).await;
    assert!(matches!(result, Err(error) if error == "subscription canceled"));
    let state = e.state.lock().unwrap();
    assert!(state.leases.is_empty());
    assert!(state.subscribers.is_empty());
}

#[test]
fn closed_unfollowed_rooms_keep_retry_intent_and_back_off_without_reserving_capacity() {
    let e = ChatEngine::new(ChatSettings {
        join_mode: crate::types::ChatJoinMode::Single,
        ..Default::default()
    });
    let now = Instant::now();
    {
        let mut state = e.state.lock().unwrap();
        state.membership.lingering.insert("visitor".into());
        state.fail_room("visitor", "403 Forbidden".into(), now);
    }
    let manager = crate::eventsub::EventSubManager::new();
    let mut coordinator = admission::AdmissionCoordinator::default();
    assert_eq!(
        runtime::admission_action(&e, &manager, &mut coordinator, now),
        (None, 0, 0)
    );
    let retry = now + std::time::Duration::from_secs(5);
    assert_eq!(
        runtime::admission_action(&e, &manager, &mut coordinator, retry).0,
        Some(admission::AdmissionAction::Join("visitor".into()))
    );
    e.state
        .lock()
        .unwrap()
        .fail_room("visitor", "403 Forbidden".into(), retry);
    assert_eq!(
        runtime::admission_action(
            &e,
            &manager,
            &mut coordinator,
            retry + std::time::Duration::from_secs(9)
        )
        .0,
        None
    );
    assert!(e
        .state
        .lock()
        .unwrap()
        .membership
        .lingering
        .contains("visitor"));
    assert_eq!(
        runtime::admission_action(
            &e,
            &manager,
            &mut coordinator,
            retry + std::time::Duration::from_secs(10)
        )
        .0,
        Some(admission::AdmissionAction::Join("visitor".into()))
    );
}

#[tokio::test]
async fn late_metadata_cannot_replace_a_newer_room_feed() {
    let e = engine(2);
    let old = request(&e, 1);
    let newer = request(&e, 2);
    let (complete, resolution) = tokio::sync::oneshot::channel();
    let mut opening = Box::pin(runtime::prepare_feed(
        &e,
        "window",
        &old,
        "a".into(),
        async { resolution.await.unwrap() },
    ));
    assert!(futures::poll!(opening.as_mut()).is_pending());
    let info = ChatRoomInfo {
        broadcaster_id: "b-id".into(),
        broadcaster_login: "b".into(),
        broadcaster_name: "B".into(),
    };
    let (identity, _feed) =
        runtime::prepare_feed(&e, "window", &newer, "b".into(), async { Ok(info) })
            .await
            .unwrap();
    complete.send(Ok(ChatRoomInfo::default())).unwrap();
    assert!(matches!(opening.await, Err(error) if error == "subscription canceled"));
    assert!(e.request_active("window", &newer));
    assert_eq!(e.state.lock().unwrap().subscribers.len(), 1);
    assert_eq!(
        serde_json::to_value(identity).unwrap(),
        json!({"broadcaster_id":"b-id", "broadcaster_login":"b", "broadcaster_name":"B"})
    );
}

#[test]
fn retention_is_per_room_and_survives_ui_detach() {
    let e = engine(2);
    for id in 0..4 {
        e.insert(message("a", id));
        e.insert(message("b", id));
    }
    assert_eq!(e.history("a").len(), 2);
    assert_eq!(e.history("b").len(), 2);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    let r = request(&e, 1);
    assert!(attach(&e, &r, "a", feed.id));
    e.detach("window", &r);
    assert_eq!(e.history("a")[0].message_id, "a-2");
    assert!(e.state.lock().unwrap().membership.lingering.contains("a"));
}
#[test]
fn duplicate_messages_do_not_publish_twice() {
    let e = engine(500);
    let feed = e.subscribe(FeedScope::All);
    assert!(e.insert(message("a", 1)));
    assert!(!e.insert(message("a", 1)));
    assert_eq!(feed.poll().len(), 1);
}
#[test]
fn channel_feed_filters_and_starts_with_snapshot() {
    let e = engine(500);
    e.insert(message("a", 1));
    e.insert(message("b", 1));
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    e.insert(message("b", 2));
    e.insert(message("a", 2));
    let frames = feed.poll();
    assert_eq!(frames.len(), 2);
    assert!(
        matches!(&frames[0].event, FeedEvent::Snapshot { messages } if messages.len() == 1 && messages[0].channel_sequence == 0)
    );
    let FeedEvent::Message { message } = &frames[1].event else {
        panic!("expected live message");
    };
    assert_eq!(message.message_id, "a-2");
    assert_eq!(message.channel_sequence, 1);
    assert_eq!(
        serde_json::to_value(message).unwrap(),
        serde_json::to_value(&e.history("a")[1]).unwrap()
    );
}
#[test]
fn global_feed_is_live_only() {
    let e = engine(500);
    e.insert(message("a", 1));
    let feed = e.subscribe(FeedScope::All);
    assert!(feed.poll().is_empty());
    e.insert(message("b", 2));
    assert_eq!(feed.poll().len(), 1);
}
#[test]
fn concurrent_snapshot_and_arrivals_are_delivered_exactly_once() {
    let e = engine(2000);
    let writer = e.clone();
    let task = thread::spawn(move || {
        for id in 0..1000 {
            writer.insert(message("a", id));
        }
    });
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    task.join().unwrap();
    let mut ids = HashSet::new();
    loop {
        let frames = feed.poll();
        if frames.is_empty() {
            break;
        }
        for frame in frames {
            match &frame.event {
                FeedEvent::Snapshot { messages } => {
                    for m in messages {
                        assert!(ids.insert(m.message_id.clone()));
                    }
                }
                FeedEvent::Message { message } => assert!(ids.insert(message.message_id.clone())),
                _ => panic!("unexpected recovery"),
            }
            feed.acknowledge(frame.cursor);
        }
    }
    assert_eq!(ids.len(), 1000);
}
#[test]
fn ipc_window_stops_delivery_until_acknowledged() {
    let e = engine(500);
    let feed = e.subscribe(FeedScope::All);
    for id in 0..300 {
        e.insert(message("a", id));
    }
    let frames = feed.poll();
    assert_eq!(frames.len(), DELIVERY_WINDOW);
    assert!(feed.poll().is_empty());
    feed.acknowledge(frames[63].cursor);
    assert_eq!(feed.poll().len(), 64);
}
#[test]
fn lag_resets_room_to_retained_history() {
    let e = engine(20);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    for id in 0..2000 {
        e.insert(message("a", id));
    }
    let frames = feed.poll();
    assert_eq!(frames.len(), 1);
    let FeedEvent::Reset { messages } = &frames[0].event else {
        panic!("expected recovery snapshot");
    };
    assert_eq!(messages.len(), 20);
    assert_eq!(messages[0].message_id, "a-1980");
    assert_eq!(
        messages
            .iter()
            .map(|m| m.channel_sequence)
            .collect::<Vec<_>>(),
        (1980..2000).collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::to_value(messages).unwrap(),
        serde_json::to_value(e.history("a")).unwrap()
    );
}
#[test]
fn global_lag_is_explicit_and_does_not_block_other_subscribers() {
    let e = engine(20);
    let slow = e.subscribe(FeedScope::All);
    let fast = e.subscribe(FeedScope::Channel("a".into()));
    for id in 0..2000 {
        e.insert(message("a", id));
        for frame in fast.poll() {
            fast.acknowledge(frame.cursor);
        }
    }
    assert!(matches!(
        &slow.poll()[0].event,
        FeedEvent::Gap {
            after: 0,
            through: 2000
        }
    ));
    assert_eq!(e.metrics().retained_messages, 20);
}
#[test]
fn immediate_limit_changes_preserve_other_settings() {
    let e = engine(5);
    for id in 0..10 {
        e.insert(message("a", id));
    }
    let mut settings = ChatSettings {
        message_limit: 2,
        ..Default::default()
    };
    e.configure(settings.clone());
    assert_eq!(e.history("a").len(), 2);
    settings.message_limit = 10;
    e.configure(settings);
    assert_eq!(e.history("a").len(), 2);
    e.insert(message("a", 11));
    assert_eq!(e.history("a").len(), 3);
}
#[test]
fn translations_update_history_and_memory_metrics() {
    let e = engine(2);
    e.insert(message("a", 1));
    let before = e.metrics().estimated_history_bytes;
    let update = ChannelMessageTranslationUpdate {
        message_id: "a-1".into(),
        translation: crate::types::ChannelMessageTranslation {
            source_language: "ru".into(),
            target_language: "en".into(),
            translated_text: "translated text".into(),
        },
    };
    e.translate("a", update);
    assert!(e.history("a")[0].translation.is_some());
    assert!(e.metrics().estimated_history_bytes > before);
    assert_eq!(e.metrics().retained_messages, 1);
    e.shutdown();
    assert_eq!(e.metrics().retained_messages, 0);
    assert_eq!(e.metrics().estimated_history_bytes, 0);
}
#[test]
fn canceled_pending_subscription_cannot_reactivate() {
    let e = engine(2);
    let r = request(&e, 1);
    e.detach("window", &r);
    let feed = e.subscribe(FeedScope::Channel("a".into()));
    assert!(!attach(&e, &r, "a", feed.id));
}
#[test]
fn newer_room_replaces_foreground_without_stale_cleanup_removing_it() {
    let e = engine(2);
    let a = e.subscribe(FeedScope::Channel("a".into()));
    let first = request(&e, 1);
    attach(&e, &first, "a", a.id);
    let b = e.subscribe(FeedScope::Channel("b".into()));
    let second = request(&e, 2);
    attach(&e, &second, "b", b.id);
    e.detach("window", &first);
    assert_eq!(
        e.state.lock().unwrap().membership.foreground,
        HashSet::from(["b".into()])
    );
    e.close_owner("window");
    assert!(e.state.lock().unwrap().membership.foreground.is_empty());
}
#[test]
fn shutdown_rejects_late_ingestion_and_clears_subscriptions() {
    let e = engine(2);
    let feed = e.subscribe(FeedScope::All);
    e.shutdown();
    assert!(!e.insert(message("a", 1)));
    assert!(feed.poll().is_empty());
}
#[test]
fn hundred_room_load_remains_bounded_with_stalled_consumer() {
    let e = engine(50);
    let stalled = e.subscribe(FeedScope::All);
    for id in 0..200 {
        for room in 0..100 {
            e.insert(message(&format!("room{room}"), id));
        }
    }
    let metrics = e.metrics();
    assert_eq!(metrics.channels_with_history, 100);
    assert_eq!(metrics.retained_messages, 5000);
    assert!(e
        .state
        .lock()
        .unwrap()
        .subscribers
        .values()
        .all(|s| s.queue.len() <= FEED_CAPACITY));
    assert!(matches!(&stalled.poll()[0].event, FeedEvent::Gap { .. }));
}

#[derive(Clone)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn diagnostics_logs_every_minute_and_stops_on_logout() {
    use tracing::instrument::WithSubscriber;
    let e = engine(2);
    e.insert(message("a", 1));
    let logs = Arc::new(Mutex::new(Vec::new()));
    let writer = LogWriter(logs.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    let task = tokio::spawn(runtime::run_diagnostics(e.clone()).with_subscriber(subscriber));
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert!(logs.lock().unwrap().is_empty());
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let text = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert_eq!(text.matches("chat store summary").count(), 1);
    assert!(text.contains("retained_messages=1"));
    assert!(text.contains("channels_with_history=1"));
    assert!(text.contains("estimated_history_bytes="));
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        String::from_utf8(logs.lock().unwrap().clone())
            .unwrap()
            .matches("chat store summary")
            .count(),
        2
    );
    e.shutdown();
    task.await.unwrap();
    tokio::time::advance(std::time::Duration::from_secs(600)).await;
    assert_eq!(
        String::from_utf8(logs.lock().unwrap().clone())
            .unwrap()
            .matches("chat store summary")
            .count(),
        2
    );
}

#[tokio::test(start_paused = true)]
async fn diagnostics_obeys_debug_level_filtering() {
    use tracing::instrument::WithSubscriber;
    let e = engine(2);
    let logs = Arc::new(Mutex::new(Vec::new()));
    let writer = LogWriter(logs.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    let task = tokio::spawn(runtime::run_diagnostics(e.clone()).with_subscriber(subscriber));
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    tokio::task::yield_now().await;
    e.shutdown();
    task.await.unwrap();
    assert!(logs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn subscriptions_to_an_ended_session_are_immediately_closed() {
    let e = engine(2);
    e.shutdown();
    let mut feed = e.subscribe(FeedScope::Channel("a".into()));
    assert!(feed.poll().is_empty());
    assert!(!feed.changed().await);
}

#[test]
fn channel_sequences_ignore_other_rooms_translations_errors_and_duplicates() {
    let e = engine(10);
    let feed = e.subscribe(FeedScope::All);
    assert!(e.insert(message("a", 0)));
    assert!(e.insert(message("b", 0)));
    assert!(!e.insert(message("a", 0)));
    e.translate(
        "a",
        ChannelMessageTranslationUpdate {
            message_id: "a-0".into(),
            translation: crate::types::ChannelMessageTranslation {
                source_language: "ru".into(),
                target_language: "en".into(),
                translated_text: "translated".into(),
            },
        },
    );
    e.state
        .lock()
        .unwrap()
        .unavailable("b", "room unavailable".into());
    assert!(e.insert(message("a", 1)));
    assert!(e.insert(message("b", 1)));
    assert!(e.insert(message("a", 2)));
    assert_eq!(
        e.history("a")
            .iter()
            .map(|m| m.channel_sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        e.history("b")
            .iter()
            .map(|m| m.channel_sequence)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let a = e.history("a");
    assert_eq!(a[0].index, 1);
    assert!(a[1].index > a[0].index + 1);
    for frame in feed.poll() {
        if let FeedEvent::Message { message } = &frame.event {
            let stored = e
                .history(&message.broadcaster_user_login)
                .into_iter()
                .find(|m| m.message_id == message.message_id)
                .unwrap();
            assert_eq!(message.channel_sequence, stored.channel_sequence);
            assert_eq!(message.index, frame.cursor);
        }
    }
}

#[test]
fn repository_returns_canonical_sequence_keyed_by_broadcaster_and_resets_on_clear() {
    let mut repository = MemoryRepository::new(1);
    let mut original = message("a", 0);
    original.channel_sequence = 999;
    let accepted = repository.insert(original.clone()).unwrap();
    assert_eq!(accepted.channel_sequence, 0);
    assert_eq!(
        repository.history("a")[0].channel_sequence,
        accepted.channel_sequence
    );
    assert!(repository.insert(original).is_none());
    let mut renamed = message("a", 1);
    renamed.broadcaster_user_login = "renamed".into();
    let accepted = repository.insert(renamed).unwrap();
    assert_eq!(accepted.channel_sequence, 1);
    assert_eq!(repository.history("renamed")[0].channel_sequence, 1);
    assert_eq!(repository.history("a")[0].channel_sequence, 1);
    repository.clear();
    assert_eq!(
        repository.insert(message("a", 2)).unwrap().channel_sequence,
        0
    );
}

#[test]
fn channel_sequences_survive_odd_even_trims_limit_changes_and_room_reopening() {
    for limit in [1, 2, 3] {
        let e = engine(limit);
        for id in 0..7 {
            e.insert(message("a", id));
            e.insert(message("b", id));
        }
        let expected: Vec<_> = ((7 - limit) as u64..7).collect();
        assert_eq!(
            e.history("a")
                .iter()
                .map(|m| m.channel_sequence)
                .collect::<Vec<_>>(),
            expected
        );
        let first = e.subscribe(FeedScope::Channel("a".into()));
        let r = request(&e, 1);
        assert!(attach(&e, &r, "a", first.id));
        e.detach("window", &r);
        let reopened = e.subscribe(FeedScope::Channel("a".into()));
        let frames = reopened.poll();
        let FeedEvent::Snapshot { messages } = &frames[0].event else {
            panic!("expected opening history");
        };
        assert_eq!(
            messages
                .iter()
                .map(|m| m.channel_sequence)
                .collect::<Vec<_>>(),
            expected
        );
        e.configure(ChatSettings {
            message_limit: 1,
            ..Default::default()
        });
        assert_eq!(e.history("a")[0].channel_sequence, 6);
        e.configure(ChatSettings {
            message_limit: 4,
            ..Default::default()
        });
        e.insert(message("a", 7));
        assert_eq!(
            e.history("a")
                .iter()
                .map(|m| m.channel_sequence)
                .collect::<Vec<_>>(),
            vec![6, 7]
        );
    }
}

#[test]
fn subscription_eviction_preserves_channel_sequence_on_rejoin() {
    let e = engine(2);
    e.insert(message("visitor", 0));
    e.insert(message("visitor", 1));
    let feed = e.subscribe(FeedScope::Channel("visitor".into()));
    let first = request(&e, 1);
    attach(&e, &first, "visitor", feed.id);
    e.detach("window", &first);
    {
        let mut state = e.state.lock().unwrap();
        for i in 0..membership::MAX_ROOMS {
            state.membership.followed.insert(format!("live{i}"), true);
        }
        let selected: Vec<_> = state
            .membership
            .ranked(crate::types::ChatJoinMode::Live)
            .into_iter()
            .take(membership::STEADY_ROOMS)
            .collect();
        assert!(!selected.contains(&"visitor".to_owned()));
        state.membership.displaced("visitor");
        assert!(!state.membership.lingering.contains("visitor"));
    }
    let feed = e.subscribe(FeedScope::Channel("visitor".into()));
    let rejoined = request(&e, 2);
    assert!(attach(&e, &rejoined, "visitor", feed.id));
    let frames = feed.poll();
    let FeedEvent::Snapshot { messages } = &frames[0].event else {
        panic!("expected retained history");
    };
    assert_eq!(
        messages
            .iter()
            .map(|m| m.channel_sequence)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    e.insert(message("visitor", 2));
    assert_eq!(
        e.history("visitor")
            .iter()
            .map(|m| m.channel_sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

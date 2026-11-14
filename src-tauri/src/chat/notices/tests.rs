use super::*;
use crate::chat::{ChatEngine, ChatFeedRequest, FeedEvent, FeedScope};
use crate::types::ChatSettings;
use std::sync::Arc;

fn room() -> ChatRoomInfo {
    ChatRoomInfo {
        broadcaster_id: "123".into(),
        broadcaster_login: "room".into(),
        broadcaster_name: "Room".into(),
    }
}

fn subscription() -> NoticeSubscription {
    NoticeSubscription {
        room: room(),
        generation: 7,
    }
}

fn fixture(tags: &str, target: &str) -> String {
    format!("@room-id=123;tmi-sent-ts=1794691390000{tags} :tmi.twitch.tv CLEARCHAT #room{target}")
}

fn parse(raw: &str) -> Result<NoticeEvent, ()> {
    irc::parse_clear(&irc::Line::parse(raw).ok_or(())?, &subscription())
}

fn text(input: &SystemMessageInput) -> String {
    input
        .segments
        .iter()
        .map(|segment| match segment {
            SystemMessageSegment::User { display_name, .. } => display_name.as_str(),
            SystemMessageSegment::Text { text } => text.as_str(),
            _ => panic!("unexpected segment"),
        })
        .collect()
}

#[test]
fn timeout_ban_and_clear_have_exact_text_icons_and_identities() {
    for (raw, expected, icon) in [
        (
            fixture(";target-user-id=456;ban-duration=90", " :viewer"),
            "viewer was timed out for 1m 30s.",
            "clock",
        ),
        (
            fixture(";target-user-id=456", " :viewer").replace("1794691390000", "1794691503000"),
            "viewer was banned.",
            "shield",
        ),
        (fixture("", ""), "Chat was cleared.", "shield"),
    ] {
        let event = parse(&raw).unwrap();
        assert_eq!(event.channel_id, "123");
        assert_eq!(event.generation, 7);
        let input = format_notice(event);
        input.validate().unwrap();
        assert_eq!(text(&input), expected);
        assert_eq!(serde_json::to_value(input.icon).unwrap(), icon);
        if expected.starts_with("viewer") {
            assert!(
                matches!(&input.segments[0], SystemMessageSegment::User { user_id, login, display_name } if user_id == "456" && login == "viewer" && display_name == "viewer")
            );
        }
    }
    assert_eq!(
        parse(&fixture("", "")).unwrap().timestamp,
        "2026-11-14T21:23:10Z"
    );
    assert_eq!(format_duration(90061), "1d 1h 1m 1s");
    assert_eq!(format_duration(86400), "1d");
}

#[test]
fn malformed_notices_never_turn_into_bans_or_room_clears() {
    for duration in [
        "",
        "-1",
        "+90",
        "1.5",
        "invalid",
        "0",
        "18446744073709551616",
    ] {
        assert!(parse(&fixture(
            &format!(";target-user-id=456;ban-duration={duration}"),
            " :viewer"
        ))
        .is_err());
    }
    for raw in [
        fixture("", " :viewer"),
        fixture(";target-user-id=", " :viewer"),
        fixture(";target-user-id=456", ""),
        fixture(";ban-duration=90", ""),
        fixture(";target-user-id=456", " :bad user"),
        fixture(";target-user-id=456;ban-duration", " :viewer"),
        fixture(
            ";target-user-id=456;ban-duration=90;ban-duration=broken",
            " :viewer",
        ),
        fixture("", " extra"),
        fixture("", "").replace("1794691390000", "bad"),
        fixture("", "").replace("1794691390000", "18446744073709551615"),
        fixture("", "").replace("room-id=123;", ""),
        fixture("", "").replace("tmi-sent-ts=1794691390000", ""),
        fixture("", "").replace("room-id=123", "room-id=999"),
    ] {
        assert!(parse(&raw).is_err(), "{raw}");
    }
    assert!(irc::Line::parse("@invalid=\\s :viewer!viewer PRIVMSG #room :hello").is_none());
}

fn open(engine: &Arc<ChatEngine>, owner: &str, revision: u64) -> ChatFeedRequest {
    let request = ChatFeedRequest {
        epoch: engine.feed_context(owner).epoch,
        revision,
    };
    assert!(engine.begin_request(owner, &request));
    let feed = engine.subscribe(FeedScope::Channel("room".into()));
    assert!(engine.attach(owner, &request, "room".into(), feed.id));
    request
}

fn admitted() -> Arc<ChatEngine> {
    let engine = ChatEngine::new(ChatSettings::default());
    let mut state = engine.state.lock().unwrap();
    state.directory.insert("room".into(), room());
    state.membership.selected.insert("room".into());
    drop(state);
    engine
}

fn event(engine: &ChatEngine, id: &str) -> NoticeEvent {
    let generation = engine.notice_roster().borrow()["room"].generation;
    NoticeEvent {
        id: id.into(),
        timestamp: "2026-11-14T21:23:10Z".into(),
        channel_id: "123".into(),
        generation,
        action: NoticeAction::ChatCleared,
    }
}

#[test]
fn shared_leases_disable_reenable_and_stale_generations_are_atomic() {
    let engine = admitted();
    assert!(engine.notice_roster().borrow().is_empty());
    let first = open(&engine, "one", 1);
    let old = event(&engine, "old");
    let second = open(&engine, "two", 1);
    assert_eq!(event(&engine, "same").generation, old.generation);
    engine.detach("one", &first);
    assert!(engine.insert_notice(old.clone()));
    engine.detach("two", &second);
    assert!(!engine.insert_notice(event_with_id(old.clone(), "closed")));
    open(&engine, "one", 2);
    assert_ne!(event(&engine, "new").generation, old.generation);
    assert!(!engine.insert_notice(event_with_id(old.clone(), "stale")));
    engine.configure(ChatSettings {
        moderation_notices_enabled: false,
        ..Default::default()
    });
    assert!(engine.notice_roster().borrow().is_empty());
    assert_eq!(engine.history("room").len(), 1);
    engine.configure(ChatSettings::default());
    assert!(!engine.insert_notice(event_with_id(old, "disabled")));
    let current = event(&engine, "current");
    assert!(engine.insert_notice(current.clone()));
    assert!(!engine.insert_notice(current));
    let snapshot = engine.subscribe(FeedScope::Channel("room".into())).poll();
    assert!(matches!(&snapshot[0].event, FeedEvent::Snapshot { messages } if messages.len() == 2));
    let stopped = event(&engine, "stopped");
    engine.shutdown();
    assert!(!engine.insert_notice(stopped));
}

fn event_with_id(mut event: NoticeEvent, id: &str) -> NoticeEvent {
    event.id = id.into();
    event
}

struct FakeProvider(NoticeEvent);
impl NoticeProvider for FakeProvider {
    fn run(
        self: Box<Self>,
        _desired: watch::Receiver<Roster>,
        sink: NoticeSink,
        _health: watch::Sender<ProviderHealth>,
    ) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            sink.emit(self.0);
        })
    }
}

#[tokio::test]
async fn another_provider_uses_the_same_consumer_formatter_history_and_feed() {
    let engine = admitted();
    open(&engine, "one", 1);
    let feed = engine.subscribe(FeedScope::Channel("room".into()));
    let (tx, rx) = mpsc::channel(NOTICE_CAPACITY);
    let (health, _) = watch::channel(ProviderHealth::default());
    let provider: Box<dyn NoticeProvider> = Box::new(FakeProvider(event(&engine, "fake")));
    provider
        .run(
            engine.notice_roster(),
            NoticeSink::new(tx, health.clone()),
            health,
        )
        .await;
    runtime::consume(engine.clone(), rx).await;
    assert_eq!(engine.history("room").len(), 1);
    let frames = feed.poll();
    assert!(
        matches!(&frames[1].event, FeedEvent::Message { message: crate::chat::timeline::ChatTimelineEntry::System(message) } if message.message_id == "fake")
    );
}

#[test]
fn overflow_reports_a_gap_on_the_separate_health_channel() {
    let engine = admitted();
    open(&engine, "one", 1);
    let (tx, mut rx) = mpsc::channel(NOTICE_CAPACITY);
    let (health, status) = watch::channel(ProviderHealth::default());
    let sink = NoticeSink::new(tx, health);
    for id in 0..129 {
        sink.emit(event(&engine, &id.to_string()));
    }
    assert_eq!(status.borrow().coverage_gaps, 1);
    for _ in 0..128 {
        assert!(rx.try_recv().is_ok());
    }
    assert!(rx.try_recv().is_err());
}

#[test]
fn id_cache_is_bounded_and_irc_ids_do_not_depend_on_subscription_lifetime() {
    let mut ids = RecentIds::default();
    for id in 0..4097 {
        assert!(ids.admit(&id.to_string()));
    }
    assert!(!ids.admit("4096"));
    assert!(ids.admit("0"));
    assert_eq!(ids.ids.len(), 4096);
    let raw = fixture(";target-user-id=456;ban-duration=90", " :viewer");
    let first = parse(&raw).unwrap();
    let mut sub = subscription();
    sub.generation += 1;
    let second = irc::parse_clear(&irc::Line::parse(&raw).unwrap(), &sub).unwrap();
    assert_eq!(first.id, second.id);
}

#[test]
fn removal_effects_and_notice_ids_are_independent_and_never_add_duplicate_notices() {
    use crate::chat::moderation::{ModerationAction, ModerationEvent};
    let engine = admitted();
    open(&engine, "one", 1);
    let feed = engine.subscribe(FeedScope::Channel("room".into()));
    let effect = ModerationEvent {
        notification_id: "same-opaque-id".into(),
        ts: "2026-11-14T21:23:10Z".into(),
        broadcaster_user_id: "123".into(),
        broadcaster_user_login: "room".into(),
        broadcaster_user_name: "Room".into(),
        action: ModerationAction::ChatCleared,
    };
    assert!(engine.moderate(effect.clone()));
    assert!(engine.history("room").is_empty());
    let notice = event(&engine, "same-opaque-id");
    assert!(engine.insert_notice(notice.clone()));
    assert!(!engine.insert_notice(notice));
    assert!(!engine.moderate(effect.clone()));
    engine.configure(ChatSettings {
        moderation_notices_enabled: false,
        ..Default::default()
    });
    assert!(engine.moderate(ModerationEvent {
        notification_id: "without-irc".into(),
        ..effect
    }));
    assert_eq!(engine.history("room").len(), 1);
    let frames = feed.poll();
    assert_eq!(frames.len(), 4);
    assert!(matches!(&frames[1].event, FeedEvent::Moderation { .. }));
    assert!(matches!(&frames[2].event, FeedEvent::Message { .. }));
    assert!(matches!(&frames[3].event, FeedEvent::Moderation { .. }));
}

type LocalSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn command(socket: &mut LocalSocket) -> String {
    use futures::StreamExt;
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
                return text.to_string();
            }
        }
    })
    .await
    .expect("client command timed out")
}

async fn send(socket: &mut LocalSocket, text: &str) {
    use futures::SinkExt;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            text.to_owned().into(),
        ))
        .await
        .unwrap();
}

async fn registration(listener: &tokio::net::TcpListener) -> (LocalSocket, String) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
    assert_eq!(
        command(&mut socket).await,
        "CAP REQ :twitch.tv/commands twitch.tv/tags\r\n"
    );
    let nick = command(&mut socket).await;
    assert!(nick.starts_with("NICK justinfan"));
    let nick = nick.trim().strip_prefix("NICK ").unwrap().to_owned();
    assert_eq!(
        command(&mut socket).await,
        format!("USER {nick} 8 * :{nick}\r\n")
    );
    (socket, nick)
}

fn roster(login: &str, id: &str, generation: u64) -> Roster {
    [(
        login.into(),
        NoticeSubscription {
            room: ChatRoomInfo {
                broadcaster_id: id.into(),
                broadcaster_login: login.into(),
                broadcaster_name: login.into(),
            },
            generation,
        },
    )]
    .into_iter()
    .collect()
}

async fn local_provider() -> (
    tokio::net::TcpListener,
    watch::Sender<Roster>,
    mpsc::Receiver<NoticeEvent>,
    watch::Receiver<ProviderHealth>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let settings = Arc::new(|| crate::types::EventSubSettings {
        retry_base_secs: 1,
        retry_max_secs: 1,
        ..Default::default()
    });
    let provider: Box<dyn NoticeProvider> = Box::new(irc::IrcNoticeProvider::local(url, settings));
    let (desired, receiver) = watch::channel(roster("room", "123", 7));
    let (tx, rx) = mpsc::channel(NOTICE_CAPACITY);
    let (health, status) = watch::channel(ProviderHealth::default());
    let task = tokio::spawn(provider.run(receiver, NoticeSink::new(tx, health.clone()), health));
    (listener, desired, rx, status, task)
}

async fn welcome(socket: &mut LocalSocket, nick: &str) {
    send(socket, &format!(":tmi.twitch.tv CAP * ACK :twitch.tv/commands twitch.tv/tags\r\n:tmi.twitch.tv 001 {nick} :Welcome\r\n")).await;
}

#[tokio::test]
async fn anonymous_registration_multiline_notices_keepalive_parts_and_lifetime_changes() {
    use futures::StreamExt;
    let (listener, desired, mut events, _health, task) = local_provider().await;
    let (mut socket, nick) = registration(&listener).await;
    send(
        &mut socket,
        &format!(":tmi.twitch.tv 001 {nick} :Welcome\r\n"),
    )
    .await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(150), socket.next())
            .await
            .is_err()
    );
    send(
        &mut socket,
        ":tmi.twitch.tv CAP * ACK :twitch.tv/commands twitch.tv/tags\r\n",
    )
    .await;
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    send(&mut socket, &format!("@room-id=123 :tmi.twitch.tv ROOMSTATE #room\r\n@invalid=tag :viewer!viewer PRIVMSG #room :unrelated\r\n{}\r\nPING :tmi.twitch.tv\r\n", fixture(";target-user-id=456;ban-duration=90", " :viewer"))).await;
    assert_eq!(command(&mut socket).await, "PONG :tmi.twitch.tv\r\n");
    let notice = events.recv().await.unwrap();
    assert_eq!(notice.generation, 7);
    assert_eq!(
        text(&format_notice(notice)),
        "viewer was timed out for 1m 30s."
    );
    desired.send_replace(roster("other", "789", 8));
    assert_eq!(command(&mut socket).await, "PART #room\r\n");
    assert_eq!(command(&mut socket).await, "JOIN #other\r\n");
    send(&mut socket, &format!("{}\r\n@room-id=789 :tmi.twitch.tv ROOMSTATE #other\r\n@room-id=789;tmi-sent-ts=1794691503000 :tmi.twitch.tv CLEARCHAT #other\r\n", fixture("", ""))).await;
    let next = events.recv().await.unwrap();
    assert_eq!(next.channel_id, "789");
    assert_eq!(next.generation, 8);
    assert!(events.try_recv().is_err());
    desired.send_replace(Roster::new());
    assert!(matches!(
        socket.next().await,
        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))
    ));
    drop(desired);
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn reconnect_uses_the_latest_roster_and_rejects_unacknowledged_capabilities() {
    let (listener, desired, _events, mut health, task) = local_provider().await;
    let (mut socket, nick) = registration(&listener).await;
    welcome(&mut socket, &nick).await;
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    send(&mut socket, ":tmi.twitch.tv RECONNECT\r\n").await;
    desired.send_replace(roster("latest", "789", 8));
    let (mut next, nick) = registration(&listener).await;
    welcome(&mut next, &nick).await;
    assert_eq!(command(&mut next).await, "JOIN #latest\r\n");
    send(&mut next, ":tmi.twitch.tv RECONNECT\r\n").await;
    let (mut rejected, _) = registration(&listener).await;
    send(
        &mut rejected,
        ":tmi.twitch.tv CAP * NAK :twitch.tv/tags\r\n",
    )
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if health.borrow_and_update().issue.as_deref()
                == Some("Channel notice capabilities were rejected.")
            {
                break;
            }
            health.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    task.abort();
}

#[tokio::test]
async fn full_notice_queue_does_not_block_irc_keepalive() {
    let (listener, desired, events, health, task) = local_provider().await;
    let (mut socket, nick) = registration(&listener).await;
    welcome(&mut socket, &nick).await;
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    let mut incoming = "@room-id=123 :tmi.twitch.tv ROOMSTATE #room\r\n".to_owned();
    for _ in 0..129 {
        incoming.push_str(&fixture("", ""));
        incoming.push_str("\r\n");
    }
    incoming.push_str("PING :still-alive\r\n");
    send(&mut socket, &incoming).await;
    assert_eq!(command(&mut socket).await, "PONG :still-alive\r\n");
    assert_eq!(events.len(), 128);
    assert_eq!(health.borrow().coverage_gaps, 1);
    drop(desired);
    task.abort();
}

#[tokio::test]
async fn notice_join_failure_reports_health_and_retries_without_losing_other_protocol_work() {
    let (listener, desired, mut events, mut health, task) = local_provider().await;
    let (mut socket, nick) = registration(&listener).await;
    welcome(&mut socket, &nick).await;
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    send(&mut socket, "@msg-id=msg_channel_suspended :tmi.twitch.tv NOTICE #room :Unavailable\r\nPING :retrying\r\n").await;
    assert_eq!(command(&mut socket).await, "PART #room\r\n");
    assert_eq!(command(&mut socket).await, "PONG :retrying\r\n");
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if health
                .borrow_and_update()
                .issue
                .as_deref()
                .is_some_and(|issue| issue.starts_with("Could not join"))
            {
                break;
            }
            health.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    send(
        &mut socket,
        &format!(
            "@room-id=123 :tmi.twitch.tv ROOMSTATE #room\r\n{}\r\n",
            fixture("", "")
        ),
    )
    .await;
    assert!(matches!(
        events.recv().await.unwrap().action,
        NoticeAction::ChatCleared
    ));
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if health.borrow_and_update().issue.is_none() {
                break;
            }
            health.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    drop(desired);
    task.abort();
}

#[tokio::test]
async fn socket_closure_reconnects_and_empty_rosters_wait_for_new_subscriptions() {
    let (listener, desired, _events, _health, task) = local_provider().await;
    let (mut socket, nick) = registration(&listener).await;
    welcome(&mut socket, &nick).await;
    assert_eq!(command(&mut socket).await, "JOIN #room\r\n");
    socket.close(None).await.unwrap();
    let (mut next, nick) = registration(&listener).await;
    welcome(&mut next, &nick).await;
    assert_eq!(command(&mut next).await, "JOIN #room\r\n");
    desired.send_replace(Roster::new());
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(!task.is_finished());
    desired.send_replace(roster("reopened", "789", 9));
    let (mut reopened, nick) = registration(&listener).await;
    welcome(&mut reopened, &nick).await;
    assert_eq!(command(&mut reopened).await, "JOIN #reopened\r\n");
    drop(desired);
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

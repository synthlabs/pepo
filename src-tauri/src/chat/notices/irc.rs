use super::{
    NoticeAction, NoticeEvent, NoticeProvider, NoticeSink, NoticeSubscription, ProviderHealth,
    Roster,
};
use crate::chat::membership::JoinLimiter;
use crate::eventsub::EventSubSettingsReader;
use futures::{future::BoxFuture, SinkExt, StreamExt};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const URL: &str = "wss://irc-ws.chat.twitch.tv:443";
static NEXT_NICK: AtomicU64 = AtomicU64::new(1);
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct IrcNoticeProvider {
    url: String,
    settings: EventSubSettingsReader,
}

impl IrcNoticeProvider {
    pub fn new(settings: EventSubSettingsReader) -> Self {
        Self {
            url: URL.into(),
            settings,
        }
    }

    #[cfg(test)]
    pub(super) fn local(url: String, settings: EventSubSettingsReader) -> Self {
        Self { url, settings }
    }

    async fn serve(
        self,
        mut desired: watch::Receiver<Roster>,
        sink: NoticeSink,
        health: watch::Sender<ProviderHealth>,
    ) {
        let mut limiter = JoinLimiter::default();
        let mut attempt = 0;
        loop {
            while desired.borrow().is_empty() {
                set_issue(&health, None);
                if desired.changed().await.is_err() {
                    return;
                }
            }
            set_issue(&health, Some("Connecting to channel notices.".into()));
            let timeout = (self.settings)().socket_idle_timeout();
            let connection = tokio::time::timeout(timeout, connect_async(&self.url));
            tokio::pin!(connection);
            let connected = loop {
                tokio::select! {
                    result = &mut connection => break result.map_err(|_| "Connection timed out.".to_string())
                        .and_then(|result| result.map_err(|_| "Connection failed.".to_string())),
                    result = desired.changed() => {
                        if result.is_err() { return; }
                        if desired.borrow().is_empty() { break Err("Subscriptions ended.".into()); }
                    }
                }
            };
            let result = match connected {
                Ok((mut socket, _)) => {
                    let result = self
                        .session(&mut socket, &mut desired, &sink, &health, &mut limiter)
                        .await;
                    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close(None)).await;
                    result
                }
                Err(error) => Err(error),
            };
            if desired.has_changed().is_err() {
                return;
            }
            if desired.borrow().is_empty() {
                attempt = 0;
                continue;
            }
            if health.borrow().issue.is_none() {
                attempt = 0;
            }
            match result {
                Ok(()) => attempt = 0,
                Err(error) => set_issue(&health, Some(error)),
            }
            let delay = Duration::from_secs((self.settings)().retry_delay_secs(attempt));
            attempt = attempt.saturating_add(1);
            let retry = tokio::time::sleep(delay);
            tokio::pin!(retry);
            loop {
                tokio::select! {
                    _ = &mut retry => break,
                    result = desired.changed() => {
                        if result.is_err() { return; }
                        if desired.borrow().is_empty() { break; }
                    }
                }
            }
        }
    }

    async fn send(&self, socket: &mut Socket, command: String) -> Result<(), String> {
        tokio::time::timeout(
            (self.settings)().socket_idle_timeout(),
            socket.send(Message::Text(format!("{command}\r\n").into())),
        )
        .await
        .map_err(|_| "Sending a notice command timed out.".to_string())?
        .map_err(|_| "Channel notices connection closed.".into())
    }

    async fn session(
        &self,
        socket: &mut Socket,
        desired: &mut watch::Receiver<Roster>,
        sink: &NoticeSink,
        health: &watch::Sender<ProviderHealth>,
        limiter: &mut JoinLimiter,
    ) -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            + u128::from(NEXT_NICK.fetch_add(1, Ordering::Relaxed));
        let nick = format!("justinfan{nonce}");
        self.send(socket, "CAP REQ :twitch.tv/commands twitch.tv/tags".into())
            .await?;
        self.send(socket, format!("NICK {nick}")).await?;
        self.send(socket, format!("USER {nick} 8 * :{nick}"))
            .await?;
        let mut registered = false;
        let (mut commands, mut tags) = (false, false);
        let mut joined = Roster::new();
        let mut pending: BTreeMap<String, (NoticeSubscription, Instant)> = BTreeMap::new();
        let mut failed: HashMap<String, (Instant, u32)> = HashMap::new();
        let mut ticker = tokio::time::interval(Duration::from_millis(100));
        let mut last_received = Instant::now();
        let registration_started = Instant::now();
        loop {
            let roster = desired.borrow_and_update().clone();
            if roster.is_empty() {
                set_issue(health, None);
                return Ok(());
            }
            let ready = registered && commands && tags;
            let removed: Vec<_> = joined
                .iter()
                .chain(pending.iter().map(|(login, (sub, _))| (login, sub)))
                .filter(|(login, sub)| roster.get(*login) != Some(*sub))
                .map(|(login, _)| login.clone())
                .collect();
            for login in removed {
                self.send(socket, format!("PART #{login}")).await?;
                joined.remove(&login);
                pending.remove(&login);
                failed.remove(&login);
            }
            failed.retain(|login, _| roster.contains_key(login));
            if ready {
                for (login, sub) in &roster {
                    if joined.contains_key(login)
                        || pending.contains_key(login)
                        || failed
                            .get(login)
                            .is_some_and(|(retry_at, _)| *retry_at > Instant::now())
                    {
                        continue;
                    }
                    if !limiter.acquire(Instant::now()) {
                        break;
                    }
                    self.send(socket, format!("JOIN #{login}")).await?;
                    pending.insert(login.clone(), (sub.clone(), Instant::now()));
                }
                let expired: Vec<_> = pending
                    .iter()
                    .filter(|(_, (_, started))| {
                        started.elapsed() >= (self.settings)().socket_idle_timeout()
                    })
                    .map(|(login, _)| login.clone())
                    .collect();
                for login in expired {
                    self.send(socket, format!("PART #{login}")).await?;
                    pending.remove(&login);
                    schedule_retry(&mut failed, login, (self.settings)());
                }
                let issue = if !failed.is_empty() {
                    Some(format!(
                        "Could not join {} channel notice subscription(s); retrying.",
                        failed.len()
                    ))
                } else if joined.len() != roster.len() {
                    Some("Joining channel notices.".into())
                } else {
                    None
                };
                set_issue(health, issue);
            } else if registration_started.elapsed() >= (self.settings)().socket_idle_timeout() {
                return Err("Channel notice registration timed out.".into());
            }
            if last_received.elapsed() >= (self.settings)().socket_idle_timeout() {
                return Err("Channel notices connection went idle.".into());
            }
            tokio::select! {
                result = desired.changed() => { if result.is_err() { return Ok(()); } }
                _ = ticker.tick() => {}
                incoming = socket.next() => {
                    last_received = Instant::now();
                    match incoming {
                        Some(Ok(Message::Text(text))) => {
                            for raw in text.split('\n') {
                                let Some(line) = Line::parse(raw.trim_end_matches('\r')) else { continue; };
                                match line.command {
                                    "PING" => self.send(socket, format!("PONG {}", line.params)).await?,
                                    "RECONNECT" => return Err("Channel notices requested a reconnect.".into()),
                                    "CAP" => {
                                        if line.params.split_ascii_whitespace().any(|part| part == "NAK") {
                                            return Err("Channel notice capabilities were rejected.".into());
                                        }
                                        if line.params.split_ascii_whitespace().any(|part| part == "ACK") {
                                            commands |= line.params.split_ascii_whitespace().any(|part| part.trim_start_matches(':') == "twitch.tv/commands");
                                            tags |= line.params.split_ascii_whitespace().any(|part| part.trim_start_matches(':') == "twitch.tv/tags");
                                        }
                                    }
                                    "001" => { registered = line.params.split_ascii_whitespace().next() == Some(nick.as_str()); }
                                    "ROOMSTATE" | "366" | "JOIN" if ready => {
                                        if let Some(login) = line.room() {
                                            if line.command == "JOIN" && line.prefix.split('!').next() != Some(nick.as_str()) { continue; }
                                            if let Some((sub, _)) = pending.get(login) {
                                                if desired.borrow().get(login) != Some(sub) { continue; }
                                                if line.command == "ROOMSTATE" && line.tag("room-id") != Some(sub.room.broadcaster_id.as_str()) { continue; }
                                                let (sub, _) = pending.remove(login).unwrap();
                                                joined.insert(login.to_owned(), sub);
                                                failed.remove(login);
                                            }
                                        }
                                    }
                                    "NOTICE" => {
                                        if let Some(login) = line.room() {
                                            let join_failure = pending.contains_key(login) || matches!(line.tag("msg-id"),
                                                Some("msg_banned" | "msg_timedout" | "msg_channel_suspended" | "no_permission" | "msg_room_not_found"));
                                            if join_failure {
                                                self.send(socket, format!("PART #{login}")).await?;
                                                pending.remove(login);
                                                joined.remove(login);
                                                if roster.contains_key(login) { schedule_retry(&mut failed, login.to_owned(), (self.settings)()); }
                                            }
                                        } else { return Err("Channel notice registration was rejected.".into()); }
                                    }
                                    "CLEARCHAT" => {
                                        let Some(login) = line.room() else { self.malformed(); continue; };
                                        let Some(sub) = joined.get(login) else { continue; };
                                        if desired.borrow().get(login) != Some(sub) { continue; }
                                        match parse_clear(&line, sub) {
                                            Ok(event) => sink.emit(event),
                                            Err(()) => self.malformed(),
                                        }
                                    }
                                    "403" | "404" | "471" | "473" | "474" | "475" => {
                                        if let Some(login) = line.room() {
                                            pending.remove(login);
                                            joined.remove(login);
                                            if roster.contains_key(login) { schedule_retry(&mut failed, login.into(), (self.settings)()); }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Some(Ok(Message::Ping(data))) => {
                            tokio::time::timeout((self.settings)().socket_idle_timeout(), socket.send(Message::Pong(data))).await
                                .map_err(|_| "Channel notice keepalive timed out.".to_string())?
                                .map_err(|_| "Channel notice keepalive failed.".to_string())?;
                        }
                        Some(Ok(Message::Close(_))) | None => return Err("Channel notices connection closed.".into()),
                        Some(Err(_)) => return Err("Channel notices connection failed.".into()),
                        _ => {}
                    }
                }
            }
        }
    }

    fn malformed(&self) {
        crate::logging::error_repeated(
            "channel_notices:malformed",
            "Malformed channel notice omitted.",
            (self.settings)().unparseable_warning_throttle(),
        );
    }
}

impl NoticeProvider for IrcNoticeProvider {
    fn run(
        self: Box<Self>,
        desired: watch::Receiver<Roster>,
        sink: NoticeSink,
        health: watch::Sender<ProviderHealth>,
    ) -> BoxFuture<'static, ()> {
        Box::pin(self.serve(desired, sink, health))
    }
}

fn schedule_retry(
    failed: &mut HashMap<String, (Instant, u32)>,
    login: String,
    settings: crate::types::EventSubSettings,
) {
    let attempts = failed
        .get(&login)
        .map_or(0, |(_, attempts)| attempts.saturating_add(1));
    failed.insert(
        login,
        (
            Instant::now() + Duration::from_secs(settings.retry_delay_secs(attempts)),
            attempts,
        ),
    );
}

fn set_issue(health: &watch::Sender<ProviderHealth>, issue: Option<String>) {
    health.send_if_modified(|status| {
        if status.issue == issue {
            return false;
        }
        status.issue = issue;
        true
    });
}

pub(super) struct Line<'a> {
    command: &'a str,
    params: &'a str,
    tags: &'a str,
    prefix: &'a str,
}

impl<'a> Line<'a> {
    pub(super) fn parse(mut raw: &'a str) -> Option<Self> {
        let mut tags = "";
        let mut prefix = "";
        if let Some(tagged) = raw.strip_prefix('@') {
            (tags, raw) = tagged.split_once(' ')?;
            raw = raw.trim_start();
        }
        if let Some(prefixed) = raw.strip_prefix(':') {
            (prefix, raw) = prefixed.split_once(' ')?;
            raw = raw.trim_start();
        }
        let (command, params) = raw.split_once(' ').unwrap_or((raw, ""));
        // Skip ordinary chat before inspecting or allocating tags.
        if matches!(command, "PRIVMSG" | "USERNOTICE" | "WHISPER" | "CLEARMSG")
            || command.is_empty()
        {
            return None;
        }
        Some(Self {
            command,
            params: params.trim_start(),
            tags,
            prefix,
        })
    }

    fn tag(&self, key: &str) -> Option<&'a str> {
        self.tags
            .split(';')
            .map(|tag| tag.split_once('=').unwrap_or((tag, "")))
            .find_map(|(name, value)| (name == key).then_some(value))
    }

    fn room(&self) -> Option<&'a str> {
        self.params
            .split_ascii_whitespace()
            .take_while(|part| !part.starts_with(':'))
            .find_map(|part| part.strip_prefix('#'))
    }
}

pub(super) fn parse_clear(line: &Line<'_>, sub: &NoticeSubscription) -> Result<NoticeEvent, ()> {
    if line.command != "CLEARCHAT"
        || line.room() != Some(sub.room.broadcaster_login.as_str())
        || line.tag("room-id") != Some(sub.room.broadcaster_id.as_str())
    {
        return Err(());
    }
    for key in ["room-id", "tmi-sent-ts", "target-user-id", "ban-duration"] {
        if line
            .tags
            .split(';')
            .filter(|tag| tag.split('=').next() == Some(key))
            .count()
            > 1
        {
            return Err(());
        }
    }
    let millis = line.tag("tmi-sent-ts").ok_or(())?;
    if millis.is_empty() || !millis.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    let millis: u64 = millis.parse().map_err(|_| ())?;
    let timestamp = OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .map_err(|_| ())?
        .format(&Rfc3339)
        .map_err(|_| ())?;
    let target = match line.params.split_once(' ') {
        Some((_, trailing)) => Some(trailing.trim_start().strip_prefix(':').ok_or(())?),
        None => None,
    };
    let duration = line.tag("ban-duration");
    let target_id = line.tag("target-user-id");
    let (action, key) = match target {
        Some(login) => {
            if login.is_empty()
                || !login
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err(());
            }
            let user_id = target_id
                .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
                .ok_or(())?;
            if let Some(duration) = duration {
                if duration.is_empty() || !duration.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(());
                }
                let duration_secs: u64 = duration.parse().map_err(|_| ())?;
                if duration_secs == 0 {
                    return Err(());
                }
                (
                    NoticeAction::UserTimedOut {
                        user_id: user_id.into(),
                        login: login.into(),
                        duration_secs,
                    },
                    format!("timeout:{user_id}:{login}:{duration_secs}"),
                )
            } else {
                (
                    NoticeAction::UserBanned {
                        user_id: user_id.into(),
                        login: login.into(),
                    },
                    format!("ban:{user_id}:{login}"),
                )
            }
        }
        None if duration.is_none() && target_id.is_none() => {
            (NoticeAction::ChatCleared, "clear".into())
        }
        _ => return Err(()),
    };
    Ok(NoticeEvent {
        id: format!("irc:{}:{millis}:{key}", sub.room.broadcaster_id),
        timestamp,
        channel_id: sub.room.broadcaster_id.clone(),
        generation: sub.generation,
        action,
    })
}

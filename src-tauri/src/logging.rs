use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use log::LevelFilter;
use tracing::{error, info, warn};

const CHAT_PARSE_SUMMARY_INTERVAL: Duration = Duration::from_secs(60);

static REPEATED_LOGS: OnceLock<Coalescer> = OnceLock::new();
static CHAT_PARSE_STATS: OnceLock<Mutex<ChatParseStats>> = OnceLock::new();

pub(crate) fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

pub(crate) fn pepo_log_level() -> LevelFilter {
    match std::env::var("PEPO_LOG").as_deref() {
        Ok("trace") | Ok("TRACE") => LevelFilter::Trace,
        Ok("debug") | Ok("DEBUG") | Ok("1") | Ok("true") | Ok("TRUE") => LevelFilter::Debug,
        _ => LevelFilter::Info,
    }
}

pub(crate) fn warn_repeated(
    key: impl Into<String>,
    message: impl Into<String>,
    interval: Duration,
) {
    let key = key.into();
    let message = message.into();
    match repeated_logs().record(key.clone(), message.clone(), Instant::now(), interval) {
        RepeatLogDecision::Emit => warn!("{}", message),
        RepeatLogDecision::Suppress => {}
        RepeatLogDecision::Summary(summary) => warn!(
            key = %key,
            suppressed = summary.suppressed,
            window_secs = summary.window.as_secs_f64(),
            last = %summary.last_message,
            "suppressed repeated warning"
        ),
    }
}

pub(crate) fn error_repeated(
    key: impl Into<String>,
    message: impl Into<String>,
    interval: Duration,
) {
    let key = key.into();
    let message = message.into();
    match repeated_logs().record(key.clone(), message.clone(), Instant::now(), interval) {
        RepeatLogDecision::Emit => error!("{}", message),
        RepeatLogDecision::Suppress => {}
        RepeatLogDecision::Summary(summary) => error!(
            key = %key,
            suppressed = summary.suppressed,
            window_secs = summary.window.as_secs_f64(),
            last = %summary.last_message,
            "suppressed repeated error"
        ),
    }
}

pub(crate) fn record_chat_parse(
    msg_len: usize,
    providers: Vec<String>,
    fragments: usize,
    duration: Duration,
    emote_hits: usize,
) {
    let stats = CHAT_PARSE_STATS.get_or_init(|| Mutex::new(ChatParseStats::default()));
    let mut stats = stats.lock().unwrap();
    if let Some(summary) = stats.record(
        Instant::now(),
        CHAT_PARSE_SUMMARY_INTERVAL,
        msg_len,
        providers,
        fragments,
        duration,
        emote_hits,
    ) {
        info!(
            messages = summary.messages,
            emote_hits = summary.emote_hits,
            avg_msg_len = summary.avg_msg_len,
            avg_fragments = summary.avg_fragments,
            avg_parse_ms = summary.avg_parse_ms,
            max_parse_ms = summary.max_parse_ms,
            providers = ?summary.providers,
            "chat parser summary"
        );
    }
}

fn repeated_logs() -> &'static Coalescer {
    REPEATED_LOGS.get_or_init(Coalescer::default)
}

#[derive(Default)]
struct Coalescer {
    entries: Mutex<HashMap<String, CoalescedEntry>>,
}

impl Coalescer {
    fn record(
        &self,
        key: String,
        message: String,
        now: Instant,
        interval: Duration,
    ) -> RepeatLogDecision {
        let mut entries = self.entries.lock().unwrap();
        let Some(entry) = entries.get_mut(&key) else {
            entries.insert(key, CoalescedEntry::new(now));
            return RepeatLogDecision::Emit;
        };

        if now.duration_since(entry.last_emit) >= interval {
            if entry.suppressed == 0 {
                entry.last_emit = now;
                return RepeatLogDecision::Emit;
            }

            entry.suppressed += 1;
            entry.last_seen = now;
            entry.last_message = message;
            let first_suppressed = entry.first_suppressed.unwrap_or(now);
            let summary = RepeatLogSummary {
                suppressed: entry.suppressed,
                window: now.duration_since(first_suppressed),
                last_message: entry.last_message.clone(),
            };

            entry.last_emit = now;
            entry.first_suppressed = None;
            entry.suppressed = 0;
            return RepeatLogDecision::Summary(summary);
        }

        entry.suppressed += 1;
        entry.first_suppressed.get_or_insert(now);
        entry.last_seen = now;
        entry.last_message = message;
        RepeatLogDecision::Suppress
    }
}

struct CoalescedEntry {
    last_emit: Instant,
    first_suppressed: Option<Instant>,
    last_seen: Instant,
    last_message: String,
    suppressed: u64,
}

impl CoalescedEntry {
    fn new(now: Instant) -> Self {
        Self {
            last_emit: now,
            first_suppressed: None,
            last_seen: now,
            last_message: String::new(),
            suppressed: 0,
        }
    }
}

#[derive(Debug, PartialEq)]
enum RepeatLogDecision {
    Emit,
    Suppress,
    Summary(RepeatLogSummary),
}

#[derive(Debug, PartialEq)]
struct RepeatLogSummary {
    suppressed: u64,
    window: Duration,
    last_message: String,
}

#[derive(Default)]
struct ChatParseStats {
    window_start: Option<Instant>,
    messages: u64,
    total_msg_len: u64,
    total_fragments: u64,
    total_parse_ms: f64,
    max_parse_ms: f64,
    emote_hits: u64,
    providers: Vec<String>,
}

impl ChatParseStats {
    fn record(
        &mut self,
        now: Instant,
        interval: Duration,
        msg_len: usize,
        providers: Vec<String>,
        fragments: usize,
        duration: Duration,
        emote_hits: usize,
    ) -> Option<ChatParseSummary> {
        let window_start = *self.window_start.get_or_insert(now);

        self.messages += 1;
        self.total_msg_len += msg_len as u64;
        self.total_fragments += fragments as u64;
        let parse_ms = duration_ms(duration);
        self.total_parse_ms += parse_ms;
        self.max_parse_ms = self.max_parse_ms.max(parse_ms);
        self.emote_hits += emote_hits as u64;
        self.providers = providers;

        if now.duration_since(window_start) < interval {
            return None;
        }

        let summary = ChatParseSummary {
            messages: self.messages,
            emote_hits: self.emote_hits,
            avg_msg_len: self.total_msg_len as f64 / self.messages as f64,
            avg_fragments: self.total_fragments as f64 / self.messages as f64,
            avg_parse_ms: self.total_parse_ms / self.messages as f64,
            max_parse_ms: self.max_parse_ms,
            providers: self.providers.clone(),
        };

        *self = ChatParseStats {
            window_start: Some(now),
            ..Default::default()
        };

        Some(summary)
    }
}

#[derive(Debug, PartialEq)]
struct ChatParseSummary {
    messages: u64,
    emote_hits: u64,
    avg_msg_len: f64,
    avg_fragments: f64,
    avg_parse_ms: f64,
    max_parse_ms: f64,
    providers: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalescer_emits_first_suppresses_repeats_and_summarizes_later() {
        let coalescer = Coalescer::default();
        let now = Instant::now();
        let interval = Duration::from_secs(10);

        assert_eq!(
            coalescer.record("poll".into(), "first".into(), now, interval),
            RepeatLogDecision::Emit
        );
        assert_eq!(
            coalescer.record(
                "poll".into(),
                "second".into(),
                now + Duration::from_secs(1),
                interval
            ),
            RepeatLogDecision::Suppress
        );
        assert_eq!(
            coalescer.record(
                "poll".into(),
                "third".into(),
                now + Duration::from_secs(2),
                interval
            ),
            RepeatLogDecision::Suppress
        );
        assert_eq!(
            coalescer.record(
                "poll".into(),
                "fourth".into(),
                now + Duration::from_secs(10),
                interval
            ),
            RepeatLogDecision::Summary(RepeatLogSummary {
                suppressed: 3,
                window: Duration::from_secs(9),
                last_message: "fourth".into(),
            })
        );
    }

    #[test]
    fn coalescer_logs_sparse_repeats_without_summary() {
        let coalescer = Coalescer::default();
        let now = Instant::now();
        let interval = Duration::from_secs(10);

        assert_eq!(
            coalescer.record("poll".into(), "first".into(), now, interval),
            RepeatLogDecision::Emit
        );
        assert_eq!(
            coalescer.record(
                "poll".into(),
                "second".into(),
                now + Duration::from_secs(11),
                interval
            ),
            RepeatLogDecision::Emit
        );
    }

    #[test]
    fn chat_parse_stats_returns_window_summary() {
        let mut stats = ChatParseStats::default();
        let now = Instant::now();
        let interval = Duration::from_secs(60);

        assert_eq!(
            stats.record(
                now,
                interval,
                10,
                vec!["Provider".into()],
                1,
                Duration::from_micros(10),
                0
            ),
            None
        );

        assert_eq!(
            stats.record(
                now + Duration::from_secs(60),
                interval,
                20,
                vec!["Provider".into(), "Global".into()],
                3,
                Duration::from_micros(30),
                2
            ),
            Some(ChatParseSummary {
                messages: 2,
                emote_hits: 2,
                avg_msg_len: 15.0,
                avg_fragments: 2.0,
                avg_parse_ms: duration_ms(Duration::from_micros(20)),
                max_parse_ms: duration_ms(Duration::from_micros(30)),
                providers: vec!["Provider".into(), "Global".into()],
            })
        );
    }
}

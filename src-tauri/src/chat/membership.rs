use crate::types::ChatJoinMode;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub const STEADY_ROOMS: usize = 50;
pub const MAX_ROOMS: usize = STEADY_ROOMS + 1;

#[derive(Clone, Default)]
pub struct Membership {
    pub followed: HashMap<String, bool>,
    pub lingering: HashSet<String>,
    pub foreground: HashSet<String>,
    pub selected: HashSet<String>,
    pub unavailable: HashSet<String>,
    opened: HashMap<String, u64>,
    recent: VecDeque<String>,
    sequence: u64,
}

impl Membership {
    pub fn open(&mut self, login: &str) {
        if self.foreground.insert(login.to_owned()) {
            self.sequence += 1;
            self.opened.insert(login.to_owned(), self.sequence);
            self.recent.retain(|room| room != login);
            self.lingering.remove(login);
        }
    }

    pub fn close(&mut self, login: &str) {
        if self.foreground.remove(login) {
            self.opened.remove(login);
            self.recent.retain(|room| room != login);
            self.recent.push_front(login.to_owned());
            self.lingering.insert(login.to_owned());
            while self.recent.len() > STEADY_ROOMS {
                if let Some(expired) = self.recent.pop_back() {
                    self.lingering.remove(&expired);
                }
            }
        }
    }

    pub fn intended(&self, mode: ChatJoinMode) -> HashSet<String> {
        let mut candidates = self.foreground.clone();
        if mode == ChatJoinMode::Recent {
            candidates.extend(self.lingering.iter().cloned());
        }
        candidates.extend(self.followed.iter().filter_map(|(login, live)| {
            (mode == ChatJoinMode::All && *live).then(|| login.clone())
        }));
        candidates
    }

    pub fn ranked(&self, mode: ChatJoinMode) -> Vec<String> {
        let mut candidates = self.intended(mode);
        candidates.retain(|login| !self.unavailable.contains(login));
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_by_key(|login| {
            let (priority, recency) = if self.foreground.contains(login) {
                (0, u64::MAX - self.opened.get(login).copied().unwrap_or(0))
            } else if mode == ChatJoinMode::Recent {
                (
                    1,
                    self.recent
                        .iter()
                        .position(|room| room == login)
                        .unwrap_or(usize::MAX) as u64,
                )
            } else {
                (1, 0)
            };
            (
                priority,
                recency,
                !self.selected.contains(login),
                login.clone(),
            )
        });
        candidates
    }

    pub fn displaced(&mut self, login: &str) {
        self.selected.remove(login);
    }

    pub fn would_admit(&self, login: &str, mode: ChatJoinMode) -> bool {
        self.ranked(mode)
            .iter()
            .take(STEADY_ROOMS)
            .any(|candidate| candidate == login)
    }
}

#[derive(Default)]
pub struct JoinLimiter {
    attempts: VecDeque<Instant>,
}

impl JoinLimiter {
    pub fn acquire(&mut self, now: Instant) -> bool {
        while self
            .attempts
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(10))
        {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= 20 {
            return false;
        }
        self.attempts.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_modes_match_their_rosters() {
        let mut m = Membership::default();
        m.followed.insert("live".into(), true);
        m.followed.insert("offline".into(), false);
        m.open("closed");
        m.close("closed");
        m.open("open");
        assert_eq!(m.ranked(ChatJoinMode::All), ["open", "live"]);
        assert_eq!(m.ranked(ChatJoinMode::Recent), ["open", "closed"]);
        assert_eq!(m.ranked(ChatJoinMode::Open), ["open"]);
        m.displaced("closed");
        assert!(m.intended(ChatJoinMode::Recent).contains("closed"));
    }

    #[test]
    fn newest_openings_win_and_shared_opens_keep_their_position() {
        let mut m = Membership::default();
        for i in 0..51 {
            m.open(&format!("room{i:03}"));
        }
        m.open("room000");
        let rooms = m.ranked(ChatJoinMode::Open);
        assert_eq!(rooms[0], "room050");
        assert!(!m.would_admit("room000", ChatJoinMode::Open));
        assert!(m.would_admit("room050", ChatJoinMode::Open));
        assert_eq!(STEADY_ROOMS * ROOM_STREAMS + 1, 251);
        assert_eq!(MAX_ROOMS * ROOM_STREAMS + 1, 256);
    }
    const ROOM_STREAMS: usize = 5;

    #[test]
    fn recent_closed_records_are_bounded_and_keep_latest_closing_first() {
        let mut m = Membership::default();
        for i in 0..60 {
            let login = format!("room{i:03}");
            m.open(&login);
            m.close(&login);
        }
        assert_eq!(m.lingering.len(), 50);
        assert_eq!(m.ranked(ChatJoinMode::Recent)[0], "room059");
        assert!(!m.lingering.contains("room009"));
        m.open("room010");
        m.close("room010");
        assert_eq!(m.ranked(ChatJoinMode::Recent)[0], "room010");
    }

    #[test]
    fn foreground_preempts_and_existing_live_members_win_ties() {
        let mut m = Membership::default();
        for i in 0..101 {
            m.followed.insert(format!("live{i:03}"), true);
        }
        m.selected.insert("live100".into());
        m.open("visitor");
        let rooms: Vec<_> = m
            .ranked(ChatJoinMode::All)
            .into_iter()
            .take(STEADY_ROOMS)
            .collect();
        assert_eq!(rooms[0], "visitor");
        assert_eq!(rooms[1], "live100");
        assert_eq!(rooms.len(), 50);
    }

    #[test]
    fn unavailable_rooms_release_admission_priority_without_erasing_recency() {
        let mut m = Membership::default();
        for i in 0..51 {
            let login = format!("room{i:03}");
            m.open(&login);
            m.close(&login);
        }
        m.unavailable.insert("room050".into());
        assert_eq!(m.ranked(ChatJoinMode::Recent).len(), 49);
        assert!(m.intended(ChatJoinMode::Recent).contains("room050"));
        m.unavailable.clear();
        assert_eq!(m.ranked(ChatJoinMode::Recent)[0], "room050");
    }

    #[test]
    fn sliding_window_includes_retries_and_reconnects() {
        let mut limiter = JoinLimiter::default();
        let start = Instant::now();
        for _ in 0..20 {
            assert!(limiter.acquire(start));
        }
        assert!(!limiter.acquire(start + Duration::from_millis(9999)));
        assert!(limiter.acquire(start + Duration::from_secs(10)));
    }
}

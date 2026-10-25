use crate::types::ChatJoinMode;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub const MAX_ROOMS: usize = 100;
pub const STEADY_ROOMS: usize = MAX_ROOMS - 1;

#[derive(Clone, Default)]
pub struct Membership {
    pub followed: HashMap<String, bool>,
    pub lingering: HashSet<String>,
    pub foreground: HashSet<String>,
    pub selected: HashSet<String>,
    pub unavailable: HashSet<String>,
}

impl Membership {
    pub fn intended(&self, mode: ChatJoinMode) -> HashSet<String> {
        let mut candidates = self.foreground.clone();
        candidates.extend(self.lingering.iter().cloned());
        candidates.extend(self.followed.iter().filter_map(|(login, live)| {
            (mode == ChatJoinMode::All || (mode == ChatJoinMode::Live && *live))
                .then(|| login.clone())
        }));
        candidates
    }

    pub fn ranked(&self, mode: ChatJoinMode) -> Vec<String> {
        let mut candidates = self.intended(mode);
        candidates.retain(|login| !self.unavailable.contains(login));
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_by_key(|login| {
            let priority = if self.foreground.contains(login) {
                0
            } else if self.followed.get(login) == Some(&true) && mode != ChatJoinMode::Single {
                1
            } else if self.followed.contains_key(login) && mode == ChatJoinMode::All {
                2
            } else {
                3
            };
            (priority, !self.selected.contains(login), login.clone())
        });
        candidates
    }

    pub fn displaced(&mut self, login: &str) {
        self.selected.remove(login);
        self.lingering.remove(login);
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
    fn modes_and_lingering_rooms() {
        let mut m = Membership::default();
        m.followed.insert("live".into(), true);
        m.followed.insert("offline".into(), false);
        m.lingering.insert("visitor".into());
        assert_eq!(m.ranked(ChatJoinMode::Live), ["live", "visitor"]);
        assert_eq!(m.ranked(ChatJoinMode::Single), ["visitor"]);
        assert_eq!(m.ranked(ChatJoinMode::All), ["live", "offline", "visitor"]);
    }
    #[test]
    fn ranking_preserves_lingering_intent_until_displacement_commits() {
        let mut m = Membership::default();
        m.lingering.insert("visitor".into());
        m.selected.insert("visitor".into());
        for i in 0..100 {
            m.followed.insert(format!("live{i:03}"), true);
        }
        let ranked = m.ranked(ChatJoinMode::Live);
        assert_eq!(ranked.len(), 101);
        assert!(!ranked
            .iter()
            .take(STEADY_ROOMS)
            .any(|login| login == "visitor"));
        assert!(m.lingering.contains("visitor"));
        m.displaced("visitor");
        assert!(!m.lingering.contains("visitor"));
        m.followed.clear();
        assert!(m.ranked(ChatJoinMode::Live).is_empty());
    }
    #[test]
    fn foreground_preempts_and_existing_members_win_ties() {
        let mut m = Membership::default();
        for i in 0..101 {
            m.followed.insert(format!("live{i:03}"), true);
        }
        m.selected.insert("live100".into());
        m.foreground.insert("visitor".into());
        let rooms: Vec<_> = m
            .ranked(ChatJoinMode::Live)
            .into_iter()
            .take(STEADY_ROOMS)
            .collect();
        assert_eq!(rooms[0], "visitor");
        assert_eq!(rooms[1], "live100");
        assert!(!rooms.contains(&"live099".into()));
    }
    #[test]
    fn unavailable_rooms_release_capacity_for_healthy_candidates() {
        let mut m = Membership::default();
        for i in 0..100 {
            m.followed.insert(format!("live{i:03}"), true);
        }
        m.unavailable.insert("live000".into());
        let rooms = m.ranked(ChatJoinMode::Live);
        assert_eq!(rooms.len(), STEADY_ROOMS);
        assert!(!rooms.contains(&"live000".into()));
        assert!(rooms.contains(&"live099".into()));
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

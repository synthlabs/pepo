use super::membership::{JoinLimiter, Membership, MAX_ROOMS, STEADY_ROOMS};
use crate::types::ChatJoinMode;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RoomFailure {
    pub reason: String,
    pub attempts: u32,
    pub retry_at: Instant,
}

pub fn retry_delay(attempts: u32) -> Duration {
    Duration::from_secs((5u64.saturating_mul(1u64 << attempts.saturating_sub(1).min(6))).min(300))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionAction {
    Join(String),
    Leave { login: String, displaced: bool },
}

#[derive(Default)]
pub struct AdmissionCoordinator {
    cleanup: HashMap<String, Option<RoomFailure>>,
    displacements: HashSet<String>,
    limiter: JoinLimiter,
    backoff_until: Option<Instant>,
}

impl AdmissionCoordinator {
    pub fn require_cleanup(&mut self, login: &str) {
        self.cleanup.entry(login.to_owned()).or_insert(None);
        self.displacements.remove(login);
    }

    pub fn cleanup_succeeded(&mut self, login: &str) {
        self.cleanup.remove(login);
        self.displacements.remove(login);
    }

    pub fn cleanup_failed(&mut self, login: &str, reason: String, now: Instant) {
        let attempts = self
            .cleanup
            .get(login)
            .and_then(Option::as_ref)
            .map_or(1, |f| f.attempts.saturating_add(1));
        self.cleanup.insert(
            login.to_owned(),
            Some(RoomFailure {
                reason,
                attempts,
                retry_at: now + retry_delay(attempts),
            }),
        );
    }

    pub fn throttle(&mut self, now: Instant) {
        self.backoff_until = Some(now + Duration::from_secs(60));
    }

    pub fn acquire_join(&mut self, now: Instant) -> bool {
        !self.paused(now) && self.limiter.acquire(now)
    }

    fn paused(&self, now: Instant) -> bool {
        self.backoff_until.is_some_and(|until| until > now)
    }

    pub fn next_action(
        &mut self,
        membership: &Membership,
        mode: ChatJoinMode,
        desired: &[String],
        physical: &HashSet<String>,
        complete: &HashSet<String>,
        now: Instant,
    ) -> Option<AdmissionAction> {
        if self.paused(now) {
            return None;
        }
        let intended = membership.intended(mode);
        let ranked = membership.ranked(mode);
        let targets: HashSet<_> = ranked.iter().take(STEADY_ROOMS).cloned().collect();
        let retained: Vec<_> = self
            .displacements
            .iter()
            .filter(|login| complete.contains(*login) && targets.contains(*login))
            .cloned()
            .collect();
        for login in retained {
            self.cleanup_succeeded(&login);
        }
        for login in desired {
            if !intended.contains(login) {
                self.require_cleanup(login);
                self.displacements.insert(login.clone());
            }
        }
        if !self.cleanup.is_empty() {
            let mut ready: Vec<_> = self
                .cleanup
                .iter()
                .filter(|(_, failure)| failure.as_ref().is_none_or(|f| f.retry_at <= now))
                .map(|(login, _)| login.clone())
                .collect();
            ready.sort();
            return ready
                .into_iter()
                .next()
                .map(|login| AdmissionAction::Leave {
                    displaced: self.displacements.contains(&login) || !intended.contains(&login),
                    login,
                });
        }
        if complete.len() > STEADY_ROOMS {
            let mut displaced: Vec<_> = complete.difference(&targets).cloned().collect();
            displaced.sort();
            if let Some(login) = displaced.into_iter().next() {
                self.require_cleanup(&login);
                self.displacements.insert(login.clone());
                return Some(AdmissionAction::Leave {
                    login,
                    displaced: true,
                });
            }
        }
        if physical.len() >= MAX_ROOMS {
            return None;
        }
        ranked
            .into_iter()
            .take(STEADY_ROOMS)
            .find(|login| !complete.contains(login))
            .map(AdmissionAction::Join)
    }

    pub fn cleanup_errors(&self) -> impl Iterator<Item = (&String, &RoomFailure)> {
        self.cleanup
            .iter()
            .filter_map(|(login, failure)| failure.as_ref().map(|failure| (login, failure)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rooms {
        membership: Membership,
        physical: HashSet<String>,
        complete: HashSet<String>,
    }

    impl Rooms {
        fn full() -> Self {
            let complete: HashSet<_> = (0..STEADY_ROOMS).map(|i| format!("room{i:03}")).collect();
            let mut membership = Membership::default();
            membership.followed = complete.iter().map(|login| (login.clone(), true)).collect();
            membership.selected = complete.clone();
            membership.foreground.insert("visitor".into());
            Self {
                membership,
                physical: complete.clone(),
                complete,
            }
        }

        fn next(
            &self,
            coordinator: &mut AdmissionCoordinator,
            now: Instant,
        ) -> Option<AdmissionAction> {
            let desired: Vec<_> = self.physical.iter().cloned().collect();
            coordinator.next_action(
                &self.membership,
                ChatJoinMode::Live,
                &desired,
                &self.physical,
                &self.complete,
                now,
            )
        }
    }

    #[test]
    fn candidate_must_complete_before_any_healthy_room_is_displaced() {
        let mut rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        assert_eq!(
            rooms.next(&mut coordinator, now),
            Some(AdmissionAction::Join("visitor".into()))
        );
        rooms.physical.insert("visitor".into());
        assert_eq!(rooms.physical.len(), MAX_ROOMS);
        assert_eq!(rooms.next(&mut coordinator, now), None);
        rooms.complete.insert("visitor".into());
        let Some(AdmissionAction::Leave {
            login,
            displaced: true,
        }) = rooms.next(&mut coordinator, now)
        else {
            panic!("expected replacement after success");
        };
        assert_ne!(login, "visitor");
        assert_eq!(rooms.complete.len(), MAX_ROOMS);
        rooms.physical.remove(&login);
        rooms.complete.remove(&login);
        rooms.membership.displaced(&login);
        coordinator.cleanup_succeeded(&login);
        assert_eq!(rooms.complete.len(), STEADY_ROOMS);
        assert_eq!(rooms.next(&mut coordinator, now), None);
    }

    #[test]
    fn failed_partial_candidate_holds_the_probe_until_cleanup_succeeds() {
        let mut rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        rooms.physical.insert("visitor".into());
        rooms.membership.unavailable.insert("visitor".into());
        rooms.membership.foreground.insert("another".into());
        coordinator.require_cleanup("visitor");
        let cleanup = Some(AdmissionAction::Leave {
            login: "visitor".into(),
            displaced: false,
        });
        assert_eq!(rooms.next(&mut coordinator, now), cleanup);
        coordinator.cleanup_failed("visitor", "temporary deletion failure".into(), now);
        assert_eq!(
            rooms.next(&mut coordinator, now + Duration::from_secs(4)),
            None
        );
        assert_eq!(
            rooms.next(&mut coordinator, now + Duration::from_secs(5)),
            cleanup
        );
        assert_eq!(rooms.complete.len(), STEADY_ROOMS);
        rooms.physical.remove("visitor");
        coordinator.cleanup_succeeded("visitor");
        assert_eq!(
            rooms.next(&mut coordinator, now),
            Some(AdmissionAction::Join("another".into()))
        );
    }

    #[test]
    fn failed_replacement_cleanup_retains_its_displacement_semantics() {
        let mut rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        rooms.physical.insert("visitor".into());
        rooms.complete.insert("visitor".into());
        let action = rooms.next(&mut coordinator, now).unwrap();
        let AdmissionAction::Leave {
            ref login,
            displaced: true,
        } = action
        else {
            panic!("expected displacement");
        };
        coordinator.cleanup_failed(login, "temporary deletion failure".into(), now);
        assert_eq!(
            rooms.next(&mut coordinator, now + Duration::from_secs(5)),
            Some(action)
        );
    }

    #[test]
    fn canceled_candidate_is_cleaned_up_instead_of_displacing_a_healthy_room() {
        let mut rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        rooms.physical.insert("visitor".into());
        rooms.complete.insert("visitor".into());
        rooms.membership.foreground.clear();
        assert_eq!(
            rooms.next(&mut coordinator, Instant::now()),
            Some(AdmissionAction::Leave {
                login: "visitor".into(),
                displaced: true
            })
        );
    }

    #[test]
    fn room_backoff_doubles_and_caps_at_five_minutes() {
        assert_eq!(
            (1..=9)
                .map(|attempts| retry_delay(attempts).as_secs())
                .collect::<Vec<_>>(),
            [5, 10, 20, 40, 80, 160, 300, 300, 300]
        );
        assert_eq!(retry_delay(u32::MAX), Duration::from_secs(300));
    }

    #[test]
    fn priority_change_during_admission_preserves_the_committed_members() {
        let mut rooms = Rooms::full();
        rooms.membership.foreground.clear();
        rooms.membership.foreground.insert("aaa".into());
        rooms.membership.followed.insert("aaa".into(), true);
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        assert_eq!(
            rooms.next(&mut coordinator, now),
            Some(AdmissionAction::Join("aaa".into()))
        );
        rooms.membership.foreground.remove("aaa");
        rooms.membership.lingering.insert("aaa".into());
        assert!(!rooms.membership.would_admit("aaa", ChatJoinMode::Live));
        rooms.physical.insert("aaa".into());
        rooms.complete.insert("aaa".into());
        assert_eq!(
            rooms.next(&mut coordinator, now),
            Some(AdmissionAction::Leave {
                login: "aaa".into(),
                displaced: true
            })
        );
    }

    #[test]
    fn global_throttle_and_join_budget_apply_to_foreground_admissions() {
        let rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        for _ in 0..20 {
            assert!(coordinator.acquire_join(now));
        }
        assert!(!coordinator.acquire_join(now));
        assert!(coordinator.acquire_join(now + Duration::from_secs(10)));
        coordinator.throttle(now);
        assert_eq!(
            rooms.next(&mut coordinator, now + Duration::from_secs(59)),
            None
        );
        assert!(!coordinator.acquire_join(now + Duration::from_secs(59)));
        assert_eq!(
            rooms.next(&mut coordinator, now + Duration::from_secs(60)),
            Some(AdmissionAction::Join("visitor".into()))
        );
    }

    #[test]
    fn a_new_foreground_lease_reconsiders_a_queued_displacement() {
        let mut rooms = Rooms::full();
        let mut coordinator = AdmissionCoordinator::default();
        let now = Instant::now();
        rooms.physical.insert("visitor".into());
        rooms.complete.insert("visitor".into());
        let Some(AdmissionAction::Leave { login, .. }) = rooms.next(&mut coordinator, now) else {
            panic!("expected displacement");
        };
        coordinator.cleanup_failed(&login, "temporary deletion failure".into(), now);
        rooms.membership.foreground.insert(login.clone());
        let Some(AdmissionAction::Leave {
            login: replacement,
            displaced: true,
        }) = rooms.next(&mut coordinator, now + Duration::from_secs(5))
        else {
            panic!("expected a new displacement");
        };
        assert_ne!(replacement, login);
    }
}

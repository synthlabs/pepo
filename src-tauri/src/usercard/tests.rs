use super::*;

fn target(id: Option<&str>, login: &str, channel: &str) -> UserCardTarget {
    UserCardTarget {
        user_id: id.map(str::to_owned),
        login: login.into(),
        display_name: login.into(),
        channel_id: channel.into(),
        channel_login: channel.into(),
        anchor: CardAnchor::default(),
    }
}

fn insert(registry: &mut Registry, label: &str, target: UserCardTarget, activation: u64) {
    registry.cards.insert(
        label.into(),
        Card {
            state: UserCardState {
                user: ObservedUser {
                    user_id: target.user_id.clone().unwrap_or_default(),
                    login: target.login.clone(),
                    display_name: target.display_name.clone(),
                    color: None,
                },
                profile_image_url: None,
                channel_id: target.channel_id.clone(),
                channel_login: target.channel_login.clone(),
                badges: None,
                profile_status: ProfileStatus::Loading,
                error: None,
                pinned: false,
                chrome: UserCardChrome::Compact,
                revision: 0,
            },
            target,
            activation,
            armed: false,
            native_ready: false,
            request: 0,
            tasks: Vec::new(),
        },
    );
}

#[test]
fn deduplicates_known_and_unresolved_references_without_conflating_login_owners() {
    let mut registry = Registry::default();
    insert(&mut registry, "one", target(None, "maya", "a"), 1);
    assert_eq!(
        registry.find(&target(Some("1"), "MAYA", "b")).as_deref(),
        Some("one")
    );
    registry.resolve("one", "1", "maya");
    assert_eq!(
        registry.find(&target(Some("1"), "renamed", "b")).as_deref(),
        Some("one")
    );
    assert!(registry.find(&target(Some("2"), "maya", "b")).is_none());
}

#[test]
fn blur_policy_is_armed_by_focus_and_respects_pin_and_current_setting() {
    let mut registry = Registry::default();
    insert(&mut registry, "one", target(Some("1"), "maya", "a"), 1);
    assert!(!registry.focus("one", false, true));
    assert!(!registry.focus("one", true, true));
    assert!(registry.focus("one", false, true));
    registry.cards.get_mut("one").unwrap().state.pinned = true;
    assert!(!registry.focus("one", false, true));
    registry.cards.get_mut("one").unwrap().state.pinned = false;
    assert!(!registry.focus("one", false, false));
}

#[test]
fn lookup_alias_merge_keeps_latest_channel_and_pin() {
    let mut registry = Registry::default();
    insert(&mut registry, "known", target(Some("1"), "old", "a"), 1);
    insert(&mut registry, "lookup", target(None, "maya", "b"), 2);
    registry.cards.get_mut("lookup").unwrap().state.pinned = true;
    assert_eq!(
        registry.resolve("lookup", "1", "maya").as_deref(),
        Some("known")
    );
    assert_eq!(registry.cards.len(), 1);
    assert_eq!(registry.cards["known"].target.channel_id, "b");
    assert!(registry.cards["known"].state.pinned);
}

#[test]
fn placement_clamps_to_negative_monitor_origins_and_small_work_areas() {
    assert_eq!(
        fit_position(-2200.0, 1200.0, 360.0, 240.0, (-1920.0, 0.0, 0.0, 1040.0)),
        (-1920, 800)
    );
    assert_eq!(
        fit_position(500.0, 500.0, 360.0, 240.0, (0.0, 0.0, 300.0, 200.0)),
        (0, 0)
    );
}

use super::{irc::IrcNoticeProvider, NoticeProvider, NoticeSink, ProviderHealth, NOTICE_CAPACITY};
use crate::chat::ChatEngine;
use crate::network::{Failure, NetworkRuntime};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, watch};

pub async fn consume(engine: Arc<ChatEngine>, mut events: mpsc::Receiver<super::NoticeEvent>) {
    while let Some(event) = events.recv().await {
        if !engine.active() {
            break;
        }
        engine.insert_notice(event);
    }
}

pub fn start(
    app: &AppHandle,
    engine: Arc<ChatEngine>,
) -> Vec<tauri::async_runtime::JoinHandle<()>> {
    let settings_app = app.clone();
    let settings = Arc::new(move || {
        crate::app_settings(settings_app.state::<crate::StateSyncer>().inner()).eventsub
    });
    let provider: Box<dyn NoticeProvider> = Box::new(IrcNoticeProvider::new(settings));
    let desired = engine.notice_roster();
    let (health, mut health_rx) = watch::channel(ProviderHealth::default());
    let (sender, receiver) = mpsc::channel(NOTICE_CAPACITY);
    let sink = NoticeSink::new(sender, health.clone());
    let provider_task = tauri::async_runtime::spawn(provider.run(desired, sink, health));
    let consumer_task = tauri::async_runtime::spawn(consume(engine.clone(), receiver));
    let app = app.clone();
    let health_task = tauri::async_runtime::spawn(async move {
        let mut seen_gaps = 0;
        let mut last_issue = None;
        let mut clear_gap_at = None;
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                result = health_rx.changed() => { if result.is_err() { break; } }
                _ = timer.tick() => {}
            }
            if !engine.active() {
                break;
            }
            let status = health_rx.borrow_and_update().clone();
            let network = app.state::<NetworkRuntime>();
            if engine.notice_roster().borrow().is_empty() {
                network.recovered(&app, "chat.notices.connection");
                network.recovered(&app, "chat.notices.gap");
                seen_gaps = status.coverage_gaps;
                clear_gap_at = None;
                last_issue = None;
                continue;
            }
            if last_issue != status.issue {
                if let Some(issue) = &status.issue {
                    network.failed(
                        &app,
                        "chat.notices.connection",
                        "Channel notices",
                        &Failure::new("connection", issue.clone(), true),
                        false,
                        false,
                        None,
                    );
                } else {
                    network.recovered(&app, "chat.notices.connection");
                }
                last_issue = status.issue.clone();
            }
            if status.coverage_gaps != seen_gaps {
                seen_gaps = status.coverage_gaps;
                clear_gap_at = Some(tokio::time::Instant::now() + Duration::from_secs(10));
                network.failed(
                    &app,
                    "chat.notices.gap",
                    "Channel notices",
                    &Failure::new(
                        "coverage_gap",
                        "Some channel notices were missed while the notice queue was full.",
                        true,
                    ),
                    false,
                    false,
                    None,
                );
            } else if clear_gap_at.is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
                && status.issue.is_none()
            {
                network.recovered(&app, "chat.notices.gap");
                clear_gap_at = None;
            }
        }
    });
    vec![provider_task, consumer_task, health_task]
}

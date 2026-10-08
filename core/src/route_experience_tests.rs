use super::*;
use crate::{
    model::{CreateOptions, NoticeKind},
    route_policy::RoutePolicy,
};

fn fast_policy() -> RoutePolicy {
    RoutePolicy {
        recovery_budget: Duration::from_millis(400),
        backoff_base: Duration::from_millis(30),
        backoff_cap: Duration::from_millis(60),
        tick: Duration::from_millis(10),
        slow_window: Duration::from_millis(600),
        probe_interval: Duration::from_millis(50),
        ..Default::default()
    }
}

#[tokio::test]
async fn temporary_probe_failure_rejoins_candidates_and_finishes_without_failure_notice() {
    let fixture = Fixture::new(Mode::RecoverAfterProbe, 40_000).await;
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let done = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Completed
    })
    .await;
    assert!(fixture.requests.lock().unwrap().len() >= 3);
    assert_eq!(
        done.notices
            .iter()
            .filter(|notice| notice.kind == NoticeKind::DownloadFailed)
            .count(),
        0
    );
    assert!(done.tasks[0].details.recovery_info.is_none());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn exhausted_recovery_emits_one_failure_and_does_not_retry_forever() {
    let fixture = Fixture::new(Mode::Unavailable, 40_000).await;
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let waiting = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::WaitingNetwork
    })
    .await;
    assert!(waiting.notices.is_empty());
    assert!(!waiting.tasks[0].details.route_failures.is_empty());
    let done = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Failed
    })
    .await;
    assert!(done.tasks[0].error.as_ref().unwrap().contains("恢复额度"));
    assert_eq!(
        done.notices
            .iter()
            .filter(|notice| notice.kind == NoticeKind::DownloadFailed)
            .count(),
        1
    );
    let requests = fixture.requests.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fixture.requests.lock().unwrap().len(), requests);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn waiting_task_yields_and_budget_freezes_while_another_task_downloads() {
    let fixture = Fixture::new(Mode::SelectiveUnavailable, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let mut policy = fast_policy();
    policy.backoff_base = Duration::from_millis(250);
    policy.backoff_cap = Duration::from_millis(250);
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        policy,
    )
    .unwrap();
    let bad = SOURCE.replace("file.bin", "blocked.bin");
    manager.create(bad, root.path().into()).await.unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::WaitingNetwork
    })
    .await;
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let during = wait_for(&manager, |snapshot| snapshot.tasks[1].downloaded > 0).await;
    let remaining = during.tasks[0]
        .details
        .recovery_info
        .as_ref()
        .unwrap()
        .remaining_ms;
    let elapsed = during.tasks[0].details.elapsed_ms;
    assert!(
        during.tasks[0]
            .details
            .recovery_info
            .as_ref()
            .unwrap()
            .waiting_for_slot
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let later = manager.snapshot().await.unwrap();
    assert_eq!(
        later.tasks[0]
            .details
            .recovery_info
            .as_ref()
            .unwrap()
            .remaining_ms,
        remaining
    );
    assert_eq!(later.tasks[0].details.elapsed_ms, elapsed);
    manager
        .action(later.tasks[0].id.clone(), Action::Pause)
        .await
        .unwrap();
    let completed = wait_for(&manager, |snapshot| {
        snapshot.tasks[1].status == TaskStatus::Completed
    })
    .await;
    assert_eq!(completed.tasks[0].status, TaskStatus::Paused);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn immediate_retry_respects_server_cooldown_and_preserves_budget() {
    let fixture = Fixture::new(Mode::RateLimit, 40_000).await;
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let waiting = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::WaitingNetwork
    })
    .await;
    assert!(
        waiting.tasks[0]
            .details
            .recovery_info
            .as_ref()
            .unwrap()
            .retry_in_ms
            > 50_000
    );
    let before = waiting.tasks[0]
        .details
        .recovery_info
        .as_ref()
        .unwrap()
        .remaining_ms;
    let requests = fixture.requests.lock().unwrap().len();
    manager
        .action(waiting.tasks[0].id.clone(), Action::Resume)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let after = manager.snapshot().await.unwrap();
    assert_eq!(fixture.requests.lock().unwrap().len(), requests);
    assert!(
        after.tasks[0]
            .details
            .recovery_info
            .as_ref()
            .unwrap()
            .remaining_ms
            < before
    );
    manager
        .action(waiting.tasks[0].id.clone(), Action::Pause)
        .await
        .unwrap();
    assert_eq!(
        manager.snapshot().await.unwrap().tasks[0].status,
        TaskStatus::Paused
    );
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn waiting_cancel_and_shutdown_stop_recovery_and_restart_needs_user() {
    let fixture = Fixture::new(Mode::Unavailable, 40_000).await;
    for cancel in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("tasks.sqlite3");
        let manager =
            Manager::start_with_policy(&db, fixture.engine(), Arc::new(|_| {}), fast_policy())
                .unwrap();
        manager
            .create(SOURCE.into(), root.path().into())
            .await
            .unwrap();
        let waiting = wait_for(&manager, |snapshot| {
            snapshot.tasks[0].status == TaskStatus::WaitingNetwork
        })
        .await;
        std::fs::write(root.path().join("keep.txt"), b"keep").unwrap();
        if cancel {
            manager
                .action(waiting.tasks[0].id.clone(), Action::Cancel)
                .await
                .unwrap();
        }
        manager.shutdown().await.unwrap();
        let requests = fixture.requests.lock().unwrap().len();
        let restarted = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
        let state = restarted.snapshot().await.unwrap();
        assert_eq!(
            state.tasks[0].status,
            if cancel {
                TaskStatus::Cancelled
            } else {
                TaskStatus::Paused
            }
        );
        assert!(state.tasks[0].details.recovery_info.is_none());
        assert!(state.tasks[0].details.retry_info.is_none());
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(fixture.requests.lock().unwrap().len(), requests);
        assert_eq!(
            std::fs::read(root.path().join("keep.txt")).unwrap(),
            b"keep"
        );
        restarted.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn fixed_routes_do_not_fail_over_and_permanent_errors_do_not_wait() {
    let bad = Fixture::new(Mode::Unavailable, 40_000).await;
    let good = Fixture::new(Mode::Range, 40_000).await;
    let mut engine = bad.engine();
    let mut fallback = good.network.routes[0].clone();
    fallback.id = "fallback".into();
    engine.network.routes.push(fallback);
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        engine,
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create_with(
            SOURCE.into(),
            root.path().into(),
            CreateOptions {
                preferred_route: Some("fixture".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Failed
    })
    .await;
    assert!(good.requests.lock().unwrap().is_empty());
    manager.shutdown().await.unwrap();
    for mode in [Mode::NotFound, Mode::BadDigest, Mode::Html] {
        let fixture = Fixture::new(mode, 40_000).await;
        let directory = tempfile::tempdir().unwrap();
        let manager = Manager::start_with_policy(
            &directory.path().join("tasks.sqlite3"),
            fixture.engine(),
            Arc::new(|_| {}),
            fast_policy(),
        )
        .unwrap();
        manager
            .create(SOURCE.into(), directory.path().into())
            .await
            .unwrap();
        let failed = wait_for(&manager, |snapshot| {
            snapshot.tasks[0].status == TaskStatus::Failed
        })
        .await;
        assert!(failed.tasks[0].details.recovery_info.is_none());
        assert!(!failed.tasks[0].error.as_ref().unwrap().contains("恢复额度"));
        manager.shutdown().await.unwrap();
    }
}

async fn slow_download() -> (Fixture, Fixture, tempfile::TempDir, Manager) {
    let slow = Fixture::new(Mode::Crawl, 12 * 1024 * 1024).await;
    let good = Fixture::new(Mode::Range, 12 * 1024 * 1024).await;
    let mut engine = slow.engine();
    let mut candidate = good.network.routes[0].clone();
    candidate.id = "faster".into();
    candidate.name = "备选测试线路".into();
    candidate.prefix = candidate
        .prefix
        .map(|prefix| prefix.replace("/download/", "/slow-probe/"));
    engine.network.routes.push(candidate);
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        engine,
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    (slow, good, root, manager)
}

#[tokio::test]
async fn confirmed_suggestion_revalidates_restarts_without_pinning_and_finishes_correctly() {
    let (_slow, good, _root, manager) = slow_download().await;
    let proposed = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].details.route_suggestion.is_some()
    })
    .await;
    let suggestion = proposed.tasks[0]
        .details
        .route_suggestion
        .as_ref()
        .unwrap()
        .clone();
    assert!(suggestion.restart_bytes > 0);
    manager
        .apply_route_suggestion(proposed.tasks[0].id.clone(), suggestion.id.clone())
        .await
        .unwrap();
    let completed = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Completed
    })
    .await;
    assert!(completed.tasks[0].details.preferred_route.is_none());
    assert_eq!(
        std::fs::read(completed.tasks[0].final_path.as_ref().unwrap()).unwrap(),
        *good.bytes
    );
    assert!(manager
        .apply_route_suggestion(proposed.tasks[0].id.clone(), suggestion.id)
        .await
        .is_err());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn dismissing_suggestion_suppresses_probes_without_stopping_current_download() {
    let (_slow, good, _root, manager) = slow_download().await;
    let proposed = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].details.route_suggestion.is_some()
    })
    .await;
    let suggestion = proposed.tasks[0].details.route_suggestion.as_ref().unwrap();
    manager
        .dismiss_route_suggestion(proposed.tasks[0].id.clone(), suggestion.id.clone())
        .await
        .unwrap();
    let requests = good.requests.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(750)).await;
    let later = manager.snapshot().await.unwrap();
    assert_eq!(good.requests.lock().unwrap().len(), requests);
    assert!(later.tasks[0].downloaded > proposed.tasks[0].downloaded);
    assert!(later.tasks[0].details.route_suggestion.is_none());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_confirmation_and_pause_during_recheck_preserve_original_parts() {
    let (_slow, good, root, manager) = slow_download().await;
    let proposed = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].details.route_suggestion.is_some()
    })
    .await;
    let task = &proposed.tasks[0];
    let suggestion = task.details.route_suggestion.as_ref().unwrap();
    let gate = good.hold_probe("file.bin");
    let other = manager.clone();
    let id = task.id.clone();
    let suggestion_id = suggestion.id.clone();
    let pending =
        tokio::spawn(async move { other.apply_route_suggestion(id, suggestion_id).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(manager
        .apply_route_suggestion(task.id.clone(), suggestion.id.clone())
        .await
        .is_err());
    manager
        .action(task.id.clone(), Action::Pause)
        .await
        .unwrap();
    assert!(pending.await.unwrap().is_err());
    gate.add_permits(2);
    let paused = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Paused
    })
    .await;
    assert!(paused.tasks[0].downloaded >= task.downloaded);
    assert!(root.path().join(format!(".githubsp-{}", task.id)).exists());
    assert!(paused.tasks[0].details.preferred_route.is_none());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn unavailable_candidate_during_confirmation_keeps_original_download() {
    let (_slow, good, _root, manager) = slow_download().await;
    let proposed = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].details.route_suggestion.is_some()
    })
    .await;
    good.server.abort();
    tokio::task::yield_now().await;
    let task = &proposed.tasks[0];
    assert!(manager
        .apply_route_suggestion(
            task.id.clone(),
            task.details.route_suggestion.as_ref().unwrap().id.clone()
        )
        .await
        .is_err());
    let preserved = manager.snapshot().await.unwrap();
    assert_eq!(preserved.tasks[0].status, TaskStatus::Downloading);
    assert!(preserved.tasks[0].downloaded >= task.downloaded);
    assert_eq!(preserved.tasks[0].route, task.route);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn multiple_waiting_tasks_are_bounded_and_both_remain_cancellable() {
    let fixture = Fixture::new(Mode::Unavailable, 40_000).await;
    let root = tempfile::tempdir().unwrap();
    let second = root.path().join("second");
    std::fs::create_dir(&second).unwrap();
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        fast_policy(),
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::WaitingNetwork
    })
    .await;
    manager.create(SOURCE.into(), second).await.unwrap();
    let both = wait_for(&manager, |snapshot| {
        snapshot
            .tasks
            .iter()
            .all(|task| task.status == TaskStatus::WaitingNetwork)
    })
    .await;
    assert_eq!(both.tasks.len(), 2);
    let done = wait_for(&manager, |snapshot| {
        snapshot
            .tasks
            .iter()
            .all(|task| task.status == TaskStatus::Failed)
    })
    .await;
    assert_eq!(
        done.notices
            .iter()
            .filter(|notice| notice.kind == NoticeKind::DownloadFailed)
            .count(),
        2
    );
    for task in done.tasks {
        manager.action(task.id, Action::Cancel).await.unwrap();
    }
    manager.shutdown().await.unwrap();
}

#[test]
fn interrupted_waiting_state_restores_paused_without_stale_session_context() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    let mut value = record(root.path());
    value.task.status = TaskStatus::WaitingNetwork;
    value.task.details.elapsed_ms = Some(1200);
    value.task.details.retry_info = Some(crate::model::RetryInfo {
        phase: "recovering".into(),
        attempt: 0,
        max_attempts: 0,
        reason: "等待网络".into(),
        retry_in_ms: 3000,
    });
    value.task.details.recovery_info = Some(crate::model::RecoveryInfo {
        remaining_ms: 100_000,
        retry_in_ms: 3000,
        waiting_for_slot: false,
    });
    store.save(&value).unwrap();
    let restored = store.recover().unwrap().remove(0);
    assert_eq!(restored.task.status, TaskStatus::Paused);
    assert_eq!(restored.task.details.elapsed_ms, Some(1200));
    assert!(!restored.task.details.elapsed_is_partial);
    assert!(restored.task.details.retry_info.is_none());
    assert!(restored.task.details.recovery_info.is_none());
}

#[tokio::test]
async fn waiting_cancel_cleanup_failure_stops_recovery_and_keeps_unknown_files() {
    let fixture = Fixture::new(Mode::Unavailable, 40000).await;
    let root = tempfile::tempdir().unwrap();
    let mut policy = fast_policy();
    policy.recovery_budget = Duration::from_secs(2);
    policy.backoff_base = Duration::from_secs(1);
    policy.backoff_cap = Duration::from_secs(1);
    let manager = Manager::start_with_policy(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
        policy,
    )
    .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let waiting = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::WaitingNetwork
    })
    .await;
    let id = waiting.tasks[0].id.clone();
    let workspace = Workspace::new(root.path(), &id).unwrap();
    let unknown = workspace.path.join("keep.txt");
    std::fs::write(&unknown, b"keep").unwrap();
    assert!(manager.action(id, Action::Cancel).await.is_err());
    let stopped = manager.snapshot().await.unwrap();
    assert_eq!(stopped.tasks[0].status, TaskStatus::Failed);
    assert!(stopped.tasks[0].details.recovery_info.is_none());
    assert!(stopped.tasks[0]
        .error
        .as_ref()
        .unwrap()
        .contains("清理失败"));
    assert_eq!(std::fs::read(unknown).unwrap(), b"keep");
    manager.shutdown().await.unwrap();
}

use super::*;
use crate::model::DiagnosticSource;

fn url(filename: &str) -> String {
    SOURCE.replace("file.bin", filename)
}

fn diagnose(
    manager: &Manager,
    target: &str,
) -> tokio::task::JoinHandle<crate::error::Result<Snapshot>> {
    let manager = manager.clone();
    let target = target.to_string();
    tokio::spawn(async move { manager.diagnose(&target).await })
}

#[tokio::test]
async fn empty_and_explicit_targets_do_not_create_downloads_and_old_cache_has_no_inferred_source() {
    let fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("diagnostics.sqlite3");
    let manager = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    for input in ["", " \t\n"] {
        let snapshot = manager.diagnose(input).await.unwrap();
        let context = snapshot.diagnostic_context.unwrap();
        assert_eq!(context.source, DiagnosticSource::Default);
        assert_eq!(context.filename, "GitHubSP-v0.2.2-windows-x64.exe");
        assert!(snapshot.diagnostics[0].available);
        assert!(!snapshot.diagnosing);
        assert!(snapshot.tasks.is_empty());
    }
    let target = url("DLSS5-Swapper-Setup-2.2.9.exe");
    let result = manager.diagnose(&format!(" {target} ")).await.unwrap();
    let context = result.diagnostic_context.unwrap();
    assert_eq!(context.source, DiagnosticSource::Input);
    assert_eq!(context.filename, "DLSS5-Swapper-Setup-2.2.9.exe");
    for input in [
        "https://github.com/test/repo",
        "https://github.com/test/repo/releases/latest",
        "http://github.com/test/repo/releases/download/v1/file.zip",
        "invalid",
    ] {
        let error = manager.diagnose(input).await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidInput);
        assert!(error
            .message
            .contains("请输入附件直链，或清空输入后使用默认测试文件"));
        assert!(!manager.snapshot().await.unwrap().diagnosing);
    }
    assert!(std::fs::read_dir(root.path()).unwrap().all(|entry| {
        let entry = entry.unwrap();
        !entry.file_type().unwrap().is_dir()
            && !entry.file_name().to_string_lossy().ends_with(".exe")
    }));
    manager.shutdown().await.unwrap();
    let restarted = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    let restored = restarted.snapshot().await.unwrap();
    assert!(restored.diagnostic_context.is_none());
    assert!(restored.diagnostics[0].available);
    assert!(restored.diagnostics[0].checked_at > 0);
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn manual_detection_keeps_download_progress_queue_pause_cancel_and_resume_working() {
    let fixture = Fixture::new(Mode::Slow, 1024 * 1024).await;
    let gate = fixture.hold_probe("manual-gated.bin");
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("queue.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let created = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let first = created.tasks[0].id.clone();
    let before = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    let worker = diagnose(&manager, &url("manual-gated.bin"));
    wait_for(&manager, |s| s.diagnosing).await;
    let second = manager
        .create(url("v2rayN-windows-64-desktop.zip"), root.path().into())
        .await
        .unwrap()
        .tasks[1]
        .id
        .clone();
    let third = manager
        .create(url("cancelled.zip"), root.path().into())
        .await
        .unwrap()
        .tasks[2]
        .id
        .clone();
    let progressing = wait_for(&manager, |s| {
        s.tasks[0].downloaded > before.tasks[0].downloaded
    })
    .await;
    assert!(progressing.diagnosing);
    assert_eq!(progressing.tasks[0].route, before.tasks[0].route);
    assert_eq!(progressing.tasks[1].status, TaskStatus::Queued);
    manager.action(third, Action::Cancel).await.unwrap();
    manager.action(first.clone(), Action::Pause).await.unwrap();
    let continued = wait_for(&manager, |s| s.tasks[1].status == TaskStatus::Completed).await;
    assert!(continued.diagnosing);
    assert_eq!(continued.tasks[0].status, TaskStatus::Paused);
    assert_eq!(continued.tasks[2].status, TaskStatus::Cancelled);
    assert_eq!(
        continued.diagnostic_context.unwrap().filename,
        "manual-gated.bin"
    );
    assert_eq!(continued.tasks[1].id, second);
    manager.action(first, Action::Resume).await.unwrap();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Completed).await;
    gate.add_permits(1);
    let completed = worker.await.unwrap().unwrap();
    assert!(!completed.diagnosing);
    assert!(completed.diagnostics[0].available);
    assert!(completed.error.is_none());
    assert_eq!(
        completed.diagnostic_context.unwrap().source,
        DiagnosticSource::Input
    );
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_detection_is_rejected_and_an_abandoned_caller_does_not_leave_it_busy() {
    let fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
    let gate = fixture.hold_probe("manual-gated.bin");
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("duplicate.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let caller = diagnose(&manager, &url("manual-gated.bin"));
    wait_for(&manager, |s| s.diagnosing).await;
    let error = manager.diagnose(SOURCE).await.unwrap_err();
    assert!(error.message.contains("线路检测正在进行"));
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    gate.add_permits(1);
    wait_for(&manager, |s| !s.diagnosing).await;
    assert!(manager.diagnose(SOURCE).await.unwrap().diagnostics[0].available);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn automatic_results_started_before_or_during_manual_detection_cannot_replace_it() {
    for automatic_first in [true, false] {
        let fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
        let automatic_gate = fixture.hold_probe("automatic-gated.bin");
        let manual_gate = fixture.hold_probe("manual-gated.bin");
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::start(
            &root.path().join("generation.sqlite3"),
            fixture.engine(),
            Arc::new(|_| {}),
        )
        .unwrap();
        if automatic_first {
            manager
                .create(url("automatic-gated.bin"), root.path().into())
                .await
                .unwrap();
            wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Probing).await;
        }
        let manual = diagnose(&manager, &url("manual-gated.bin"));
        wait_for(&manager, |s| s.diagnosing).await;
        if !automatic_first {
            manager
                .create(url("automatic-gated.bin"), root.path().into())
                .await
                .unwrap();
            wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Probing).await;
        }
        manual_gate.add_permits(1);
        let measured = manual.await.unwrap().unwrap();
        let checked_at = measured.diagnostics[0].checked_at;
        automatic_gate.add_permits(1);
        let done = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Completed).await;
        assert_eq!(
            done.diagnostic_context.unwrap().source,
            DiagnosticSource::Input
        );
        assert_eq!(done.diagnostics[0].checked_at, checked_at);
        manager
            .create(url("DLSS5-Swapper-Setup-2.2.9.exe"), root.path().into())
            .await
            .unwrap();
        let next = wait_for(&manager, |s| s.tasks[1].status == TaskStatus::Completed).await;
        let context = next.diagnostic_context.unwrap();
        assert_eq!(context.source, DiagnosticSource::Download);
        assert_eq!(context.filename, "DLSS5-Swapper-Setup-2.2.9.exe");
        manager.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn each_detection_batch_resets_unchecked_routes_instead_of_mixing_previous_results() {
    let mut fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
    let mut second = fixture.network.routes[0].clone();
    second.id = "second".into();
    fixture.network.routes.push(second);
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("batch.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    assert!(manager
        .diagnose(SOURCE)
        .await
        .unwrap()
        .diagnostics
        .iter()
        .all(|r| r.available));
    manager
        .create_with(
            SOURCE.into(),
            root.path().into(),
            crate::model::CreateOptions {
                preferred_route: Some("fixture".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let done = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert!(done.diagnostics[0].available);
    assert_eq!(done.diagnostics[1].checked_at, 0);
    assert!(!done.diagnostics[1].available);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn timeout_partial_and_all_failures_release_detection_and_do_not_fail_the_download() {
    let mut good = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let bad = Fixture::new(Mode::NotFound, 0).await;
    good.hold_probe("timeout.bin");
    good.network.probe_timeout = Duration::from_millis(200);
    let mut bad_route = bad.network.routes[0].clone();
    bad_route.id = "unavailable".into();
    good.network.routes.push(bad_route);
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("failure.sqlite3"),
        good.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    manager
        .create_with(
            SOURCE.into(),
            root.path().into(),
            crate::model::CreateOptions {
                preferred_route: Some("fixture".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    let failed = manager.diagnose(&url("timeout.bin")).await.unwrap();
    assert!(!failed.diagnosing);
    assert!(failed.diagnostics.iter().all(|r| !r.available));
    assert!(failed.diagnostics[0]
        .error
        .as_ref()
        .unwrap()
        .contains("线路探测超时"));
    assert_eq!(failed.tasks[0].status, TaskStatus::Downloading);
    assert!(failed.tasks[0].error.is_none());
    assert!(failed.error.is_none());
    let retried = manager.diagnose(SOURCE).await.unwrap();
    assert!(retried.diagnostics[0].available);
    assert!(!retried.diagnostics[1].available);
    assert!(!retried.diagnosing);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_detection_and_waits_for_its_worker_to_stop() {
    let fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
    fixture.hold_probe("manual-gated.bin");
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("shutdown.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let caller = diagnose(&manager, &url("manual-gated.bin"));
    wait_for(&manager, |s| s.diagnosing).await;
    let stopped = tokio::time::timeout(Duration::from_secs(2), manager.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(!stopped.diagnosing);
    assert_eq!(
        caller.await.unwrap().unwrap_err().kind,
        ErrorKind::Cancelled
    );
}

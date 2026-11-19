use crate::{
    engine::{Engine, Reporter},
    error::ErrorKind,
    files::{collision_name, Workspace},
    manager::{Action, Manager},
    model::{Snapshot, StoredTask, Task, TaskStatus, Verification},
    network::{parse_content_range, Network, Route},
    source::parse_release_url,
    store::Store,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;

const SOURCE: &str = "https://github.com/test/repo/releases/download/v1/file.bin";

#[path = "diagnostics_tests.rs"]
mod diagnostics_tests;

#[tokio::test]
async fn elapsed_time_survives_pause_restart_resume_and_freezes_after_completion() {
    let fixture = Fixture::new(Mode::Slow, 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("elapsed.sqlite3");
    let manager = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    let created = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let first = created.tasks[0].id.clone();
    assert_eq!(created.tasks[0].details.elapsed_ms, Some(0));
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    let queued = manager
        .create(SOURCE.replace("file.bin", "queued.bin"), root.path().into())
        .await
        .unwrap();
    let second = queued.tasks[1].id.clone();
    assert_eq!(queued.tasks[1].details.elapsed_ms, Some(0));
    manager.action(second.clone(), Action::Pause).await.unwrap();
    manager.action(first.clone(), Action::Pause).await.unwrap();
    let paused = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Paused).await;
    let elapsed = paused.tasks[0].details.elapsed_ms.unwrap();
    assert!(elapsed > 0);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let still_paused = manager.snapshot().await.unwrap();
    assert_eq!(still_paused.tasks[0].details.elapsed_ms, Some(elapsed));
    assert_eq!(still_paused.tasks[1].details.elapsed_ms, Some(0));
    manager.shutdown().await.unwrap();
    let restarted = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    let saved = restarted.snapshot().await.unwrap();
    assert_eq!(saved.tasks[0].details.elapsed_ms, Some(elapsed));
    assert!(!saved.tasks[0].details.elapsed_is_partial);
    assert!(saved.notices.is_empty());
    restarted
        .action(first.clone(), Action::Resume)
        .await
        .unwrap();
    let done = wait_for(&restarted, |s| s.tasks[0].status == TaskStatus::Completed).await;
    let total = done.tasks[0].details.elapsed_ms.unwrap();
    assert!(total > elapsed);
    assert_eq!(
        done.notices[0].kind,
        crate::model::NoticeKind::DownloadCompleted
    );
    assert_eq!(done.notices[0].task_id.as_deref(), Some(first.as_str()));
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(
        restarted.snapshot().await.unwrap().tasks[0]
            .details
            .elapsed_ms,
        Some(total)
    );
    let again = restarted
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    assert_eq!(again.tasks[2].details.elapsed_ms, Some(0));
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn elapsed_time_failure_retry_keeps_prior_execution_and_old_tasks_stay_unknown() {
    let failing = Fixture::new(Mode::NotFound, 40000).await;
    let good = Fixture::new(Mode::Retry, 40000).await;
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("retry.sqlite3");
    let manager = Manager::start(&db, failing.engine(), Arc::new(|_| {})).unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    let failed = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Failed).await;
    let before = failed.tasks[0].details.elapsed_ms.unwrap();
    manager.shutdown().await.unwrap();
    let restarted = Manager::start(&db, good.engine(), Arc::new(|_| {})).unwrap();
    restarted
        .action(failed.tasks[0].id.clone(), Action::Resume)
        .await
        .unwrap();
    let completed = wait_for(&restarted, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert!(completed.tasks[0].details.elapsed_ms.unwrap() > before);
    restarted.shutdown().await.unwrap();
    let legacy_root = tempfile::tempdir().unwrap();
    let legacy_db = legacy_root.path().join("old.sqlite3");
    let mut old = record(legacy_root.path());
    old.task.status = TaskStatus::Paused;
    Store::open(&legacy_db).unwrap().save(&old).unwrap();
    let legacy = Manager::start(&legacy_db, good.engine(), Arc::new(|_| {})).unwrap();
    legacy.action(old.task.id, Action::Resume).await.unwrap();
    let completed = wait_for(&legacy, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert!(completed.tasks[0].details.elapsed_ms.is_none());
    legacy.shutdown().await.unwrap();
}

#[tokio::test]
async fn elapsed_time_is_checkpointed_when_transfer_waits_for_bandwidth() {
    let fixture = Fixture::new(Mode::Range, 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("checkpoint.sqlite3");
    let manager = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    manager
        .settings(crate::model::Settings {
            limit_kib: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    let connection =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    tokio::time::timeout(Duration::from_secs(9), async {
        loop {
            let payload: String = connection
                .query_row("SELECT payload FROM tasks", [], |row| row.get(0))
                .unwrap();
            let record: StoredTask = serde_json::from_str(&payload).unwrap();
            if record.task.details.elapsed_ms.unwrap() >= 4000 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let live = manager.snapshot().await.unwrap();
    assert!(live.tasks[0].details.elapsed_ms.unwrap() >= 4000);
    manager.shutdown().await.unwrap();
    let restored = Store::open(&db).unwrap().recover().unwrap();
    assert!(!restored[0].task.details.elapsed_is_partial);
    assert!(
        restored[0].task.details.elapsed_ms.unwrap() >= live.tasks[0].details.elapsed_ms.unwrap()
    );
}

#[test]
fn elapsed_time_recovery_marks_only_interrupted_execution_and_defaults_settings() {
    for status in [
        TaskStatus::Probing,
        TaskStatus::Downloading,
        TaskStatus::Retrying,
        TaskStatus::Verifying,
        TaskStatus::Pausing,
        TaskStatus::Cancelling,
        TaskStatus::Queued,
        TaskStatus::Paused,
        TaskStatus::Failed,
        TaskStatus::Completed,
        TaskStatus::Cancelled,
    ] {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(&root.path().join("recover.sqlite3")).unwrap();
        let mut value = record(root.path());
        value.task.status = status;
        value.task.details.elapsed_ms = Some(1530);
        store.save(&value).unwrap();
        let recovered = store.recover().unwrap().remove(0);
        assert_eq!(recovered.task.details.elapsed_ms, Some(1530));
        assert_eq!(recovered.task.details.elapsed_is_partial, status.running());
        assert_eq!(
            store.recover().unwrap()[0].task.details.elapsed_is_partial,
            status.running()
        );
    }
    let old: crate::model::Settings =
        serde_json::from_str(r#"{"limitKib":512,"closeToTray":true}"#).unwrap();
    assert!(old.background_completion_notice);
    let disabled: crate::model::Settings =
        serde_json::from_str(r#"{"backgroundCompletionNotice":false}"#).unwrap();
    assert!(!disabled.background_completion_notice);
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Range,
    Full,
    NoLength,
    IgnoreRange,
    BadRange,
    Truncated,
    Html,
    BadDigest,
    Retry,
    RateLimit,
    ChangedEtag,
    Slow,
    NoDigest,
    Timeout,
    NotFound,
    Unsatisfiable,
    NoMetadata,
    Redirect,
    Unavailable,
    RecoverAfterProbe,
    SelectiveUnavailable,
    Crawl,
    /// 测速请求也很慢，模拟低于 50 KB/s 的网络。
    CrawlProbe,
}

struct Fixture {
    network: Network,
    bytes: Arc<Vec<u8>>,
    requests: Arc<Mutex<Vec<String>>>,
    probe_gates: Arc<Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Fixture {
    async fn new(mode: Mode, size: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let base = format!("http://{address}");
        let bytes = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let requests = Arc::new(Mutex::new(Vec::new()));
        let data = bytes.clone();
        let log = requests.clone();
        let probe_gates = Arc::new(Mutex::new(HashMap::new()));
        let gates = probe_gates.clone();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((socket, _)) = result else { break; };
                        let data = data.clone();
                        let log = log.clone();
                        let gates = gates.clone();
                        handlers.spawn(async move { serve(socket, mode, data, log, gates).await });
                    }
                    _ = handlers.join_next(), if !handlers.is_empty() => (),
                }
            }
        });
        let network = Network {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            routes: vec![Route {
                id: "fixture".into(),
                name: "本地测试线路".into(),
                prefix: Some(format!("{base}/download/")),
            }],
            api_base: format!("{base}/api"),
            probe_timeout: Duration::from_secs(3),
            idle_timeout: Duration::from_millis(300),
            retry_delay: Duration::from_millis(1),
        };
        Self {
            network,
            bytes,
            requests,
            probe_gates,
            server,
        }
    }

    fn engine(&self) -> Engine {
        Engine {
            network: self.network.clone(),
            limiter: Arc::new(crate::limiter::RateLimiter::default()),
            connections: Arc::default(),
        }
    }

    fn hold_probe(&self, filename: &str) -> Arc<tokio::sync::Semaphore> {
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        self.probe_gates
            .lock()
            .unwrap()
            .insert(filename.into(), gate.clone());
        gate
    }
}

async fn serve(
    mut socket: TcpStream,
    mode: Mode,
    data: Arc<Vec<u8>>,
    requests: Arc<Mutex<Vec<String>>>,
    probe_gates: Arc<Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>>,
) {
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let Ok(count) = socket.read(&mut buffer).await else {
            return;
        };
        if count == 0 || request.len() > 16384 {
            return;
        }
        request.extend_from_slice(&buffer[..count]);
    }
    let request = String::from_utf8_lossy(&request);
    if request.starts_with("GET /api/") {
        if mode == Mode::NoMetadata {
            let _ = socket
                .write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n")
                .await;
            return;
        }
        let digest = if mode == Mode::NoDigest {
            None
        } else if mode == Mode::BadDigest {
            Some(format!("sha256:{}", "0".repeat(64)))
        } else {
            Some(format!("sha256:{:x}", Sha256::digest(&*data)))
        };
        let body = serde_json::to_vec(&serde_json::json!({"id":1,"tag_name":"v1","name":"v1","prerelease":false,"html_url":"https://github.com/test/repo/releases/tag/v1","body":"测试版本","assets":[{"id":1,"name":"file.bin","browser_download_url":SOURCE,"size":data.len(),"digest":digest}]})).unwrap();
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len());
        if socket.write_all(head.as_bytes()).await.is_ok() {
            let _ = socket.write_all(&body).await;
        }
        return;
    }
    let range = request.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("range: bytes=")
            .map(str::to_owned)
    });
    let is_probe = range.as_deref() == Some("0-524287");
    if is_probe {
        let filename = request
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("");
        let gate = probe_gates.lock().unwrap().get(filename).cloned();
        if let Some(gate) = gate {
            let Ok(permit) = gate.acquire().await else {
                return;
            };
            permit.forget();
        }
    }
    if is_probe && request.starts_with("GET /slow-probe/") {
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    if mode == Mode::NotFound || mode == Mode::Unsatisfiable && !is_probe {
        let status = if mode == Mode::NotFound {
            "404 Not Found"
        } else {
            "416 Range Not Satisfiable"
        };
        let _ = socket
            .write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").as_bytes())
            .await;
        return;
    }
    let count = {
        let mut log = requests.lock().unwrap();
        log.push(range.clone().unwrap_or_else(|| "full".into()));
        log.iter()
            .filter(|range| range.as_str() != "0-524287")
            .count()
    };
    let unavailable = mode == Mode::Unavailable
        || mode == Mode::SelectiveUnavailable
            && request
                .split_whitespace()
                .nth(1)
                .is_some_and(|path| path.ends_with("blocked.bin"))
        || mode == Mode::RecoverAfterProbe && is_probe && requests.lock().unwrap().len() == 1;
    if unavailable {
        let _ = socket
            .write_all(
                b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
        return;
    }
    if mode == Mode::Redirect {
        let _ = socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://github.com/unsafe\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
        return;
    }
    if !is_probe && (mode == Mode::RateLimit || mode == Mode::Retry && count == 1) {
        let retry = if mode == Mode::RateLimit {
            "Retry-After: 60\r\n"
        } else {
            ""
        };
        let _ = socket.write_all(format!("HTTP/1.1 429 Too Many Requests\r\n{retry}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await;
        return;
    }
    if mode == Mode::Html {
        let body = b"<!doctype html><html>gateway error</html>";
        let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await;
        let _ = socket.write_all(body).await;
        return;
    }
    let ranged = range.is_some()
        && !matches!(mode, Mode::Full | Mode::NoLength | Mode::NoMetadata)
        && (mode != Mode::IgnoreRange || is_probe)
        && !data.is_empty();
    let (start, end) = if ranged {
        let (start, end) = range.as_deref().unwrap().split_once('-').unwrap();
        (
            start.parse::<usize>().unwrap(),
            end.parse::<usize>().unwrap().min(data.len() - 1),
        )
    } else {
        (0, data.len().saturating_sub(1))
    };
    if start > end || start >= data.len() && !data.is_empty() {
        let _ = socket
            .write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    }
    let body = if data.is_empty() {
        &data[..]
    } else {
        &data[start..=end]
    };
    let status = if ranged {
        "206 Partial Content"
    } else {
        "200 OK"
    };
    let mut headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n"
    );
    if !matches!(mode, Mode::NoLength | Mode::NoMetadata) {
        headers.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if mode != Mode::NoDigest {
        headers.push_str(if mode == Mode::ChangedEtag && !is_probe {
            "ETag: \"changed\"\r\n"
        } else {
            "ETag: \"original\"\r\n"
        });
    }
    if ranged {
        headers.push_str(&format!(
            "Content-Range: bytes {}-{end}/{}\r\n",
            if mode == Mode::BadRange {
                start + 1
            } else {
                start
            },
            data.len()
        ));
    }
    headers.push_str("\r\n");
    if socket.write_all(headers.as_bytes()).await.is_err() {
        return;
    }
    if mode == Mode::Timeout && !is_probe {
        let _ = socket.write_all(&body[..body.len().min(1024)]).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        return;
    }
    if mode == Mode::Truncated && !is_probe {
        let _ = socket.write_all(&body[..body.len() / 2]).await;
        return;
    }
    for chunk in body.chunks(16 * 1024) {
        if matches!(mode, Mode::Slow | Mode::SelectiveUnavailable) && !is_probe {
            tokio::time::sleep(Duration::from_millis(12)).await;
        }
        if mode == Mode::Crawl && !is_probe || mode == Mode::CrawlProbe {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
        if socket.write_all(chunk).await.is_err() {
            return;
        }
    }
}

fn record(directory: &Path) -> StoredTask {
    StoredTask {
        task: Task {
            details: Default::default(),
            id: uuid::Uuid::new_v4().to_string(),
            url: SOURCE.into(),
            filename: "file.bin".into(),
            directory: directory.into(),
            status: TaskStatus::Queued,
            downloaded: 0,
            total: None,
            speed: 0.0,
            eta: None,
            route: None,
            verification: Verification::Pending,
            error: None,
            final_path: None,
            created_at: 1,
            revision: 1,
        },
        checkpoint: None,
    }
}

#[test]
fn validates_release_urls_and_windows_filenames() {
    let source = parse_release_url(
        " https://github.com/test/repo/releases/download/v1/%E4%B8%AD%E6%96%87%20file.exe ",
    )
    .unwrap();
    assert_eq!(source.filename, "中文 file.exe");
    for url in [
        "",
        "http://github.com/test/repo/releases/download/v1/a.exe",
        "https://github.com.evil.test/test/repo/releases/download/v1/a.exe",
        "https://user:password@github.com/test/repo/releases/download/v1/a.exe",
        "https://github.com/test/repo/releases/download/v1/CON.exe",
        "https://github.com/test/repo/releases/download/v1/a%2Fb.exe",
        "https://github.com/test/repo/releases/download/v1/a.exe?token=x",
        "https://github.com/test/repo/archive/main.zip",
    ] {
        assert!(parse_release_url(url).is_err(), "{url}");
    }
}

#[tokio::test]
async fn older_download_completion_is_not_lost_when_recent_history_is_full() {
    let fixture = Fixture::new(Mode::Range, 32 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    for i in 0..70 {
        let mut item = record(root.path());
        item.task.status = TaskStatus::Completed;
        item.task.created_at = 100 + i;
        item.task.details.queue_position = i + 1;
        store.save(&item).unwrap();
    }
    let mut item = record(root.path());
    item.task.status = TaskStatus::Paused;
    item.task.details.elapsed_ms = Some(0);
    let id = item.task.id.clone();
    store.save(&item).unwrap();
    store.close().unwrap();
    let manager = Manager::start(&path, fixture.engine(), Arc::new(|_| {})).unwrap();
    assert_eq!(manager.snapshot().await.unwrap().tasks.len(), 51);
    manager.action(id.clone(), Action::Resume).await.unwrap();
    let completed = wait_for(&manager, |state| state.completed_tasks == 71).await;
    assert_eq!(completed.tasks.len(), 50);
    assert!(completed.tasks.iter().all(|task| task.id != id));
    let notice = completed
        .notices
        .iter()
        .find(|notice| notice.task_id.as_deref() == Some(&id))
        .unwrap();
    assert_eq!(notice.completion.as_ref().unwrap().filename, "file.bin");
    let downloaded = manager.task(id.clone()).await.unwrap();
    let file = downloaded.final_path.unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), *fixture.bytes);
    assert!(manager.action(id, Action::Resume).await.is_err());
    manager
        .remove_completed(completed.history_revision)
        .await
        .unwrap();
    assert_eq!(manager.snapshot().await.unwrap().total_tasks, 0);
    assert!(file.is_file());
    manager
        .create(downloaded.url, root.path().to_owned())
        .await
        .unwrap();
    let next = wait_for(&manager, |state| state.completed_tasks == 1).await;
    let next_file = next.tasks[0].final_path.as_ref().unwrap();
    assert_ne!(next_file, &file);
    assert_eq!(std::fs::read(next_file).unwrap(), *fixture.bytes);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn v2_queue_order_settings_history_and_notifications() {
    let fixture = Fixture::new(Mode::Slow, 4 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("tasks.sqlite3");
    let manager = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    manager
        .settings(crate::model::Settings {
            limit_kib: 1024,
            close_to_tray: true,
            auto_check: false,
            background_completion_notice: true,
            ..Default::default()
        })
        .await
        .unwrap();
    let first = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap()
        .tasks[0]
        .id
        .clone();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    let diagnosed = manager.diagnose(SOURCE).await.unwrap();
    assert!(diagnosed.diagnostics[0].available);
    assert_eq!(diagnosed.tasks[0].status, TaskStatus::Downloading);
    let second = manager
        .create(SOURCE.replace("file.bin", "second.bin"), root.path().into())
        .await
        .unwrap()
        .tasks[1]
        .id
        .clone();
    let third = manager
        .create(SOURCE.replace("file.bin", "third.bin"), root.path().into())
        .await
        .unwrap()
        .tasks[2]
        .id
        .clone();
    let paused = manager
        .create(SOURCE.replace("file.bin", "paused.bin"), root.path().into())
        .await
        .unwrap()
        .tasks[3]
        .id
        .clone();
    manager.action(paused.clone(), Action::Pause).await.unwrap();
    let current = manager.snapshot().await.unwrap();
    assert!(manager
        .reorder(
            vec![third.clone(), second.clone()],
            current.queue_revision.saturating_sub(1)
        )
        .await
        .is_err());
    let sorted = manager
        .reorder(vec![third.clone(), second.clone()], current.queue_revision)
        .await
        .unwrap();
    assert_eq!(
        sorted
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Queued)
            .map(|t| &t.id)
            .collect::<Vec<_>>(),
        [&third, &second]
    );
    assert_eq!(
        sorted.tasks.iter().find(|t| t.id == paused).unwrap().status,
        TaskStatus::Paused
    );
    assert!(manager
        .route(first.clone(), Some("fixture".into()), true)
        .await
        .is_err());
    manager.action(first.clone(), Action::Pause).await.unwrap();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Paused).await;
    manager.shutdown().await.unwrap();
    let restarted = Manager::start(&db, fixture.engine(), Arc::new(|_| {})).unwrap();
    let snapshot = restarted.snapshot().await.unwrap();
    assert_eq!(snapshot.settings.limit_kib, 1024);
    assert!(snapshot.settings.close_to_tray);
    assert!(snapshot
        .tasks
        .iter()
        .all(|t| t.status == TaskStatus::Paused));
    assert_eq!(snapshot.tasks[1].id, third);
    assert_eq!(snapshot.tasks[2].id, second);
    for id in [third, second, paused] {
        restarted.action(id, Action::Cancel).await.unwrap();
    }
    restarted
        .settings(crate::model::Settings::default())
        .await
        .unwrap();
    restarted
        .action(first.clone(), Action::Resume)
        .await
        .unwrap();
    let completed = wait_for(&restarted, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert_eq!(
        completed
            .notices
            .iter()
            .filter(|n| n.message.contains("下载完成"))
            .count(),
        1
    );
    assert!(completed.tasks[0].details.completed_at.is_some());
    let history = restarted
        .history("test/repo".into(), Some(TaskStatus::Completed), 1)
        .await
        .unwrap();
    assert_eq!(history.items[0].file_state, "present");
    let file = completed.tasks[0].final_path.as_ref().unwrap();
    let renamed = root.path().join("moved.bin");
    std::fs::rename(file, &renamed).unwrap();
    assert_eq!(
        restarted
            .history("file".into(), None, 1)
            .await
            .unwrap()
            .items[0]
            .file_state,
        "missing"
    );
    restarted.action(first, Action::Remove).await.unwrap();
    assert!(renamed.exists());
    assert!(restarted.acknowledge().await.unwrap().notices.is_empty());
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn v2_pinned_route_and_throttled_cancellation() {
    let fixture = Fixture::new(Mode::Range, 17 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    manager
        .settings(crate::model::Settings {
            limit_kib: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(manager
        .create_with(
            SOURCE.into(),
            root.path().into(),
            crate::model::CreateOptions {
                preferred_route: Some("unknown".into()),
                ..Default::default()
            }
        )
        .await
        .is_err());
    let snapshot = manager
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
    let id = snapshot.tasks[0].id.clone();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Downloading).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let started = std::time::Instant::now();
    manager.action(id.clone(), Action::Cancel).await.unwrap();
    wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Cancelled).await;
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!root.path().join(format!(".githubsp-{id}")).exists());
    let snapshot = manager.diagnose(SOURCE).await.unwrap();
    assert_eq!(snapshot.diagnostics.len(), 1);
    assert!(snapshot.diagnostics[0].available);
    manager.shutdown().await.unwrap();
}

#[test]
fn v2_upgrades_real_legacy_task_shape_without_completion_time() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("legacy.sqlite3");
    let task = record(root.path());
    let mut payload = serde_json::to_value(&task).unwrap();
    let data = payload["task"].as_object_mut().unwrap();
    for key in [
        "repository",
        "tag",
        "assetId",
        "officialSha256",
        "preferredRoute",
        "failure",
        "completedAt",
        "elapsedMs",
        "elapsedIsPartial",
        "queuePosition",
    ] {
        data.remove(key);
    }
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY,created_at INTEGER,payload TEXT); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT); PRAGMA user_version=1;").unwrap();
    db.execute(
        "INSERT INTO tasks VALUES(?1,1,?2)",
        rusqlite::params![task.task.id, payload.to_string()],
    )
    .unwrap();
    drop(db);
    let store = Store::open(&path).unwrap();
    let records = store.recover().unwrap();
    assert_eq!(records[0].task.status, TaskStatus::Paused);
    assert_eq!(
        records[0].task.details.repository.as_deref(),
        Some("test/repo")
    );
    assert!(records[0].task.details.completed_at.is_none());
    assert!(records[0].task.details.elapsed_ms.is_none());
}

#[test]
fn parses_ranges_and_collision_names() {
    assert_eq!(parse_content_range("bytes 2-4/10").unwrap().end, 4);
    for range in [
        "bytes */10",
        "bytes 2-1/3",
        "bytes 0-10/10",
        "bytes 0-1/*",
        "items 0-1/10",
    ] {
        assert!(parse_content_range(range).is_err());
    }
    assert_eq!(collision_name("a.exe", 1), "a (1).exe");
    assert_eq!(collision_name("README", 2), "README (2)");
}

#[tokio::test]
async fn downloads_ranges_and_preserves_existing_file() {
    let fixture = Fixture::new(Mode::Range, 64 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("中文 文件夹");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("file.bin"), b"original").unwrap();
    let file = fixture
        .engine()
        .run(
            record(&directory),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await
        .unwrap();
    assert!(file.verified);
    assert_eq!(file.path.file_name().unwrap(), "file (1).bin");
    assert_eq!(std::fs::read(file.path).unwrap(), *fixture.bytes);
    assert_eq!(
        std::fs::read(directory.join("file.bin")).unwrap(),
        b"original"
    );
}

#[tokio::test]
async fn supports_single_stream_unknown_length_and_empty_file() {
    for (mode, size) in [
        (Mode::Full, 12_000),
        (Mode::NoLength, 15_000),
        (Mode::Range, 0),
        (Mode::NoDigest, 18_000),
        (Mode::NoMetadata, 20_000),
    ] {
        let fixture = Fixture::new(mode, size).await;
        let directory = tempfile::tempdir().unwrap();
        let file = fixture
            .engine()
            .run(
                record(directory.path()),
                CancellationToken::new(),
                Reporter::default(),
            )
            .await
            .unwrap();
        assert_eq!(file.size, size as u64);
        assert_eq!(
            file.verified,
            !matches!(mode, Mode::NoDigest | Mode::NoMetadata)
        );
        assert_eq!(std::fs::read(file.path).unwrap(), *fixture.bytes);
    }
}

#[tokio::test]
async fn rejects_invalid_responses_and_corrupt_content() {
    for mode in [
        Mode::IgnoreRange,
        Mode::BadRange,
        Mode::Truncated,
        Mode::Html,
        Mode::BadDigest,
        Mode::ChangedEtag,
        Mode::Timeout,
        Mode::NotFound,
        Mode::Unsatisfiable,
    ] {
        let fixture = Fixture::new(mode, 64 * 1024).await;
        let directory = tempfile::tempdir().unwrap();
        assert!(fixture
            .engine()
            .run(
                record(directory.path()),
                CancellationToken::new(),
                Reporter::default()
            )
            .await
            .is_err());
        assert!(!directory.path().join("file.bin").exists());
    }
}

#[tokio::test]
async fn retries_transient_errors_and_respects_long_retry_after() {
    let fixture = Fixture::new(Mode::Retry, 40_000).await;
    let directory = tempfile::tempdir().unwrap();
    fixture
        .engine()
        .run(
            record(directory.path()),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await
        .unwrap();
    assert_eq!(fixture.requests.lock().unwrap().len(), 3);
    let limited = Fixture::new(Mode::RateLimit, 40_000).await;
    assert!(limited
        .engine()
        .run(
            record(directory.path()),
            CancellationToken::new(),
            Reporter::default()
        )
        .await
        .is_err());
    assert_eq!(limited.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn splits_large_files_by_connection_setting() {
    let fixture = Fixture::new(Mode::Range, 16 * 1024 * 1024).await;
    let directory = tempfile::tempdir().unwrap();
    // 默认 4 个连接：每段 4 MiB。
    let result = fixture
        .engine()
        .run(
            record(directory.path()),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(result.path).unwrap(), *fixture.bytes);
    {
        let requests = fixture.requests.lock().unwrap();
        for range in [
            "0-4194303",
            "4194304-8388607",
            "8388608-12582911",
            "12582912-16777215",
        ] {
            assert!(requests.iter().any(|request| request == range), "{range}");
        }
    }
    // 设置为 2 个连接时只分两段。
    fixture.requests.lock().unwrap().clear();
    let engine = fixture.engine();
    engine.connections.set(2);
    let other = tempfile::tempdir().unwrap();
    engine
        .run(
            record(other.path()),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await
        .unwrap();
    let requests = fixture.requests.lock().unwrap();
    assert!(requests.iter().any(|request| request == "0-8388607"));
    assert!(requests.iter().any(|request| request == "8388608-16777215"));
    assert!(!requests.iter().any(|request| request == "0-4194303"));
}

#[tokio::test]
async fn slow_probe_uses_partial_sample_instead_of_timing_out() {
    let fixture = Fixture::new(Mode::CrawlProbe, 2 * 1024 * 1024).await;
    let source = parse_release_url(SOURCE).unwrap();
    let probe = fixture
        .network
        .probe(
            fixture.network.routes[0].clone(),
            &source,
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(probe.bytes >= crate::network::PROBE_MIN_SAMPLE);
    assert!(probe.bytes < crate::network::PROBE_BYTES);
    assert_eq!(probe.total, Some(2 * 1024 * 1024));
    assert!(probe.range_supported);
}

#[tokio::test]
async fn segmented_download_pauses_resumes_and_assembles_identical_file() {
    let fixture = Fixture::new(Mode::Slow, 8 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let id = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap()
        .tasks[0]
        .id
        .clone();
    wait_for(&manager, |s| s.tasks[0].downloaded > 512 * 1024).await;
    manager.action(id.clone(), Action::Pause).await.unwrap();
    let paused = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Paused).await;
    assert!(paused.tasks[0].downloaded < 8 * 1024 * 1024);
    let before = fixture.requests.lock().unwrap().len();
    manager.action(id, Action::Resume).await.unwrap();
    let done = wait_for(&manager, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert_eq!(
        std::fs::read(done.tasks[0].final_path.as_ref().unwrap()).unwrap(),
        *fixture.bytes
    );
    // 默认 4 段；续传请求从各段已下载的位置继续，而不是从每段开头重下。
    // 先复制出续传后的请求记录再释放锁，测试服务器处理请求时也要拿这把锁。
    let resumed = fixture.requests.lock().unwrap()[before..].to_vec();
    let starts: Vec<u64> = resumed
        .iter()
        .filter(|range| range.as_str() != "0-524287")
        .filter_map(|range| range.split('-').next()?.parse().ok())
        .collect();
    let boundaries = [0, 2 * 1024 * 1024, 4 * 1024 * 1024, 6 * 1024 * 1024];
    assert!(starts.iter().any(|start| !boundaries.contains(start)));
    manager.shutdown().await.unwrap();
}

#[test]
fn part_plan_respects_threshold_minimum_segment_and_limit() {
    use crate::engine::plan_parts;
    const MIB: u64 = 1024 * 1024;
    // 小于 2 MiB、不支持分段或大小未知时只用一个连接。
    assert_eq!(plan_parts(Some(MIB), true, 4), vec![(0, Some(MIB - 1))]);
    assert_eq!(plan_parts(Some(10 * MIB), false, 4).len(), 1);
    assert_eq!(plan_parts(None, true, 4), vec![(0, None)]);
    // 每段至少 1 MiB：3 MiB 最多分 3 段。
    assert_eq!(plan_parts(Some(3 * MIB), true, 8).len(), 3);
    // 连接数上限为 8，范围首尾相接且覆盖整个文件。
    let total = 100 * MIB + 7;
    let parts = plan_parts(Some(total), true, 20);
    assert_eq!(parts.len(), 8);
    let mut next = 0;
    for (start, end) in &parts {
        assert_eq!(*start, next);
        next = end.unwrap() + 1;
    }
    assert_eq!(next, total);
}

/// 这些测试验证单连接下的续传偏移和换线逻辑，显式关闭分段下载。
pub(crate) async fn single_connection(manager: &Manager) {
    let mut settings = manager.snapshot().await.unwrap().settings;
    settings.connections = 1;
    manager.settings(settings).await.unwrap();
}

async fn wait_for(manager: &Manager, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let snapshot = manager.snapshot().await.unwrap();
            if condition(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("等待任务状态超时")
}

#[tokio::test]
async fn queue_pause_resume_duplicate_requests_and_cancellation() {
    let fixture = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let second = root.path().join("second");
    std::fs::create_dir(&second).unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    single_connection(&manager).await;
    let first = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap()
        .tasks[0]
        .id
        .clone();
    assert!(manager
        .create(SOURCE.into(), root.path().into())
        .await
        .is_err());
    let queued = manager
        .create(SOURCE.into(), second.clone())
        .await
        .unwrap()
        .tasks[1]
        .id
        .clone();
    manager.action(queued.clone(), Action::Pause).await.unwrap();
    let snapshot = wait_for(&manager, |snapshot| snapshot.tasks[0].downloaded > 0).await;
    assert_eq!(snapshot.tasks[1].status, TaskStatus::Paused);
    manager.action(first.clone(), Action::Pause).await.unwrap();
    let paused = wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Paused
    })
    .await;
    let offset = paused.tasks[0].downloaded;
    assert!(offset > 0 && offset < 2 * 1024 * 1024);
    manager.action(first.clone(), Action::Resume).await.unwrap();
    manager.action(first.clone(), Action::Resume).await.unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Completed
    })
    .await;
    assert!(fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|value| value.starts_with(&format!("{offset}-"))));
    manager
        .action(queued.clone(), Action::Cancel)
        .await
        .unwrap();
    assert!(!second.join(format!(".githubsp-{queued}")).exists());
    manager.action(first, Action::Remove).await.unwrap();
    assert!(root.path().join("file.bin").exists());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_flushes_progress_and_restart_waits_for_user() {
    let fixture = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("tasks.sqlite3");
    let manager = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    wait_for(&manager, |snapshot| snapshot.tasks[0].downloaded > 0).await;
    let shutdown = manager.shutdown().await.unwrap();
    assert_eq!(shutdown.tasks[0].status, TaskStatus::Paused);
    drop(manager);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let restarted = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    let snapshot = restarted.snapshot().await.unwrap();
    assert_eq!(snapshot.tasks[0].status, TaskStatus::Paused);
    assert!(snapshot.tasks[0].downloaded > 0);
    assert_eq!(
        snapshot.last_directory.as_deref(),
        Some(dunce::canonicalize(root.path()).unwrap().to_str().unwrap())
    );
    restarted
        .action(snapshot.tasks[0].id.clone(), Action::Resume)
        .await
        .unwrap();
    wait_for(&restarted, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Completed
    })
    .await;
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn data_directory_migration_preserves_active_and_queued_downloads_and_resumes_original_parts()
{
    let fixture = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("旧 数据");
    let target = root.path().join("新 数据");
    let downloads = root.path().join("下载");
    let queued = downloads.join("排队");
    for path in [&source, &target, &queued] {
        std::fs::create_dir_all(path).unwrap();
    }
    let manager = Manager::start(
        &source.join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    single_connection(&manager).await;
    manager
        .create(SOURCE.into(), downloads.clone())
        .await
        .unwrap();
    manager.create(SOURCE.into(), queued).await.unwrap();
    wait_for(&manager, |s| s.tasks[0].downloaded > 0).await;
    // 目录预检查必须可在数据库仍打开时执行，不能直接读取被 SQLite 占用的文件。
    crate::relocation::validate_target(&source, &target).unwrap();
    let stopped = manager.shutdown().await.unwrap();
    assert!(stopped.tasks.iter().all(|t| t.status == TaskStatus::Paused));
    let first = &stopped.tasks[0];
    let offset = first.downloaded;
    assert!(offset > 0);
    let partial = downloads.join(format!(".githubsp-{}", first.id));
    assert!(partial.exists());
    let locations = crate::relocation::Locations {
        default_directory: source.clone(),
        config_file: root.path().join("location.json"),
    };
    locations.migrate(&source, &target, false).unwrap();
    locations.verify_before_start().unwrap();
    let restarted = Manager::start(
        &target.join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let restored = restarted.snapshot().await.unwrap();
    assert_eq!(restored.tasks.len(), 2);
    assert!(restored
        .tasks
        .iter()
        .all(|t| t.status == TaskStatus::Paused));
    assert_eq!(restored.tasks[0].downloaded, offset);
    assert_eq!(restored.tasks[0].directory, first.directory);
    locations.mark_started().unwrap();
    locations.finish().unwrap();
    assert!(!source.exists());
    assert!(partial.exists());
    restarted
        .action(first.id.clone(), Action::Resume)
        .await
        .unwrap();
    wait_for(&restarted, |s| s.tasks[0].status == TaskStatus::Completed).await;
    assert!(downloads.join("file.bin").exists());
    assert!(fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|v| v.starts_with(&format!("{offset}-"))));
    assert_eq!(
        restarted.snapshot().await.unwrap().tasks[1].status,
        TaskStatus::Paused
    );
    restarted.shutdown().await.unwrap();
}

#[test]
fn recovers_abrupt_exit_and_preserves_unrecognized_temporary_files() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("tasks.sqlite3")).unwrap();
    let mut task = record(directory.path());
    task.task.status = TaskStatus::Downloading;
    store.save(&task).unwrap();
    assert_eq!(store.recover().unwrap()[0].task.status, TaskStatus::Paused);
    let workspace = Workspace::new(directory.path(), &task.task.id).unwrap();
    std::fs::write(workspace.path.join("unrelated.txt"), b"keep").unwrap();
    assert!(workspace.cleanup().is_err());
    assert_eq!(
        std::fs::read(workspace.path.join("unrelated.txt")).unwrap(),
        b"keep"
    );
    assert!(Workspace::new(directory.path(), "..").is_err());
    assert_eq!(
        Workspace::new(
            &directory.path().join("tasks.sqlite3"),
            &uuid::Uuid::new_v4().to_string()
        )
        .unwrap_err()
        .kind,
        ErrorKind::InvalidInput
    );
}

#[tokio::test]
async fn switches_to_another_route_without_mixing_partial_content() {
    let broken = Fixture::new(Mode::IgnoreRange, 64 * 1024).await;
    let good = Fixture::new(Mode::Range, 64 * 1024).await;
    let mut engine = broken.engine();
    let mut route = good.network.routes[0].clone();
    route.id = "fallback".into();
    route.prefix = route
        .prefix
        .map(|prefix| prefix.replace("/download/", "/slow-probe/"));
    engine.network.routes.push(route);
    let root = tempfile::tempdir().unwrap();
    let file = engine
        .run(
            record(root.path()),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(file.path).unwrap(), *good.bytes);
    assert_eq!(broken.requests.lock().unwrap().len(), 2);
    assert_eq!(good.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn active_cancel_stops_writes_and_removes_only_owned_parts() {
    let fixture = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("keep.txt"), b"keep").unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        fixture.engine(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let id = manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap()
        .tasks[0]
        .id
        .clone();
    wait_for(&manager, |snapshot| snapshot.tasks[0].downloaded > 0).await;
    manager.action(id.clone(), Action::Cancel).await.unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Cancelled
    })
    .await;
    assert!(!root.path().join(format!(".githubsp-{id}")).exists());
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!root.path().join("file.bin").exists());
    assert_eq!(
        std::fs::read(root.path().join("keep.txt")).unwrap(),
        b"keep"
    );
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn recovers_publication_after_crash_without_a_second_copy() {
    let fixture = Fixture::new(Mode::Range, 32 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let mut record = record(root.path());
    let saved = Arc::new(Mutex::new(None));
    let saved_clone = saved.clone();
    let (reporter, mut messages) = Reporter::channel();
    let observer = tokio::spawn(async move {
        while let Some(message) = messages.recv().await {
            if let crate::model::EngineUpdate::Checkpoint(checkpoint) = message.update {
                *saved_clone.lock().unwrap() = Some(checkpoint);
            }
            let _ = message.ack.send(Ok(()));
        }
    });
    let first = fixture
        .engine()
        .run(record.clone(), CancellationToken::new(), reporter)
        .await
        .unwrap();
    observer.await.unwrap();
    record.checkpoint = saved.lock().unwrap().clone();
    assert!(record.checkpoint.as_ref().unwrap().publication.is_some());
    let requests = fixture.requests.lock().unwrap().len();
    let second = fixture
        .engine()
        .run(record, CancellationToken::new(), Reporter::default())
        .await
        .unwrap();
    assert_eq!(first.path, second.path);
    assert_eq!(fixture.requests.lock().unwrap().len(), requests);
    assert!(!root.path().join("file (1).bin").exists());
}

#[tokio::test]
async fn detects_changed_partial_files_before_resuming() {
    let fixture = Fixture::new(Mode::Slow, 2 * 1024 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("tasks.sqlite3");
    let manager = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    single_connection(&manager).await;
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    wait_for(&manager, |snapshot| snapshot.tasks[0].downloaded > 0).await;
    manager.shutdown().await.unwrap();
    drop(manager);
    let record = Store::open(&database).unwrap().recover().unwrap().remove(0);
    let workspace = Workspace::new(root.path(), &record.task.id).unwrap();
    let path = workspace.part(0).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 0xff;
    std::fs::write(path, bytes).unwrap();
    let prior_requests = fixture.requests.lock().unwrap().len();
    let engine = fixture.engine();
    engine.connections.set(1);
    let result = engine
        .run(record, CancellationToken::new(), Reporter::default())
        .await
        .unwrap();
    assert_eq!(std::fs::read(result.path).unwrap(), *fixture.bytes);
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(
        requests[prior_requests + 1],
        format!("0-{}", fixture.bytes.len() - 1)
    );
}

#[cfg(windows)]
#[tokio::test]
async fn locked_partial_file_fails_without_publishing() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new(Mode::Range, 32 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let record = record(root.path());
    let workspace = Workspace::new(root.path(), &record.task.id).unwrap();
    let path = workspace.part(0).unwrap();
    std::fs::write(&path, b"keep").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let result = fixture
        .engine()
        .run(record, CancellationToken::new(), Reporter::default())
        .await;
    drop(lock);
    assert_eq!(result.unwrap_err().kind, ErrorKind::Io);
    assert_eq!(std::fs::read(path).unwrap(), b"keep");
    assert!(!root.path().join("file.bin").exists());
}

#[tokio::test]
async fn refuses_a_partial_file_changed_during_retry() {
    let fixture = Fixture::new(Mode::Truncated, 32 * 1024).await;
    let root = tempfile::tempdir().unwrap();
    let record = record(root.path());
    let part = Workspace::new(root.path(), &record.task.id)
        .unwrap()
        .part(0)
        .unwrap();
    let (reporter, mut updates) = Reporter::channel();
    let engine = fixture.engine();
    let download =
        tokio::spawn(async move { engine.run(record, CancellationToken::new(), reporter).await });
    let mut changed = false;
    while let Some(message) = tokio::time::timeout(Duration::from_secs(5), updates.recv())
        .await
        .unwrap()
    {
        if matches!(
            message.update,
            crate::model::EngineUpdate::Status(TaskStatus::Retrying, _)
        ) {
            let mut bytes = std::fs::read(&part).unwrap();
            assert!(!bytes.is_empty());
            bytes[0] ^= 0xff;
            std::fs::write(&part, bytes).unwrap();
            changed = true;
        }
        message.ack.send(Ok(())).unwrap();
    }
    let error = download.await.unwrap().unwrap_err();
    assert!(changed);
    assert!(error.message.contains("分片在重试前发生变化"));
    assert!(!root.path().join("file.bin").exists());
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn enforces_redirect_hosts_https_and_hop_limit() {
    use crate::network::{redirect_allowed, redirect_policy};
    for target in [
        "https://github.com/file",
        "https://release-assets.githubusercontent.com/file",
        "https://gh-proxy.org/file",
        "https://ghproxy.net/file",
    ] {
        let url = url::Url::parse(target).unwrap();
        assert!(redirect_allowed(&url, 4));
        assert!(!redirect_allowed(&url, 5));
    }
    for target in [
        "http://github.com/file",
        "https://github.com.evil.example/file",
        "https://127.0.0.1/file",
        "https://user@github.com/file",
        "https://github.com:444/file",
    ] {
        assert!(!redirect_allowed(&url::Url::parse(target).unwrap(), 1));
    }
    let mut fixture = Fixture::new(Mode::Redirect, 32 * 1024).await;
    fixture.network.client = reqwest::Client::builder()
        .no_proxy()
        .redirect(redirect_policy())
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let result = fixture
        .engine()
        .run(
            record(root.path()),
            CancellationToken::new(),
            Reporter::default(),
        )
        .await;
    assert!(result.unwrap_err().message.contains("所有线路"));
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    assert!(!root.path().join("file.bin").exists());
}

#[path = "route_experience_tests.rs"]
mod route_experience_tests;

#[tokio::test]
async fn agent_request_is_atomic_replayable_and_retains_hash_after_restart() {
    let fixture = Fixture::new(Mode::Slow, 100_000).await;
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("agent.sqlite3");
    let manager = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    let options = crate::model::CreateOptions {
        request_key: Some("session.request-1".into()),
        ..Default::default()
    };
    let (one, two) = tokio::join!(
        manager.create_with(SOURCE.into(), root.path().into(), options.clone()),
        manager.create_with(SOURCE.into(), root.path().into(), options.clone())
    );
    one.unwrap();
    two.unwrap();
    let first = manager
        .agent_task("session.request-1".into())
        .await
        .unwrap();
    assert_eq!(manager.snapshot().await.unwrap().tasks.len(), 1);
    assert!(manager
        .create_with(
            SOURCE.replace("file.bin", "different.bin"),
            root.path().into(),
            options.clone()
        )
        .await
        .unwrap_err()
        .message
        .contains("不同下载参数"));
    manager
        .action(first.id.clone(), Action::Pause)
        .await
        .unwrap();
    wait_for(&manager, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Paused
    })
    .await;
    manager.shutdown().await.unwrap();
    let reopened = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    reopened
        .create_with(SOURCE.into(), root.path().into(), options.clone())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .agent_task("session.request-1".into())
            .await
            .unwrap()
            .id,
        first.id
    );
    reopened.action(first.id, Action::Resume).await.unwrap();
    let done = wait_for(&reopened, |snapshot| {
        snapshot.tasks[0].status == TaskStatus::Completed
    })
    .await;
    assert_eq!(
        done.tasks[0].details.sha256.as_deref(),
        Some(format!("{:x}", Sha256::digest(&*fixture.bytes)).as_str())
    );
    reopened
        .create_with(SOURCE.into(), root.path().into(), options)
        .await
        .unwrap();
    assert_eq!(reopened.snapshot().await.unwrap().tasks.len(), 1);
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn update_center_preserves_cursor_dispositions_labels_downloads_and_backup() {
    use crate::{
        catalog::Release,
        model::{ReleaseSummary, WatchEntry},
        update_center::Ledger,
    };
    let fixture = Fixture::new(Mode::Range, 4000).await;
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("center.sqlite3");
    let manager = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    let summary = |id, tag: &str| ReleaseSummary {
        id,
        tag: tag.into(),
        url: format!("https://github.com/test/repo/releases/tag/{tag}"),
        published_at: Some(id),
        atom: false,
    };
    let release = |id, tag: &str| Release {
        id,
        tag: tag.into(),
        name: tag.into(),
        prerelease: false,
        url: format!("https://github.com/test/repo/releases/tag/{tag}"),
        notes: "完整说明\n".repeat(200),
        assets: vec![],
        published_at: Some(id),
        atom: false,
    };
    let initial = manager
        .add_favorite("https://github.com/test/repo".into())
        .await
        .unwrap();
    let mut favorite = initial.favorites[0].clone();
    favorite.latest = Some(summary(1, "v1"));
    let entry = WatchEntry {
        acked_release_id: Some(1),
        notified_release_id: Some(1),
        ..Default::default()
    };
    manager
        .update_watch_release(
            favorite.clone(),
            entry.clone(),
            false,
            Some(release(1, "v1")),
        )
        .await
        .unwrap();
    let baseline = manager.update_page(None, 10, None, false).await.unwrap();
    assert!(baseline.events.is_empty());
    manager
        .project_labels(
            "test/repo".into(),
            "开发工具".into(),
            vec!["Rust".into(), "Rust".into()],
        )
        .await
        .unwrap();
    favorite.latest = Some(summary(2, "v2"));
    manager
        .update_watch_release(
            favorite.clone(),
            entry.clone(),
            true,
            Some(release(2, "v2")),
        )
        .await
        .unwrap();
    let first = manager
        .update_page(Some(baseline.next_cursor.clone()), 10, None, false)
        .await
        .unwrap();
    let repeated = manager
        .update_page(Some(baseline.next_cursor), 10, None, false)
        .await
        .unwrap();
    assert_eq!(first.events[0].id, repeated.events[0].id);
    assert!(first.events[0].notes.is_none());
    let id = first.events[0].id;
    assert_eq!(
        manager.update_event(id).await.unwrap().notes,
        Some("完整说明\n".repeat(200))
    );
    assert!(manager.update_event(id).await.unwrap().seen_at.is_none());
    let state = manager.snapshot().await.unwrap();
    assert_eq!(
        state.watch_state.entry("test/repo").unwrap().group,
        "开发工具"
    );
    assert_eq!(
        state.watch_state.entry("test/repo").unwrap().tags,
        vec!["Rust"]
    );
    assert!(state.watch_state.is_unread(&state.favorites[0]));
    manager.update_action(id, "skip".into()).await.unwrap();
    // 同一标签的 Atom/API 身份变化不能产生第二条事件或恢复提醒。
    favorite.latest = Some(summary(200, "v2"));
    manager
        .update_watch_release(favorite, entry, true, Some(release(200, "v2")))
        .await
        .unwrap();
    let state = manager.snapshot().await.unwrap();
    assert!(!state.watch_state.is_unread(&state.favorites[0]));
    assert_eq!(
        manager
            .update_page(None, 10, None, false)
            .await
            .unwrap()
            .events
            .len(),
        1
    );
    assert!(manager.update_event(id).await.unwrap().skipped);
    let created = manager
        .create(SOURCE.replace("/v1/", "/v2/"), root.path().into())
        .await
        .unwrap();
    let task_id = created.tasks[0].id.clone();
    wait_for(&manager, |state| {
        state.tasks[0].status == TaskStatus::Completed
    })
    .await;
    assert_eq!(
        manager.update_event(id).await.unwrap().downloaded_tasks,
        vec![task_id]
    );
    let backup = root.path().join("backup.sqlite3");
    manager.export_backup(backup.clone()).await.unwrap();
    manager.shutdown().await.unwrap();
    let backup = Store::open(&backup).unwrap();
    let ledger: Ledger = backup.get_json("update_ledger").unwrap().unwrap();
    assert_eq!(
        ledger
            .page(Some(&first.next_cursor), 10, None)
            .unwrap()
            .events
            .len(),
        0
    );
    assert!(ledger.event(id).unwrap().skipped);
    drop(backup);
    let reopened = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    assert_eq!(reopened.update_event(id).await.unwrap().release.id, 200);
    assert_eq!(
        reopened
            .snapshot()
            .await
            .unwrap()
            .watch_state
            .entry("test/repo")
            .unwrap()
            .group,
        "开发工具"
    );
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn software_and_watchlist_migration_require_preview_and_preserve_local_boundaries() {
    use crate::{
        model::{ReleaseSummary, WatchRule},
        watchlist,
    };
    let fixture = Fixture::new(Mode::Range, 4000).await;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("imported");
    std::fs::create_dir(&destination).unwrap();
    let destination = dunce::canonicalize(destination).unwrap();
    let database = root.path().join("software.sqlite3");
    let manager = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    manager
        .add_favorite("https://github.com/test/repo".into())
        .await
        .unwrap();
    manager
        .watch_rule(
            "test/repo".into(),
            WatchRule {
                download_directory: Some(root.path().into()),
                asset_pattern: Some("*.bin".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut portable = manager.watchlist_export().await.unwrap();
    let mut settings = manager.snapshot().await.unwrap().settings;
    settings.watch_auto_download = true;
    manager.settings(settings).await.unwrap();
    assert!(!manager
        .snapshot()
        .await
        .unwrap()
        .watch_state
        .entry("test/repo")
        .unwrap()
        .rule
        .automatic(true));
    let exported = serde_json::to_string(&portable).unwrap();
    assert!(!exported.contains("downloadDirectory"));
    assert!(!exported.contains(&root.path().to_string_lossy().to_string()));
    let file = root.path().join("watchlist.json");
    watchlist::write_file(&portable, &file).unwrap();
    assert!(watchlist::write_file(&portable, &file).is_err());
    assert_eq!(
        watchlist::parse(&std::fs::read_to_string(&file).unwrap())
            .unwrap()
            .projects
            .len(),
        1
    );
    portable.projects[0].group = "迁移后的工具".into();
    let input = serde_json::to_string(&portable).unwrap();
    let preview = manager
        .watchlist_preview(input.clone(), destination.clone(), "replace".into())
        .await
        .unwrap();
    assert_eq!(preview.items[0].state, "conflict");
    manager
        .project_labels("test/repo".into(), "预览后修改".into(), vec![])
        .await
        .unwrap();
    assert!(manager
        .import_watchlist(
            input.clone(),
            destination.clone(),
            "replace".into(),
            preview.fingerprint
        )
        .await
        .unwrap_err()
        .message
        .contains("重新预览"));
    let preview = manager
        .watchlist_preview(input.clone(), destination.clone(), "replace".into())
        .await
        .unwrap();
    manager
        .import_watchlist(
            input.clone(),
            destination.clone(),
            "replace".into(),
            preview.fingerprint,
        )
        .await
        .unwrap();
    let current = manager.snapshot().await.unwrap();
    assert_eq!(
        current
            .watch_state
            .entry("test/repo")
            .unwrap()
            .rule
            .download_directory
            .as_deref(),
        Some(destination.as_path())
    );
    assert!(current.favorites[0].latest.is_none());
    assert!(!current
        .watch_state
        .entry("test/repo")
        .unwrap()
        .rule
        .automatic(true));
    assert!(manager
        .update_page(None, 10, None, false)
        .await
        .unwrap()
        .events
        .is_empty());
    let duplicate = manager
        .watchlist_preview(input, destination.clone(), "keep".into())
        .await
        .unwrap();
    assert_eq!(duplicate.items[0].state, "duplicate");
    assert_eq!(duplicate.items[0].action, "keep");
    let mut favorite = current.favorites[0].clone();
    favorite.latest = Some(ReleaseSummary {
        id: 2,
        tag: "v2".into(),
        url: "https://github.com/test/repo/releases/tag/v2".into(),
        published_at: None,
        atom: false,
    });
    manager.update_favorite(favorite, false).await.unwrap();
    manager
        .create(SOURCE.into(), root.path().into())
        .await
        .unwrap();
    wait_for(&manager, |state| {
        state.tasks[0].status == TaskStatus::Completed
    })
    .await;
    let software = manager.software("test/repo".into(), 1).await.unwrap();
    assert_eq!(software.items[0].file_state, "present");
    assert_eq!(software.items[0].differs_from_latest, Some(true));
    let path = software.items[0].path.as_ref().unwrap();
    std::fs::rename(path, root.path().join("moved.bin")).unwrap();
    assert_eq!(
        manager.software("".into(), 1).await.unwrap().items[0].file_state,
        "missing"
    );
    std::fs::write(path, b"changed").unwrap();
    assert_eq!(
        manager.software("".into(), 1).await.unwrap().items[0].file_state,
        "changed_size"
    );
    manager.shutdown().await.unwrap();
    let reopened = Manager::start(&database, fixture.engine(), Arc::new(|_| {})).unwrap();
    assert_eq!(
        reopened
            .snapshot()
            .await
            .unwrap()
            .watch_state
            .entry("test/repo")
            .unwrap()
            .group,
        "迁移后的工具"
    );
    reopened.shutdown().await.unwrap();
}

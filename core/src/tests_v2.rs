use crate::{
    model::Settings,
    preflight,
    source::{parse_resource, Resource},
    store::Store,
};

#[test]
fn resource_forms_and_rejected_boundaries() {
    for suffix in [
        "",
        "/releases",
        "/releases/latest",
        "/releases/tag/v1",
        "/releases/latest/download/中文%20文件.zip",
        "/releases/download/v1/app.exe",
    ] {
        assert!(
            parse_resource(&format!("https://github.com/test/repo{suffix}")).is_ok(),
            "{suffix}"
        );
    }
    assert!(matches!(
        parse_resource("https://github.com/test/repo/releases/latest/download/app.exe").unwrap(),
        Resource::Latest(_, Some(_))
    ));
    for url in [
        "http://github.com/test/repo",
        "https://github.com.evil/test/repo",
        "https://user:pass@github.com/test/repo",
        "https://github.com/test/repo?token=x",
        "https://github.com/test/repo/releases/latest/download/CON.exe",
        "https://github.com/test/repo/tree/main",
        "https://github.com/test%2Frepo/repo",
    ] {
        assert!(parse_resource(url).is_err(), "{url}");
    }
}

#[test]
fn space_and_directory_preflight() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("中文 空格");
    std::fs::create_dir(&directory).unwrap();
    let result = preflight::check(&directory, Some(100), 30).unwrap();
    assert_eq!(result.required, Some(170));
    assert!(preflight::check(&directory, None, 0)
        .unwrap()
        .warning
        .is_some());
    assert!(preflight::check(&directory.join("missing"), None, 0).is_err());
    assert!(preflight::ensure_space(Some(99), Some(100)).is_err());
    assert!(preflight::ensure_space(Some(100), Some(100)).is_ok());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
}

#[test]
fn migration_backup_and_rollback() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY,created_at INTEGER,payload TEXT); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT); PRAGMA user_version=1;").unwrap();
    db.execute(
        "INSERT INTO settings VALUES('last_directory',?1)",
        ["F:\\中文 空格"],
    )
    .unwrap();
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.last_directory().unwrap().as_deref(),
        Some("F:\\中文 空格")
    );
    store
        .set_json(
            "preferences",
            &Settings {
                limit_kib: 512,
                ..Default::default()
            },
        )
        .unwrap();
    store.close().unwrap();
    assert_eq!(
        Store::open(&path).unwrap().settings().unwrap().limit_kib,
        512
    );
    let backup = std::fs::read_dir(root.path().join("backups"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("tasks-v1-before-v3-")
        })
        .unwrap();
    let backup_db = rusqlite::Connection::open(backup).unwrap();
    assert_eq!(
        backup_db
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    let bad = root.path().join("bad.sqlite3");
    let db = rusqlite::Connection::open(&bad).unwrap();
    db.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY,created_at INTEGER,payload TEXT); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT); INSERT INTO tasks VALUES('bad',1,'invalid'); PRAGMA user_version=1;").unwrap();
    drop(db);
    assert!(Store::open(&bad).is_err());
    let db = rusqlite::Connection::open(&bad).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT payload FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "invalid"
    );
}

#[tokio::test]
async fn shared_rate_limit_and_interruptible_wait() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let limiter = Arc::new(crate::limiter::RateLimiter::default());
    limiter.set(128);
    let token = tokio_util::sync::CancellationToken::new();
    let started = Instant::now();
    let (a, b) = tokio::join!(
        limiter.consume(64 * 1024, &token),
        limiter.consume(64 * 1024, &token)
    );
    a.unwrap();
    b.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(850));
    limiter.set(1);
    let cancel = token.clone();
    let work = limiter.clone();
    let pending = tokio::spawn(async move { work.consume(1024 * 1024, &cancel).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    token.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(300), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err()
            .kind,
        crate::error::ErrorKind::Cancelled
    );
    let token = tokio_util::sync::CancellationToken::new();
    let work = limiter.clone();
    let pending = tokio::spawn(async move { work.consume(1024 * 1024, &token).await });
    limiter.set(0);
    tokio::time::timeout(Duration::from_millis(300), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

struct ApiFixture {
    network: crate::network::Network,
    calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for ApiFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl ApiFixture {
    async fn new(status: u16) -> Self {
        Self::with_response(status, |request| {
            let release = serde_json::json!({"id":10,"tag_name":"v1.2.0","name":"Release","prerelease":false,"draft":false,"html_url":"https://github.com/test/repo/releases/tag/v1.2.0","body":"更新说明","assets":[{"id":20,"name":"app-x64.exe","size":1024,"browser_download_url":"https://github.com/test/repo/releases/download/v1.2.0/app-x64.exe","digest":format!("sha256:{}","a".repeat(64))}]});
            if request.contains("/releases?per_page=") {
                let mut pre = release.clone();
                pre["id"] = 11.into();
                pre["prerelease"] = true.into();
                serde_json::json!([release, pre])
            } else {
                release
            }
        })
        .await
    }

    async fn with_response(
        status: u16,
        response_body: impl Fn(&str) -> serde_json::Value + Send + 'static,
    ) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = calls.clone();
        let server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = vec![0; 16384];
                let count = socket.read(&mut bytes).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&bytes[..count]).into_owned();
                log.lock().unwrap().push(request.clone());
                let body = response_body(&request);
                let etag_match = request
                    .to_ascii_lowercase()
                    .contains("if-none-match: \"release-v1\"");
                let effective = if status == 200 && etag_match {
                    304
                } else {
                    status
                };
                let body = if effective == 304 {
                    String::new()
                } else {
                    body.to_string()
                };
                let extra = if status == 429 {
                    "Retry-After: 120\r\nX-RateLimit-Remaining: 0\r\n"
                } else {
                    "ETag: \"release-v1\"\r\n"
                };
                let response = format!("HTTP/1.1 {effective} Result\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{extra}Connection: close\r\n\r\n{body}", body.len());
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        let network = crate::network::Network {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            routes: Vec::new(),
            api_base: format!("http://{address}"),
            probe_timeout: std::time::Duration::from_secs(1),
            idle_timeout: std::time::Duration::from_secs(1),
            retry_delay: std::time::Duration::from_millis(1),
        };
        Self {
            network,
            calls,
            server,
        }
    }
}

#[tokio::test]
async fn official_catalog_pagination_latest_cache_and_rate_limit() {
    let fixture = ApiFixture::new(200).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    let (a, b) = tokio::join!(
        catalog.browse("https://github.com/test/repo", 0, false),
        catalog.browse("https://github.com/test/repo", 0, false)
    );
    assert_eq!(
        a.unwrap().releases[0].assets[0].hints,
        ["Windows", "x64", "安装包"]
    );
    b.unwrap();
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    let (url, asset) = catalog
        .resolve("https://github.com/test/repo/releases/latest/download/app-x64.exe")
        .await
        .unwrap();
    assert_eq!(
        url,
        "https://github.com/test/repo/releases/download/v1.2.0/app-x64.exe"
    );
    assert_eq!(asset.unwrap().id, 20);
    assert_eq!(
        catalog
            .browse("https://github.com/test/repo", 1, false)
            .await
            .unwrap()
            .releases
            .len(),
        1
    );
    assert_eq!(
        catalog
            .browse("https://github.com/test/repo", 1, true)
            .await
            .unwrap()
            .releases
            .len(),
        2
    );
    catalog.expire_cache().await;
    catalog
        .browse("https://github.com/test/repo", 0, false)
        .await
        .unwrap();
    assert!(fixture
        .calls
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .to_ascii_lowercase()
        .contains("if-none-match"));
    let rate_fixture = ApiFixture::new(429).await;
    let rate = crate::catalog::Catalog::new(rate_fixture.network.clone());
    for _ in 0..2 {
        assert!(rate
            .browse("https://github.com/test/repo", 0, false)
            .await
            .unwrap_err()
            .retry_after
            .is_some());
    }
    assert_eq!(rate_fixture.calls.lock().unwrap().len(), 1);
    assert!(rate.next_allowed().await > crate::model::now_ms());
    let direct = "https://github.com/test/repo/releases/download/v1/file.bin";
    assert_eq!(rate.resolve(direct).await.unwrap().0, direct);
}

#[tokio::test]
async fn batch_preview_partial_creation_and_favorites() {
    use crate::{engine::Engine, manager::Manager, model::ReleaseSummary};
    use std::sync::Arc;
    let fixture = ApiFixture::new(200).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        Engine {
            network: fixture.network.clone(),
            limiter: Arc::new(crate::limiter::RateLimiter::default()),
        },
        Arc::new(|_| {}),
    )
    .unwrap();
    let source = "https://github.com/test/repo/releases/latest/download/app-x64.exe".to_owned();
    let preview = crate::intake::preview(
        &manager,
        &catalog,
        vec![source.clone(), source.clone(), "https://evil.test/x".into()],
        root.path(),
    )
    .await
    .unwrap();
    assert_eq!(
        preview
            .items
            .iter()
            .map(|i| i.status.as_str())
            .collect::<Vec<_>>(),
        ["valid", "duplicate", "invalid"]
    );
    assert_eq!(preview.known_size, 1024);
    assert!(
        crate::intake::preview(&manager, &catalog, vec![source.clone(); 101], root.path())
            .await
            .is_err()
    );
    let result = crate::intake::create_batch(
        &manager,
        &catalog,
        vec![source.clone(), source, "bad".into()],
        root.path(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        result
            .items
            .iter()
            .map(|i| i.status.as_str())
            .collect::<Vec<_>>(),
        ["created", "failed", "failed"]
    );
    let library = crate::library::Library::new(catalog.clone(), manager.clone());
    let first = library
        .add("https://github.com/test/repo".into())
        .await
        .unwrap();
    assert_eq!(first.favorites.len(), 1);
    assert_eq!(first.favorites[0].latest.as_ref().unwrap().id, 10);
    let next = library
        .add("https://github.com/TEST/REPO".into())
        .await
        .unwrap();
    assert_eq!(next.favorites.len(), 1);
    let mut favorite = next.favorites[0].clone();
    assert!(!crate::library::apply_release(
        &mut favorite,
        ReleaseSummary {
            id: 10,
            tag: "v1.2.0".into(),
            url: "page".into()
        }
    ));
    assert!(crate::library::apply_release(
        &mut favorite,
        ReleaseSummary {
            id: 11,
            tag: "v1.3.0".into(),
            url: "page".into()
        }
    ));
    assert!(!crate::library::apply_release(
        &mut favorite,
        ReleaseSummary {
            id: 10,
            tag: "v1.2.0".into(),
            url: "page".into()
        }
    ));
    let id = favorite.id.clone();
    manager.remove_favorite("test/repo".into()).await.unwrap();
    manager.update_favorite(favorite, true).await.unwrap();
    assert!(!manager
        .snapshot()
        .await
        .unwrap()
        .favorites
        .iter()
        .any(|f| f.id == id));
    library.stop();
    assert!(library.check(None).await.is_err());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn application_update_configuration_and_comparison() {
    let fixture = ApiFixture::new(200).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    assert_eq!(
        crate::updates::check(&catalog, "", "0.2.0").await.status,
        "not_configured"
    );
    assert!(fixture.calls.lock().unwrap().is_empty());
    let update = crate::updates::check(&catalog, "test/repo", "0.2.0").await;
    assert_eq!(update.status, "available");
    let release = update.latest_release.unwrap();
    assert_eq!(Some(release.tag), update.latest);
    assert_eq!(Some(release.notes), update.notes);
    assert_eq!(Some(release.url), update.url);
    assert_eq!(release.assets[0].id, 20);
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    assert_eq!(
        crate::updates::check(&catalog, "test/repo", "v1.2.0")
            .await
            .status,
        "current"
    );
    assert_eq!(
        crate::updates::check(&catalog, "test/repo", "bad")
            .await
            .status,
        "incomparable"
    );
    assert!(crate::updates::compare("1.0.0-rc.1", "v1.0.0").unwrap());
    assert!(!crate::updates::compare("2.0.0", "v1.9.0").unwrap());
    let limited = ApiFixture::new(429).await;
    let catalog = crate::catalog::Catalog::new(limited.network.clone());
    assert_eq!(
        crate::updates::check(&catalog, "test/repo", "0.2.0")
            .await
            .status,
        "rate_limited"
    );
}

fn update_release(id: u64, tag: &str, prerelease: bool, draft: bool) -> serde_json::Value {
    let filename = format!(
        "GitHubSP-v{}-windows-x64.exe",
        tag.strip_prefix('v').unwrap_or(tag)
    );
    serde_json::json!({
        "id": id, "tag_name": tag, "name": tag, "prerelease": prerelease, "draft": draft,
        "html_url": format!("https://github.com/test/repo/releases/tag/{tag}"), "body": "更新说明",
        "assets": [{ "id": id * 10, "name": filename, "size": 1024,
            "browser_download_url": format!("https://github.com/test/repo/releases/download/{tag}/{filename}"),
            "digest": format!("sha256:{}", "a".repeat(64)) }]
    })
}

#[tokio::test]
async fn application_update_releases_filter_pages_without_losing_later_versions() {
    let fixture = ApiFixture::with_response(200, |request| {
        if request.contains("&page=1 ") {
            serde_json::json!([
                update_release(1, "v1.2.0", false, false),
                update_release(2, "v0.2.3", false, false),
                update_release(3, "v0.1.0", false, false),
                update_release(4, "v0.3.0-beta.1", false, false),
                update_release(5, "v0.4.0", true, false),
                update_release(6, "nightly", false, false),
                update_release(7, "v0.2.10", false, false),
                update_release(8, "0.2.9", false, false),
                update_release(9, "v9.0.0", false, true),
                update_release(10, "v0.2.2", false, false)
            ])
        } else if request.contains("&page=2 ") {
            serde_json::json!((11..=20)
                .map(|id| update_release(id, "v0.1.0", false, false))
                .collect::<Vec<_>>())
        } else {
            serde_json::json!([update_release(21, "v0.5.0", false, false)])
        }
    })
    .await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    let first = crate::updates::list_releases(&catalog, "test/repo", "v0.2.3", 1)
        .await
        .unwrap();
    assert_eq!(first.repository, "test/repo");
    assert_eq!(first.page, 1);
    assert!(first.has_more);
    assert_eq!(
        first
            .releases
            .iter()
            .map(|release| release.tag.as_str())
            .collect::<Vec<_>>(),
        ["v1.2.0", "v0.2.10", "0.2.9"]
    );
    crate::updates::list_releases(&catalog, "test/repo", "v0.2.3", 1)
        .await
        .unwrap();
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    let empty = crate::updates::list_releases(&catalog, "test/repo", "0.2.3", 2)
        .await
        .unwrap();
    assert!(empty.releases.is_empty());
    assert!(empty.has_more);
    let last = crate::updates::list_releases(&catalog, "test/repo", "0.2.3", 3)
        .await
        .unwrap();
    assert_eq!(last.releases[0].tag, "v0.5.0");
    assert!(!last.has_more);
}

#[tokio::test]
async fn application_update_releases_validate_configuration_and_respect_rate_limit() {
    let fixture = ApiFixture::new(200).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    for (repository, version, page) in [
        ("", "0.2.3", 1),
        (" ", "0.2.3", 1),
        ("test/repo/releases/latest", "0.2.3", 1),
        ("test/repo", "invalid", 1),
        ("test/repo", "0.2.3", 0),
        ("test/repo", "0.2.3", 1001),
    ] {
        assert!(
            crate::updates::list_releases(&catalog, repository, version, page)
                .await
                .is_err()
        );
    }
    assert!(fixture.calls.lock().unwrap().is_empty());
    let invalid = crate::updates::check(&catalog, "test/repo/releases/latest", "0.2.3").await;
    assert_eq!(invalid.status, "invalid_source");
    assert!(invalid.latest_release.is_none());

    let limited = ApiFixture::new(429).await;
    let catalog = crate::catalog::Catalog::new(limited.network.clone());
    for _ in 0..2 {
        let error = crate::updates::list_releases(&catalog, "test/repo", "0.2.3", 1)
            .await
            .unwrap_err();
        assert!(error.retry_after.is_some());
    }
    assert_eq!(limited.calls.lock().unwrap().len(), 1);
    assert!(catalog.next_allowed().await > crate::model::now_ms());

    let malformed = ApiFixture::with_response(200, |_| serde_json::json!({"invalid": true})).await;
    let catalog = crate::catalog::Catalog::new(malformed.network.clone());
    assert!(
        crate::updates::list_releases(&catalog, "test/repo", "0.2.3", 1)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn application_update_semantic_prerelease_is_not_downloadable_as_stable() {
    let fixture =
        ApiFixture::with_response(200, |_| update_release(30, "v0.3.0-rc.1", false, false)).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    let result = crate::updates::check(&catalog, "test/repo", "0.2.3").await;
    assert_eq!(result.status, "failed");
    assert!(result.message.contains("预发布"));
    assert!(result.url.is_some());
}

#[tokio::test]
async fn application_update_asset_enters_existing_queue_with_duplicate_protection() {
    use crate::{engine::Engine, manager::Manager};
    use std::sync::Arc;
    let fixture =
        ApiFixture::with_response(200, |_| update_release(30, "v0.3.0", false, false)).await;
    let catalog = crate::catalog::Catalog::new(fixture.network.clone());
    let update = crate::updates::check(&catalog, "test/repo", "0.2.3").await;
    let asset = update.latest_release.unwrap().assets.remove(0);
    let root = tempfile::tempdir().unwrap();
    let manager = Manager::start(
        &root.path().join("tasks.sqlite3"),
        Engine {
            network: fixture.network.clone(),
            limiter: Arc::new(crate::limiter::RateLimiter::default()),
        },
        Arc::new(|_| {}),
    )
    .unwrap();
    let result = crate::intake::create_batch(
        &manager,
        &catalog,
        vec![asset.url.clone()],
        root.path(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.items[0].status, "created");
    assert_eq!(result.items[0].url.as_deref(), Some(asset.url.as_str()));
    assert_eq!(result.snapshot.tasks.len(), 1);
    assert_eq!(result.snapshot.tasks[0].details.asset_id, Some(asset.id));
    assert_eq!(
        result.snapshot.tasks[0].details.official_sha256,
        asset.sha256
    );
    assert_eq!(result.snapshot.tasks[0].total, Some(asset.size));
    let duplicate =
        crate::intake::create_batch(&manager, &catalog, vec![asset.url], root.path(), None)
            .await
            .unwrap();
    assert_eq!(duplicate.items[0].status, "failed");
    assert!(duplicate.items[0]
        .message
        .as_ref()
        .unwrap()
        .contains("相同下载任务"));
    assert_eq!(duplicate.snapshot.tasks.len(), 1);
    manager.shutdown().await.unwrap();
}

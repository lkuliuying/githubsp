use githubsp_lib::{
    catalog::Catalog,
    engine::Engine,
    intake,
    manager::Manager,
    model::{TaskStatus, Verification},
    network::Network,
    store::Store,
    updates,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 3 {
        return Err("用法：verify_v2 <仓库页面> <隔离验收目录> <v0.1 数据库路径>".into());
    }
    let directory = PathBuf::from(&arguments[1]);
    std::fs::create_dir_all(&directory)?;
    let migrated = directory.join("legacy-copy.sqlite3");
    if migrated.exists() {
        return Err("迁移验收副本已存在，请使用新的验收目录".into());
    }
    // 原数据库只读打开，通过备份 API 获取含 WAL 的一致副本，所有迁移写入发生在副本。
    let source = rusqlite::Connection::open_with_flags(
        &arguments[2],
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let source_version: u32 = source.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if source_version != 1 {
        return Err("验收输入必须是 v0.1 数据库".into());
    }
    source.backup("main", &migrated, None)?;
    drop(source);
    let store = Store::open(&migrated)?;
    let records = store.recover()?;
    store.close()?;
    println!(
        "{}",
        serde_json::json!({"migration":true,"sourceVersion":source_version,"tasks":records.len(),"copy":migrated,"unknownCompletion":records.iter().all(|r|r.task.details.completed_at.is_none())})
    );
    let catalog = Catalog::new(Network::production()?);
    let page = catalog.browse(&arguments[0], 0, false).await?;
    let release = page.releases.first().ok_or("仓库无正式版本")?;
    let mut assets = release.assets.clone();
    assets.sort_by_key(|asset| asset.size);
    let selected: Vec<_> = assets
        .iter()
        .filter(|a| a.size > 0 && a.size <= 32 * 1024 * 1024)
        .take(2)
        .map(|a| a.url.clone())
        .collect();
    if selected.len() != 2 {
        return Err("需要两个不超过 32 MiB 的公开附件进行批量验收".into());
    }
    let manager = Manager::start(
        &directory.join("batch.sqlite3"),
        Engine::production()?,
        Arc::new(|_| {}),
    )?;
    let result = async {
        let preview = intake::preview(&manager, &catalog, selected.clone(), &directory).await?;
        println!("{}", serde_json::json!({"repository":page.repository,"release":release.tag,"preview":preview}));
        if preview.items.iter().any(|item| item.status != "valid") { return Err("批量预览失败".into()); }
        let created = intake::create_batch(&manager, &catalog, selected, &directory, None).await?;
        if created.items.iter().any(|item| item.status != "created") { return Err("批量创建未完全成功".into()); }
        let started = Instant::now();
        loop {
            let snapshot = manager.snapshot().await?;
            if snapshot.tasks.iter().any(|task| task.status == TaskStatus::Failed) { return Err(format!("批量下载失败：{:?}", snapshot.tasks.iter().filter_map(|t| t.error.as_ref()).collect::<Vec<_>>()).into()); }
            if snapshot.tasks.iter().all(|task| task.status == TaskStatus::Completed) {
                let verified = snapshot.tasks.iter().all(|task| task.verification == Verification::Verified);
                println!("{}", serde_json::json!({"batchCompleted":true,"officialVerified":verified,"seconds":started.elapsed().as_secs_f64(),"tasks":snapshot.tasks}));
                if !verified { return Err("真实批量下载缺少官方校验结果".into()); }
                break;
            }
            if started.elapsed() > Duration::from_secs(600) { return Err("批量验收超过 10 分钟".into()); }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let update = updates::check(&catalog, "", env!("CARGO_PKG_VERSION")).await;
        println!("{}", serde_json::json!({"unconfiguredUpdate":update}));
        Ok::<(), Box<dyn std::error::Error>>(())
    }.await;
    manager.shutdown().await?;
    catalog.stop();
    result
}

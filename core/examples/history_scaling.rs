use githubsp_lib::{
    engine::Engine,
    manager::Manager,
    model::{StoredTask, Task, TaskDetails, TaskStatus, Verification},
    store::Store,
};
use std::{
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

fn seed(path: &Path, count: u64) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Err("验收数据库已存在，请使用新的隔离路径".into());
    }
    Store::open(path)?.close()?;
    let mut connection = rusqlite::Connection::open(path)?;
    let transaction = connection.transaction()?;
    for index in 0..count + 3 {
        let task = Task {
            id: format!("00000000-0000-4000-8000-{index:012}"),
            url: "https://github.com/test/repo/releases/download/v1/中文附件.zip".into(),
            filename: "中文附件.zip".into(),
            directory: path.parent().unwrap().to_owned(),
            status: if index < count {
                TaskStatus::Completed
            } else {
                TaskStatus::Paused
            },
            downloaded: 1,
            total: Some(1),
            speed: 0.0,
            eta: None,
            route: None,
            verification: Verification::Unverified,
            error: None,
            final_path: None,
            created_at: 1000 + index / 2,
            revision: 0,
            details: TaskDetails {
                repository: Some("test/repo".into()),
                tag: Some("v1".into()),
                queue_position: index,
                ..Default::default()
            },
        };
        let status = task.status.as_str();
        let search = "中文附件.zip test/repo v1";
        let record = StoredTask {
            task,
            checkpoint: None,
        };
        transaction.execute("INSERT INTO tasks(id,created_at,payload,status,search_text,queue_position) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![record.task.id,record.task.created_at as i64,serde_json::to_string(&record)?,status,search,index as i64])?;
    }
    transaction.commit()?;
    connection.close().map_err(|(_, error)| error)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "用法：history_scaling <seed|measure> <新建隔离库绝对路径> <已完成数量>".into(),
        );
    }
    let path = Path::new(&args[1]);
    let count: u64 = args[2].parse()?;
    if !path.is_absolute() || ![1000, 10000, 100000].contains(&count) {
        return Err("需要绝对路径及 1000、10000 或 100000 条历史".into());
    }
    if args[0] == "seed" {
        return seed(path, count);
    }
    if args[0] != "measure" || !path.is_file() {
        return Err("无效操作或验收库不存在".into());
    }
    let started = Instant::now();
    let manager = Manager::start(path, Engine::production()?, Arc::new(|_| {}))?;
    let snapshot = manager.snapshot().await?;
    let startup_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(snapshot.tasks.len(), 53);
    assert_eq!(snapshot.total_tasks, count as usize + 3);
    let mut pages = Vec::new();
    for page in [1, (count / 40) as u32, (count / 20) as u32 + 1] {
        let started = Instant::now();
        let result = manager.history("".into(), None, page).await?;
        assert!(result.items.len() <= 20);
        pages.push(serde_json::json!({"page":page,"rows":result.items.len(),"milliseconds":started.elapsed().as_secs_f64()*1000.0}));
    }
    let started = Instant::now();
    assert_eq!(
        manager
            .history("中文".into(), Some(TaskStatus::Completed), 1)
            .await?
            .total,
        count as usize
    );
    println!(
        "{}",
        serde_json::json!({"completed":count,"unfinished":3,"residentTasks":snapshot.tasks.len(),"startupMilliseconds":startup_ms,"pages":pages,"searchMilliseconds":started.elapsed().as_secs_f64()*1000.0})
    );
    io::stdout().flush()?;
    // 保持工作集供父进程采样；关闭标准输入后也会正常保存并退出。
    io::stdin().read_line(&mut String::new())?;
    manager.shutdown().await?;
    Ok(())
}

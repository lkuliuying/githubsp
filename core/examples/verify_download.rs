use githubsp_lib::{
    engine::Engine,
    manager::Manager,
    model::{Snapshot, TaskStatus, Verification},
};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        return Err("用法：verify_download <公开 Release 链接> <验收目录>".into());
    }
    let directory = PathBuf::from(&arguments[1]);
    std::fs::create_dir_all(&directory)?;
    let last_output = Arc::new(Mutex::new((Instant::now(), TaskStatus::Queued)));
    let emitter = Arc::new(move |snapshot: Snapshot| {
        let Some(task) = snapshot.tasks.last() else {
            return;
        };
        let mut last = last_output.lock().unwrap();
        if task.status != last.1 || last.0.elapsed() >= Duration::from_secs(10) {
            println!(
                "{}",
                serde_json::json!({"status":task.status,"downloaded":task.downloaded,"total":task.total,"route":task.route,"bytesPerSecond":task.speed,"error":task.error})
            );
            *last = (Instant::now(), task.status);
        }
    });
    let state = directory.join("verification.sqlite3");
    let manager = Manager::start(&state, Engine::production()?, emitter)?;
    let initial = manager.snapshot().await?;
    let id = if let Some(task) = initial
        .tasks
        .iter()
        .find(|task| task.url == arguments[0] && task.status.resumable())
    {
        manager
            .action(task.id.clone(), githubsp_lib::manager::Action::Resume)
            .await?;
        task.id.clone()
    } else {
        manager
            .create(arguments[0].clone(), directory)
            .await?
            .tasks
            .last()
            .ok_or("任务未创建")?
            .id
            .clone()
    };
    let started = Instant::now();
    loop {
        let snapshot = manager.snapshot().await?;
        if let Some(error) = snapshot.error {
            manager.shutdown().await?;
            return Err(error.into());
        }
        let task = snapshot
            .tasks
            .iter()
            .find(|task| task.id == id)
            .ok_or("任务不存在")?;
        if task.status == TaskStatus::Completed {
            let path = task.final_path.clone().ok_or("未记录成品位置")?;
            let mut file = std::fs::File::open(&path)?;
            let mut hasher = Sha256::new();
            let mut buffer = vec![0; 1024 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
            println!(
                "{}",
                serde_json::json!({"completed":true,"path":path,"bytes":task.downloaded,"sha256":format!("{:x}",hasher.finalize()),"officialVerified":task.verification==Verification::Verified,"route":task.route,"seconds":started.elapsed().as_secs_f64()})
            );
            manager.shutdown().await?;
            break;
        }
        if matches!(task.status, TaskStatus::Failed | TaskStatus::Cancelled) {
            let error = task.error.clone().unwrap_or("下载未完成".into());
            manager.shutdown().await?;
            return Err(error.into());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(())
}

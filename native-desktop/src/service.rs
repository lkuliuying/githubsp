use githubsp_lib::{
    catalog::Catalog, engine::Engine, library::Library, manager::Manager, model::Snapshot,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::watch;

pub struct Service {
    pub data_directory: std::path::PathBuf,
    pub locations: Option<githubsp_lib::relocation::Locations>,
    pub migration_lock: std::sync::Mutex<Option<crate::platform::DirectoryLock>>,
    pub restart: AtomicBool,
    pub restart_message: std::sync::Mutex<Option<String>>,
    pub manager: Manager,
    pub catalog: Catalog,
    pub library: Library,
    pub state: watch::Receiver<Option<Arc<Snapshot>>>,
    pub closing: AtomicBool,
    pub visible: AtomicBool,
}

impl Service {
    pub async fn start(directory: &Path) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let engine = Engine::production()?;
        let catalog = Catalog::new(engine.network().clone());
        let (send, state) = watch::channel(None);
        let initial = send.clone();
        let manager = Manager::start(
            &directory.join("tasks.sqlite3"),
            engine,
            Arc::new(move |snapshot| {
                publish(&send, snapshot);
            }),
        )?;
        // 读取请求不会触发 actor 的事件，须显式填充初始快照供托盘和关闭策略读取。
        publish(&initial, manager.snapshot().await?);
        let library = Library::new(catalog.clone(), manager.clone());
        // 主窗口和迁移提交成功后才启动收藏检查，避免回退时丢失启动期间的新写入。
        Ok(Arc::new(Self {
            data_directory: directory.to_owned(),
            locations: None,
            migration_lock: std::sync::Mutex::new(None),
            restart: AtomicBool::new(false),
            restart_message: std::sync::Mutex::new(None),
            manager,
            catalog,
            library,
            state,
            closing: AtomicBool::new(false),
            visible: AtomicBool::new(true),
        }))
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.catalog.stop();
        self.library.stop();
        self.manager.shutdown().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
}

fn publish(send: &watch::Sender<Option<Arc<Snapshot>>>, snapshot: Snapshot) {
    send.send_if_modified(|current| {
        if current
            .as_ref()
            .is_some_and(|old| old.revision >= snapshot.revision)
        {
            return false;
        }
        *current = Some(Arc::new(snapshot));
        true
    });
}

pub fn urgent(previous: Option<&Snapshot>, next: &Snapshot) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    previous.error != next.error
        || previous.tasks.len() != next.tasks.len()
        || previous
            .notices
            .iter()
            .map(|n| &n.id)
            .ne(next.notices.iter().map(|n| &n.id))
        || previous
            .tasks
            .iter()
            .zip(&next.tasks)
            .any(|(a, b)| a.id != b.id || a.status != b.status || a.error != b.error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use githubsp_lib::{
        model::{Settings, StoredTask, Task, TaskDetails, TaskStatus, Verification},
        store::Store,
    };

    fn task(directory: &Path) -> Task {
        Task {
            id: "f9c99d5e-6d2a-4938-aea5-c394bd501786".into(),
            url: "https://github.com/test/repo/releases/download/v1/file.bin".into(),
            filename: "file.bin".into(),
            directory: directory.to_owned(),
            status: TaskStatus::Paused,
            downloaded: 0,
            total: Some(1024),
            speed: 0.0,
            eta: None,
            route: None,
            verification: Verification::Pending,
            error: None,
            final_path: None,
            created_at: 0,
            revision: 0,
            details: TaskDetails::default(),
        }
    }

    fn snapshot(revision: u64) -> Snapshot {
        Snapshot {
            total_tasks: 1,
            completed_tasks: 0,
            history_revision: revision,
            tasks: vec![task(Path::new("C:\\下载"))],
            last_directory: None,
            error: None,
            revision,
            settings: Settings::default(),
            queue_revision: 0,
            diagnostics: vec![],
            diagnostic_context: None,
            diagnosing: false,
            notices: vec![],
            favorites: vec![],
        }
    }

    #[test]
    fn only_newer_snapshots_replace_authoritative_state() {
        let (send, receive) = watch::channel(None);
        publish(&send, snapshot(3));
        let mut stale = snapshot(2);
        stale.tasks[0].status = TaskStatus::Completed;
        publish(&send, stale);
        publish(&send, snapshot(3));
        assert_eq!(
            receive.borrow().as_ref().unwrap().tasks[0].status,
            TaskStatus::Paused
        );
        let mut newer = snapshot(4);
        newer.tasks[0].status = TaskStatus::Completed;
        publish(&send, newer);
        assert_eq!(
            receive.borrow().as_ref().unwrap().tasks[0].status,
            TaskStatus::Completed
        );
    }

    #[test]
    fn progress_can_wait_but_completion_failure_and_notices_are_urgent() {
        let old = snapshot(1);
        let mut next = snapshot(2);
        next.tasks[0].downloaded = 400;
        assert!(!urgent(Some(&old), &next));
        next.tasks[0].status = TaskStatus::Completed;
        assert!(urgent(Some(&old), &next));
        next = snapshot(2);
        next.tasks[0].error = Some("写入失败".into());
        assert!(urgent(Some(&old), &next));
        next = snapshot(2);
        next.notices.push(githubsp_lib::model::Notice {
            completion: None,
            id: "n1".into(),
            kind: githubsp_lib::model::NoticeKind::DownloadCompleted,
            task_id: Some("f9c99d5e-6d2a-4938-aea5-c394bd501786".into()),
            message: "完成".into(),
            created_at: 0,
        });
        assert!(urgent(Some(&old), &next));
        assert!(urgent(None, &old));
    }

    #[tokio::test]
    async fn native_service_preserves_data_rejects_duplicate_and_saves_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("中文下载");
        std::fs::create_dir(&downloads).unwrap();
        let downloads = githubsp_lib::preflight::inspect_directory_async(downloads)
            .await
            .unwrap()
            .directory;
        let record = task(&downloads);
        let database = root.path().join("tasks.sqlite3");
        {
            let store = Store::open(&database).unwrap();
            store
                .save(&StoredTask {
                    task: record.clone(),
                    checkpoint: None,
                })
                .unwrap();
        }
        let service = Service::start(root.path()).await.unwrap();
        let saved = service.manager.snapshot().await.unwrap();
        assert_eq!(saved.tasks.len(), 1);
        assert_eq!(saved.tasks[0].id, record.id);
        assert!(service.manager.create(record.url, downloads).await.is_err());
        assert_eq!(service.manager.snapshot().await.unwrap().tasks.len(), 1);
        service
            .manager
            .settings(Settings {
                limit_kib: 256,
                close_to_tray: true,
                ..Settings::default()
            })
            .await
            .unwrap();
        service.shutdown().await.unwrap();
        let reopened = Service::start(root.path()).await.unwrap();
        assert!(
            reopened
                .state
                .borrow()
                .as_ref()
                .unwrap()
                .settings
                .close_to_tray
        );
        let saved = reopened.manager.snapshot().await.unwrap();
        assert_eq!(saved.settings.limit_kib, 256);
        assert!(saved.settings.close_to_tray);
        assert_eq!(saved.tasks[0].status, TaskStatus::Paused);
        reopened.shutdown().await.unwrap();
    }
}

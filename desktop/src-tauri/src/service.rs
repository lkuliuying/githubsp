use githubsp_lib::{
    catalog::Catalog, engine::Engine, library::Library, manager::Manager, model::Snapshot,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::{watch, RwLock};

pub struct Service {
    pub directory: PathBuf,
    pub locations: Option<githubsp_lib::relocation::Locations>,
    pub manager: Manager,
    pub catalog: Catalog,
    pub library: Library,
    pub state: watch::Receiver<Option<Arc<Snapshot>>>,
    pub gate: RwLock<()>,
    pub migration_lock: Mutex<Option<crate::platform::DirectoryLock>>,
    pub closing: AtomicBool,
    pub exit_ready: AtomicBool,
}

impl Service {
    pub async fn start(
        directory: PathBuf,
        locations: Option<githubsp_lib::relocation::Locations>,
    ) -> Result<Arc<Self>, String> {
        let engine = Engine::production().map_err(|e| e.to_string())?;
        let catalog = Catalog::new(engine.network().clone());
        let (send, state) = watch::channel(None::<Arc<Snapshot>>);
        let initial = send.clone();
        let manager = Manager::start(
            &directory.join("tasks.sqlite3"),
            engine,
            Arc::new(move |snapshot| {
                send.send_if_modified(|old| {
                    if old
                        .as_ref()
                        .is_some_and(|old| old.revision >= snapshot.revision)
                    {
                        return false;
                    }
                    *old = Some(Arc::new(snapshot));
                    true
                });
            }),
        )
        .map_err(|e| e.to_string())?;
        initial.send_replace(Some(Arc::new(
            manager.snapshot().await.map_err(|e| e.to_string())?,
        )));
        let library = Library::new(catalog.clone(), manager.clone());
        Ok(Arc::new(Self {
            directory,
            locations,
            manager,
            catalog,
            library,
            state,
            gate: RwLock::new(()),
            migration_lock: Mutex::new(None),
            closing: AtomicBool::new(false),
            exit_ready: AtomicBool::new(false),
        }))
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.closing.store(true, Ordering::Release);
        self.catalog.stop();
        self.library.stop();
        self.manager
            .shutdown()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn host_shutdown_flushes_settings_and_reopens_the_same_database() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().to_owned();
        let service = Service::start(directory.clone(), None).await.unwrap();
        let mut settings = service.manager.snapshot().await.unwrap().settings;
        settings.limit_kib = 731;
        settings.reduce_motion = true;
        service.manager.settings(settings).await.unwrap();
        service.shutdown().await.unwrap();
        assert!(service.closing.load(Ordering::Acquire));
        drop(service);
        let reopened = Service::start(directory, None).await.unwrap();
        let snapshot = reopened.manager.snapshot().await.unwrap();
        assert_eq!(snapshot.settings.limit_kib, 731);
        assert!(snapshot.settings.reduce_motion);
        assert!(snapshot.tasks.is_empty());
        reopened.shutdown().await.unwrap();
    }
}

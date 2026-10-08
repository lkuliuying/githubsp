use crate::{
    error::{DownloadError, ErrorKind, Result},
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::Semaphore;

pub(super) struct Readers {
    path: PathBuf,
    gate: Arc<Semaphore>,
    closing: AtomicBool,
}

impl Readers {
    pub fn new(path: &Path) -> Arc<Self> {
        Arc::new(Self {
            path: path.to_owned(),
            gate: Arc::new(Semaphore::new(1)),
            closing: AtomicBool::new(false),
        })
    }

    pub async fn run<T: Send + 'static>(
        self: &Arc<Self>,
        read: impl FnOnce(&mut Store, &AtomicBool) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        if self.closing.load(Ordering::Acquire) {
            return Err(DownloadError::cancelled());
        }
        let permit = self
            .gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DownloadError::cancelled())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(DownloadError::cancelled());
        }
        let readers = self.clone();
        tokio::task::spawn_blocking(move || {
            // 许可随阻塞任务持有；调用方取消不能提前释放仍在使用的数据库连接。
            let _permit = permit;
            let mut store = Store::open_readonly(&readers.path)?;
            let result = read(&mut store, &readers.closing);
            let closed = store.close();
            let result = result?;
            closed?;
            if readers.closing.load(Ordering::Acquire) {
                return Err(DownloadError::cancelled());
            }
            Ok(result)
        })
        .await
        .map_err(|error| DownloadError::new(ErrorKind::Storage, format!("读取任务异常：{error}")))?
    }

    pub fn stop(&self) {
        self.closing.store(true, Ordering::Release);
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.stop();
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| DownloadError::cancelled())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cancellation_keeps_permit_until_worker_closes_and_shutdown_drains_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tasks.sqlite3");
        Store::open(&path).unwrap().close().unwrap();
        let readers = Readers::new(&path);
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let worker = {
            let (readers, entered, release) = (readers.clone(), entered.clone(), release.clone());
            tokio::spawn(async move {
                readers
                    .run(move |store, _| {
                        entered.store(true, Ordering::Release);
                        while !release.load(Ordering::Acquire) {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        store.counts()
                    })
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            while !entered.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        worker.abort();
        assert_eq!(readers.gate.available_permits(), 0);
        let stopping = {
            let readers = readers.clone();
            tokio::spawn(async move { readers.shutdown().await })
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            while !readers.closing.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!stopping.is_finished());
        assert!(readers.run(|store, _| store.counts()).await.is_err());
        release.store(true, Ordering::Release);
        tokio::time::timeout(Duration::from_secs(5), stopping)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(readers.gate.available_permits(), 1);
        std::fs::rename(path, root.path().join("closed.sqlite3")).unwrap();
    }
}

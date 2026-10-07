use crate::{
    engine::{Engine, ProgressMessage, Reporter},
    error::{DownloadError, ErrorKind, Result},
    files::Workspace,
    model::{
        now_ms, CreateOptions, DownloadedFile, EngineUpdate, RouteReport, Settings, Snapshot,
        StoredTask, Task, TaskDetails, TaskStatus, Verification,
    },
    source::parse_release_url,
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Manager {
    sender: mpsc::Sender<Request>,
    network: crate::network::Network,
    token: CancellationToken,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    Remove,
}

enum Operation {
    Snapshot,
    CreateWith(String, PathBuf, CreateOptions),
    Route(String, Option<String>, bool),
    BeginDiagnose,
    EndDiagnose(Vec<RouteReport>),
    Settings(Settings),
    Reorder(Vec<String>, u64),
    Acknowledge,
    AddFavorite(String),
    RemoveFavorite(String),
    UpdateFavorite(crate::model::Favorite, bool),
    Action(String, Action),
    Shutdown,
}
struct Request {
    operation: Operation,
    reply: oneshot::Sender<Result<Snapshot>>,
}

impl Manager {
    pub fn start(
        path: &Path,
        engine: Engine,
        emit: Arc<dyn Fn(Snapshot) + Send + Sync>,
    ) -> Result<Self> {
        let store = Store::open(path)?;
        let records = store.recover()?;
        let last_directory = store.last_directory()?;
        let settings = store.settings()?;
        let favorites = store.favorites()?;
        engine.limiter.set(settings.limit_kib);
        let mut diagnostics: Vec<RouteReport> = store.get_json("diagnostics")?.unwrap_or_default();
        for route in &engine.network.routes {
            if !diagnostics.iter().any(|r| r.id == route.id) {
                diagnostics.push(RouteReport {
                    id: route.id.clone(),
                    name: route.name.clone(),
                    checked_at: 0,
                    bytes_per_second: 0.0,
                    available: false,
                    error: Some("尚未检测".into()),
                });
            }
        }
        let network = engine.network.clone();
        let (sender, receiver) = mpsc::channel(32);
        let actor = Actor {
            store,
            records,
            last_directory,
            engine,
            emit,
            receiver,
            active: None,
            closing: false,
            shutdown_waiters: Vec::new(),
            error: None,
            revision: 0,
            settings,
            queue_revision: 0,
            diagnostics,
            diagnosing: false,
            notices: Vec::new(),
            favorites,
        };
        tokio::spawn(actor.run());
        Ok(Self {
            sender,
            network,
            token: CancellationToken::new(),
        })
    }

    async fn request(&self, operation: Operation) -> Result<Snapshot> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .send(Request { operation, reply })
            .await
            .map_err(|_| DownloadError::new(ErrorKind::Storage, "任务管理器不可用"))?;
        receive
            .await
            .map_err(|_| DownloadError::new(ErrorKind::Storage, "任务管理器已停止响应"))?
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        self.request(Operation::Snapshot).await
    }
    pub async fn create(&self, url: String, directory: PathBuf) -> Result<Snapshot> {
        self.create_with(url, directory, CreateOptions::default())
            .await
    }
    pub async fn action(&self, id: String, action: Action) -> Result<Snapshot> {
        self.request(Operation::Action(id, action)).await
    }
    pub async fn shutdown(&self) -> Result<Snapshot> {
        self.token.cancel();
        self.request(Operation::Shutdown).await
    }
    pub async fn create_with(
        &self,
        url: String,
        directory: PathBuf,
        options: CreateOptions,
    ) -> Result<Snapshot> {
        parse_release_url(&url)?;
        let directory = crate::preflight::check_async(directory, options.size, 0)
            .await?
            .directory;
        self.request(Operation::CreateWith(url, directory, options))
            .await
    }
    pub async fn settings(&self, settings: Settings) -> Result<Snapshot> {
        self.request(Operation::Settings(settings)).await
    }
    pub async fn reorder(&self, ids: Vec<String>, revision: u64) -> Result<Snapshot> {
        self.request(Operation::Reorder(ids, revision)).await
    }
    pub async fn acknowledge(&self) -> Result<Snapshot> {
        self.request(Operation::Acknowledge).await
    }
    pub async fn add_favorite(&self, repository: String) -> Result<Snapshot> {
        self.request(Operation::AddFavorite(repository)).await
    }
    pub async fn remove_favorite(&self, repository: String) -> Result<Snapshot> {
        self.request(Operation::RemoveFavorite(repository)).await
    }
    pub async fn update_favorite(
        &self,
        favorite: crate::model::Favorite,
        notify: bool,
    ) -> Result<Snapshot> {
        self.request(Operation::UpdateFavorite(favorite, notify))
            .await
    }
    pub async fn route(
        &self,
        id: String,
        route: Option<String>,
        restart: bool,
    ) -> Result<Snapshot> {
        self.request(Operation::Route(id, route, restart)).await
    }
    pub async fn diagnose(&self, url: &str) -> Result<Snapshot> {
        let source = parse_release_url(url)?;
        self.request(Operation::BeginDiagnose).await?;
        let manager = self.clone();
        // 调用界面消失时，测速仍会释放调度占用；退出令牌会中断网络读取。
        tokio::spawn(async move {
            let metadata = manager.network.metadata(&source, &manager.token).await;
            let mut reports = Vec::new();
            for route in &manager.network.routes {
                let result = manager
                    .network
                    .probe(route.clone(), &source, metadata.as_ref(), &manager.token)
                    .await;
                reports.push(crate::network::report(route, &result));
                if manager.token.is_cancelled() {
                    break;
                }
            }
            manager.request(Operation::EndDiagnose(reports)).await
        })
        .await
        .map_err(|e| DownloadError::new(ErrorKind::Network, format!("线路检测工作任务异常：{e}")))?
    }
}

struct Active {
    id: String,
    token: CancellationToken,
    stop: Option<Action>,
    updates: mpsc::Receiver<ProgressMessage>,
    updates_open: bool,
    worker: tokio::task::JoinHandle<Result<DownloadedFile>>,
    last_progress: Instant,
}

struct Actor {
    store: Store,
    records: Vec<StoredTask>,
    last_directory: Option<String>,
    engine: Engine,
    emit: Arc<dyn Fn(Snapshot) + Send + Sync>,
    receiver: mpsc::Receiver<Request>,
    active: Option<Active>,
    closing: bool,
    shutdown_waiters: Vec<oneshot::Sender<Result<Snapshot>>>,
    error: Option<String>,
    revision: u64,
    settings: Settings,
    queue_revision: u64,
    diagnostics: Vec<RouteReport>,
    diagnosing: bool,
    notices: Vec<crate::model::Notice>,
    favorites: Vec<crate::model::Favorite>,
}

enum ActorEvent {
    Request(Option<Request>),
    Progress(Option<ProgressMessage>),
    Finished(std::result::Result<Result<DownloadedFile>, tokio::task::JoinError>),
}

impl Actor {
    async fn run(mut self) {
        loop {
            let event = if let Some(active) = &mut self.active {
                tokio::select! {
                    request = self.receiver.recv() => ActorEvent::Request(request),
                    update = active.updates.recv(), if active.updates_open => ActorEvent::Progress(update),
                    result = &mut active.worker => ActorEvent::Finished(result),
                }
            } else {
                ActorEvent::Request(self.receiver.recv().await)
            };
            match event {
                ActorEvent::Request(Some(request)) => self.handle(request),
                ActorEvent::Request(None) => {
                    if let Some(active) = self.active.take() {
                        active.token.cancel();
                        drop(active.updates);
                        if let Err(error) = active.worker.await {
                            eprintln!("下载任务退出异常：{error}");
                        }
                    }
                    break;
                }
                ActorEvent::Progress(Some(message)) => {
                    let result = self.progress(message.update);
                    if let Err(error) = &result {
                        self.fail_storage(error);
                    }
                    let _ = message.ack.send(result);
                }
                ActorEvent::Progress(None) => {
                    if let Some(active) = &mut self.active {
                        active.updates_open = false;
                    }
                }
                ActorEvent::Finished(result) => {
                    let result = result.unwrap_or_else(|error| {
                        Err(DownloadError::new(
                            ErrorKind::Io,
                            format!("下载工作任务异常：{error}"),
                        ))
                    });
                    if let Err(error) = self.finish(result) {
                        self.fail_storage(&error);
                    }
                }
            }
            if self.closing && self.active.is_none() {
                let snapshot = self.snapshot();
                let close_error = self.store.close().err().map(|error| error.to_string());
                for waiter in self.shutdown_waiters.drain(..) {
                    let result =
                        if let Some(error) = close_error.as_ref().or(snapshot.error.as_ref()) {
                            Err(DownloadError::new(ErrorKind::Storage, error))
                        } else {
                            Ok(snapshot.clone())
                        };
                    let _ = waiter.send(result);
                }
                return;
            }
            if !self.closing && self.error.is_none() && self.active.is_none() && !self.diagnosing {
                if let Err(error) = self.start_next() {
                    self.fail_storage(&error);
                }
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tasks: self
                .records
                .iter()
                .map(|record| record.task.clone())
                .collect(),
            last_directory: self.last_directory.clone(),
            error: self.error.clone(),
            revision: self.revision,
            settings: self.settings.clone(),
            queue_revision: self.queue_revision,
            diagnostics: self.diagnostics.clone(),
            diagnosing: self.diagnosing,
            notices: self.notices.clone(),
            favorites: self.favorites.clone(),
        }
    }

    fn publish(&mut self) {
        self.revision += 1;
        (self.emit)(self.snapshot());
    }

    fn persist(&mut self, index: usize, mut record: StoredTask) -> Result<()> {
        if self.records[index].task.status != record.task.status {
            self.queue_revision += 1;
        }
        record.task.revision += 1;
        self.store.save(&record)?;
        if self.records[index].task.status != record.task.status
            && matches!(
                record.task.status,
                TaskStatus::Completed | TaskStatus::Failed
            )
        {
            self.add_notice(format!(
                "{}：{}",
                if record.task.status == TaskStatus::Completed {
                    "下载完成"
                } else {
                    "下载失败"
                },
                record.task.filename
            ));
        }
        self.records[index] = record;
        self.publish();
        Ok(())
    }

    fn fail_storage(&mut self, error: &DownloadError) {
        self.error = Some(format!("任务状态无法可靠保存，已停止调度：{error}"));
        if let Some(active) = &self.active {
            active.token.cancel();
        }
        self.publish();
    }

    fn handle(&mut self, request: Request) {
        if matches!(request.operation, Operation::Snapshot) {
            let _ = request.reply.send(Ok(self.snapshot()));
            return;
        }
        if matches!(request.operation, Operation::Shutdown) {
            let result = self.begin_shutdown();
            match result {
                Ok(()) => self.shutdown_waiters.push(request.reply),
                Err(error) => {
                    let _ = request.reply.send(Err(error));
                }
            }
            return;
        }
        if self.closing || self.error.is_some() {
            let _ = request.reply.send(Err(DownloadError::new(
                ErrorKind::Storage,
                self.error.as_deref().unwrap_or("应用正在保存进度并退出"),
            )));
            return;
        }
        let result = match request.operation {
            Operation::CreateWith(url, directory, options) => self.create(url, directory, options),
            Operation::Route(id, route, restart) => self.change_route(&id, route, restart),
            Operation::BeginDiagnose => {
                if self.active.is_some() || self.diagnosing {
                    Err(DownloadError::new(
                        ErrorKind::InvalidInput,
                        "请先暂停活动下载，等待写入停止后再测速",
                    ))
                } else {
                    self.diagnosing = true;
                    self.publish();
                    Ok(())
                }
            }
            Operation::EndDiagnose(reports) => {
                self.diagnosing = false;
                self.save_diagnostics(reports)
            }
            Operation::Settings(settings) => self.update_settings(settings),
            Operation::Reorder(ids, revision) => self.reorder(ids, revision),
            Operation::Acknowledge => {
                self.notices.clear();
                self.publish();
                Ok(())
            }
            Operation::AddFavorite(repository) => self.add_favorite(repository),
            Operation::RemoveFavorite(repository) => {
                self.store.remove_favorite(&repository).map(|()| {
                    self.favorites
                        .retain(|f| !f.repository.eq_ignore_ascii_case(&repository));
                    self.publish();
                })
            }
            Operation::UpdateFavorite(favorite, notify) => self.update_favorite(favorite, notify),
            Operation::Action(id, action) => self.action(&id, action),
            _ => Ok(()),
        };
        let _ = request.reply.send(result.map(|()| self.snapshot()));
    }

    fn create(&mut self, url: String, directory: PathBuf, options: CreateOptions) -> Result<()> {
        let source = parse_release_url(&url)?;
        self.validate_route(options.preferred_route.as_deref())?;
        if self.records.iter().any(|record| {
            parse_release_url(&record.task.url).is_ok_and(|old| old.url == source.url)
                && record.task.directory == directory
                && !matches!(
                    record.task.status,
                    TaskStatus::Completed | TaskStatus::Cancelled
                )
        }) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "此目录中已有相同下载任务，请继续或重试原任务",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let workspace = Workspace::new(&directory, &id)?;
        let record = StoredTask {
            task: Task {
                details: TaskDetails {
                    repository: Some(format!("{}/{}", source.owner, source.repo)),
                    tag: Some(source.tag.clone()),
                    asset_id: options.asset_id,
                    official_sha256: options.official_sha256,
                    preferred_route: options.preferred_route,
                    queue_position: self
                        .records
                        .iter()
                        .map(|r| r.task.details.queue_position)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1),
                    ..TaskDetails::default()
                },
                id,
                url: source.url.to_string(),
                filename: source.filename,
                directory,
                status: TaskStatus::Queued,
                downloaded: 0,
                total: options.size,
                speed: 0.0,
                eta: None,
                route: None,
                verification: Verification::Pending,
                error: None,
                final_path: None,
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                revision: 1,
            },
            checkpoint: None,
        };
        if let Err(error) = self.store.insert(&record) {
            if let Err(cleanup) = workspace.cleanup() {
                return Err(DownloadError::new(
                    ErrorKind::Storage,
                    format!("{error}；临时目录清理失败：{cleanup}"),
                ));
            }
            return Err(error);
        }
        self.last_directory = Some(record.task.directory.to_string_lossy().into_owned());
        self.records.push(record);
        self.queue_revision += 1;
        self.publish();
        Ok(())
    }

    fn validate_route(&self, route: Option<&str>) -> Result<()> {
        if route.is_some_and(|id| {
            !self
                .engine
                .network
                .routes
                .iter()
                .any(|route| route.id == id)
        }) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "下载线路不在内置列表中",
            ));
        }
        Ok(())
    }
    fn add_notice(&mut self, message: String) {
        if self.notices.len() >= 100 {
            self.notices.remove(0);
        }
        self.notices.push(crate::model::Notice {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            created_at: now_ms(),
        });
    }
    fn save_diagnostics(&mut self, reports: Vec<RouteReport>) -> Result<()> {
        let mut diagnostics = self.diagnostics.clone();
        for report in reports {
            if let Some(index) = diagnostics.iter().position(|r| r.id == report.id) {
                diagnostics[index] = report;
            } else {
                diagnostics.push(report);
            }
        }
        self.store.set_json("diagnostics", &diagnostics)?;
        self.diagnostics = diagnostics;
        self.publish();
        Ok(())
    }
    fn add_favorite(&mut self, input: String) -> Result<()> {
        let repository = crate::source::parse_resource(&input)?.repository()?;
        if self
            .favorites
            .iter()
            .any(|f| f.repository.eq_ignore_ascii_case(&repository))
        {
            return Ok(());
        }
        if self.favorites.len() >= 200 {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "最多收藏 200 个项目，请先移除不再使用的收藏",
            ));
        }
        let favorite = crate::model::Favorite {
            id: uuid::Uuid::new_v4().to_string(),
            repository,
            latest: None,
            seen: Vec::new(),
            last_checked: None,
            last_success: None,
            next_check: None,
            error: None,
        };
        self.store.save_favorite(&favorite)?;
        self.favorites.push(favorite);
        self.publish();
        Ok(())
    }
    fn update_favorite(&mut self, favorite: crate::model::Favorite, notify: bool) -> Result<()> {
        let Some(index) = self.favorites.iter().position(|f| f.id == favorite.id) else {
            return Ok(());
        };
        self.store.save_favorite(&favorite)?;
        if notify {
            if let Some(latest) = &favorite.latest {
                self.add_notice(format!(
                    "项目有新正式版：{} · {}",
                    favorite.repository, latest.tag
                ));
            }
        }
        self.favorites[index] = favorite;
        self.publish();
        Ok(())
    }
    fn update_settings(&mut self, settings: Settings) -> Result<()> {
        if settings.limit_kib > 10_000_000 {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "限速请输入 0 至 10000000 KiB/s 的整数",
            ));
        }
        self.store.set_json("preferences", &settings)?;
        self.engine.limiter.set(settings.limit_kib);
        self.settings = settings;
        self.publish();
        Ok(())
    }
    fn reorder(&mut self, ids: Vec<String>, revision: u64) -> Result<()> {
        let queued: Vec<_> = self
            .records
            .iter()
            .filter(|r| r.task.status == TaskStatus::Queued)
            .map(|r| r.task.id.clone())
            .collect();
        let mut actual = ids.clone();
        actual.sort();
        actual.dedup();
        let mut expected = queued;
        expected.sort();
        if revision != self.queue_revision || actual != expected || actual.len() != ids.len() {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "队列已变化，请刷新后重试排序",
            ));
        }
        let mut records = self.records.clone();
        let positions: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.task.status == TaskStatus::Queued)
            .map(|(i, _)| i)
            .collect();
        for (position, id) in positions.iter().zip(ids) {
            let source = self.index(&id)?;
            records[*position] = self.records[source].clone();
        }
        for (position, record) in records.iter_mut().enumerate() {
            record.task.details.queue_position = position as u64;
            record.task.revision += 1;
        }
        self.store.save_order(&records)?;
        self.records = records;
        self.queue_revision += 1;
        self.publish();
        Ok(())
    }
    fn change_route(&mut self, id: &str, route: Option<String>, restart: bool) -> Result<()> {
        self.validate_route(route.as_deref())?;
        let index = self.index(id)?;
        let mut record = self.records[index].clone();
        if !matches!(
            record.task.status,
            TaskStatus::Paused | TaskStatus::Failed | TaskStatus::Queued
        ) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "请先暂停任务，等待写入停止后修改线路",
            ));
        }
        if record.task.details.preferred_route == route {
            return Ok(());
        }
        let discards = record.checkpoint.as_ref().is_some_and(|c| {
            route.as_deref().is_some_and(|r| r != c.route_id) && c.downloaded() > 0
        });
        if discards && !restart {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "切换线路将丢弃分片并重新下载，请确认后再操作",
            ));
        }
        if discards {
            Workspace::new(&record.task.directory, id)?.clear_parts()?;
            record.checkpoint = None;
            record.task.downloaded = 0;
        }
        record.task.details.preferred_route = route;
        self.persist(index, record)
    }

    fn index(&self, id: &str) -> Result<usize> {
        self.records
            .iter()
            .position(|record| record.task.id == id)
            .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "下载任务不存在"))
    }

    fn action(&mut self, id: &str, action: Action) -> Result<()> {
        let index = self.index(id)?;
        let mut record = self.records[index].clone();
        let is_active = self.active.as_ref().is_some_and(|active| active.id == id);
        match action {
            Action::Pause | Action::Cancel if is_active => {
                if record.task.status == TaskStatus::Cancelling {
                    return Ok(());
                }
                if let Some(active) = &mut self.active {
                    active.stop = Some(action);
                    active.token.cancel();
                }
                record.task.status = if matches!(action, Action::Pause) {
                    TaskStatus::Pausing
                } else {
                    TaskStatus::Cancelling
                };
            }
            Action::Pause if record.task.status == TaskStatus::Queued => {
                record.task.status = TaskStatus::Paused
            }
            Action::Pause if record.task.status == TaskStatus::Paused => return Ok(()),
            Action::Resume if record.task.status.resumable() => {
                record.task.status = TaskStatus::Queued;
                record.task.error = None;
                record.task.details.failure = None;
            }
            Action::Resume
                if record.task.status == TaskStatus::Queued || record.task.status.running() =>
            {
                return Ok(())
            }
            Action::Cancel
                if matches!(
                    record.task.status,
                    TaskStatus::Queued | TaskStatus::Paused | TaskStatus::Failed
                ) =>
            {
                Workspace::new(&record.task.directory, id)?.cleanup()?;
                record.task.status = TaskStatus::Cancelled;
                record.checkpoint = None;
                record.task.error = None;
            }
            Action::Cancel if record.task.status == TaskStatus::Cancelled => return Ok(()),
            Action::Remove
                if matches!(
                    record.task.status,
                    TaskStatus::Completed | TaskStatus::Cancelled
                ) =>
            {
                self.store.remove(id)?;
                self.records.remove(index);
                self.queue_revision += 1;
                self.publish();
                return Ok(());
            }
            _ => {
                return Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    "当前状态不支持此操作",
                ))
            }
        }
        record.task.speed = 0.0;
        record.task.eta = None;
        self.persist(index, record)
    }

    fn start_next(&mut self) -> Result<()> {
        let Some(index) = self
            .records
            .iter()
            .position(|record| record.task.status == TaskStatus::Queued)
        else {
            return Ok(());
        };
        let mut record = self.records[index].clone();
        record.task.status = TaskStatus::Probing;
        record.task.error = None;
        self.persist(index, record.clone())?;
        let engine = self.engine.clone();
        let token = CancellationToken::new();
        let cancel = token.clone();
        let (reporter, updates) = Reporter::channel();
        let id = record.task.id.clone();
        let worker = tokio::spawn(async move { engine.run(record, cancel, reporter).await });
        self.active = Some(Active {
            id,
            token,
            stop: None,
            updates,
            updates_open: true,
            worker,
            last_progress: Instant::now(),
        });
        Ok(())
    }

    fn progress(&mut self, update: EngineUpdate) -> Result<()> {
        let Some(active) = &self.active else {
            return Err(DownloadError::cancelled());
        };
        let index = self.index(&active.id)?;
        let stopping = active.stop.is_some();
        let elapsed = active.last_progress.elapsed().as_secs_f64().max(0.05);
        let mut record = self.records[index].clone();
        match update {
            EngineUpdate::Metadata(metadata) => {
                record.task.total = Some(metadata.size);
                record.task.details.asset_id = metadata.asset_id;
                record.task.details.official_sha256 = metadata.sha256;
            }
            EngineUpdate::Diagnostics(reports) => {
                return self.save_diagnostics(reports);
            }
            EngineUpdate::Status(status, route) => {
                if !stopping {
                    record.task.status = status;
                }
                if route.is_some() {
                    record.task.route = route;
                }
                if status != TaskStatus::Downloading {
                    record.task.speed = 0.0;
                    record.task.eta = None;
                }
            }
            EngineUpdate::Checkpoint(checkpoint) => {
                let downloaded = checkpoint.downloaded();
                let increment = downloaded.saturating_sub(record.task.downloaded);
                record.task.speed = if stopping || downloaded < record.task.downloaded {
                    0.0
                } else {
                    increment as f64 / elapsed
                };
                record.task.downloaded = downloaded;
                record.task.total = checkpoint.total;
                record.task.verification = if checkpoint.expected_sha256.is_some() {
                    Verification::Pending
                } else {
                    Verification::Unverified
                };
                record.task.eta =
                    checkpoint
                        .total
                        .filter(|_| record.task.speed > 0.0)
                        .map(|total| {
                            (total.saturating_sub(downloaded) as f64 / record.task.speed).ceil()
                                as u64
                        });
                record.checkpoint = Some(checkpoint);
                if let Some(active) = &mut self.active {
                    active.last_progress = Instant::now();
                }
            }
        }
        self.persist(index, record)
    }

    fn finish(&mut self, outcome: Result<DownloadedFile>) -> Result<()> {
        let Some(active) = self.active.take() else {
            return Ok(());
        };
        let index = self.index(&active.id)?;
        let mut record = self.records[index].clone();
        record.task.speed = 0.0;
        record.task.eta = None;
        match outcome {
            Ok(file) => {
                record.task.status = TaskStatus::Completed;
                record.task.downloaded = file.size;
                record.task.total = Some(file.size);
                record.task.final_path = Some(file.path);
                record.task.verification = if file.verified {
                    Verification::Verified
                } else {
                    Verification::Unverified
                };
                record.task.error = None;
                record.task.details.completed_at = Some(now_ms());
                record.task.details.failure = None;
                self.persist(index, record.clone())?;
                match Workspace::new(&record.task.directory, &active.id)?.cleanup() {
                    Ok(()) => {
                        record.checkpoint = None;
                        self.persist(index, record)?;
                    }
                    Err(error) => {
                        record.task.error = Some(format!("文件已完成，临时数据清理失败：{error}"));
                        self.persist(index, record)?;
                    }
                }
                return Ok(());
            }
            Err(error)
                if error.kind == ErrorKind::Cancelled
                    && matches!(active.stop, Some(Action::Cancel)) =>
            {
                match Workspace::new(&record.task.directory, &active.id)?.cleanup() {
                    Ok(()) => {
                        record.task.status = TaskStatus::Cancelled;
                        record.checkpoint = None;
                        record.task.error = None;
                    }
                    Err(error) => {
                        record.task.status = TaskStatus::Failed;
                        record.task.error = Some(format!("已停止下载，但清理失败：{error}"));
                    }
                }
            }
            Err(error) if error.kind == ErrorKind::Cancelled => {
                record.task.status = TaskStatus::Paused;
            }
            Err(error) => {
                record.task.status = TaskStatus::Failed;
                record.task.details.failure = Some(error.failure());
                if error.kind == ErrorKind::Integrity {
                    record.task.verification = Verification::Failed;
                }
                record.task.error = Some(error.to_string());
            }
        }
        self.persist(index, record)
    }

    fn begin_shutdown(&mut self) -> Result<()> {
        for index in 0..self.records.len() {
            if self.records[index].task.status == TaskStatus::Queued {
                let mut record = self.records[index].clone();
                record.task.status = TaskStatus::Paused;
                self.persist(index, record)?;
            }
        }
        if let Some(active) = &self.active {
            let id = active.id.clone();
            self.action(&id, Action::Pause)?;
        }
        self.closing = true;
        Ok(())
    }
}

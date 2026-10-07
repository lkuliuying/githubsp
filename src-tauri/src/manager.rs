use crate::{
    engine::{Engine, ProgressMessage, Reporter},
    error::{DownloadError, ErrorKind, Result},
    files::Workspace,
    model::{
        now_ms, CreateOptions, DiagnosticContext, DiagnosticSource, DownloadedFile, EngineUpdate,
        NoticeKind, RouteReport, Settings, Snapshot, StoredTask, Task, TaskDetails, TaskStatus,
        Verification,
    },
    source::{parse_release_url, ReleaseSource},
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

mod route_ux;
use crate::route_policy::RoutePolicy;
use route_ux::{ActiveRouteState, RouteRuntime, SuggestionResult};

const DEFAULT_DIAGNOSTIC_URL: &str = "https://github.com/lkuliuying/githubsp/releases/download/v0.2.2/GitHubSP-v0.2.2-windows-x64.exe";

#[derive(Clone)]
pub struct Manager {
    sender: mpsc::Sender<Request>,
    token: CancellationToken,
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn execution_time_counts_each_segment_once_and_preserves_unknown() {
        let started = Instant::now();
        let first = ExecutionTime {
            base_ms: Some(0),
            started,
        };
        let stopped = first.value_at(started + Duration::from_millis(1530));
        assert_eq!(stopped, Some(1530));
        let resumed = ExecutionTime {
            base_ms: stopped,
            started: started + Duration::from_secs(60),
        };
        assert_eq!(
            resumed.value_at(started + Duration::from_millis(62300)),
            Some(3830)
        );
        assert_eq!(
            resumed.value_at(started + Duration::from_millis(62300)),
            Some(3830)
        );
        assert_eq!(
            ExecutionTime {
                base_ms: None,
                started
            }
            .value_at(started + Duration::from_secs(60)),
            None
        );
        assert_eq!(
            ExecutionTime {
                base_ms: Some(u64::MAX),
                started
            }
            .value_at(started + Duration::from_secs(1)),
            Some(u64::MAX)
        );
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    pub(super) fn actor(path: &Path) -> Actor {
        let (_, receiver) = mpsc::channel(1);
        Actor {
            store: Store::open(path).unwrap(),
            records: Vec::new(),
            last_directory: None,
            engine: Engine::production().unwrap(),
            emit: Arc::new(|_| {}),
            receiver,
            active: None,
            closing: false,
            shutdown_waiters: Vec::new(),
            error: None,
            revision: 0,
            settings: Settings::default(),
            queue_revision: 0,
            diagnostics: Vec::new(),
            diagnostic_context: None,
            diagnostic_generation: 1,
            diagnostic: None,
            token: CancellationToken::new(),
            notices: Vec::new(),
            favorites: Vec::new(),
            route_ux: RouteRuntime::new(RoutePolicy::default()),
        }
    }

    fn attach(
        actor: &mut Actor,
        worker: tokio::task::JoinHandle<Result<Vec<RouteReport>>>,
    ) -> oneshot::Receiver<Result<Snapshot>> {
        let (reply, receive) = oneshot::channel();
        actor.diagnostic = Some(DiagnosticRun {
            generation: actor.diagnostic_generation,
            context: DiagnosticContext {
                source: DiagnosticSource::Input,
                filename: "DLSS5-Swapper-Setup-2.2.9.exe".into(),
            },
            worker,
            reply,
        });
        receive
    }

    #[tokio::test]
    async fn panicked_or_aborted_worker_releases_busy_state_and_allows_another_round() {
        for panics in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let mut actor = actor(&root.path().join("worker.sqlite3"));
            let worker = tokio::spawn(async move {
                if panics {
                    panic!("测试线路检测工作任务异常");
                }
                std::future::pending::<Result<Vec<RouteReport>>>().await
            });
            if !panics {
                worker.abort();
            }
            let reply = attach(&mut actor, worker);
            assert!(actor.snapshot().diagnosing);
            let result = (&mut actor.diagnostic.as_mut().unwrap().worker).await;
            assert!(result.is_err());
            actor.finish_diagnostic(result);
            assert!(reply
                .await
                .unwrap()
                .unwrap_err()
                .message
                .contains("线路检测工作任务异常"));
            assert!(!actor.snapshot().diagnosing);
            assert!(actor.snapshot().error.is_none());
            let mut reports = actor.pending_diagnostics();
            reports[0].available = true;
            reports[0].checked_at = now_ms();
            let reply = attach(&mut actor, tokio::spawn(async move { Ok(reports) }));
            let result = (&mut actor.diagnostic.as_mut().unwrap().worker).await;
            actor.finish_diagnostic(result);
            let recovered = reply.await.unwrap().unwrap();
            assert!(!recovered.diagnosing);
            assert!(recovered.diagnostics[0].available);
            assert!(recovered.error.is_none());
        }
    }

    #[tokio::test]
    async fn expired_round_cannot_overwrite_newer_display_or_saved_reports() {
        let root = tempfile::tempdir().unwrap();
        let mut actor = actor(&root.path().join("generation.sqlite3"));
        let mut reports = actor.pending_diagnostics();
        reports[0].available = true;
        reports[0].checked_at = 123;
        actor
            .save_diagnostics(
                reports,
                DiagnosticContext {
                    source: DiagnosticSource::Download,
                    filename: "v2rayN-windows-64-desktop.zip".into(),
                },
            )
            .unwrap();
        let reply = attach(&mut actor, tokio::spawn(async { Ok(Vec::new()) }));
        actor.diagnostic_generation += 1;
        let result = (&mut actor.diagnostic.as_mut().unwrap().worker).await;
        actor.finish_diagnostic(result);
        assert!(reply.await.unwrap().unwrap_err().message.contains("已过期"));
        let snapshot = actor.snapshot();
        assert!(!snapshot.diagnosing);
        assert_eq!(snapshot.diagnostics[0].checked_at, 123);
        assert_eq!(
            snapshot.diagnostic_context.unwrap().filename,
            "v2rayN-windows-64-desktop.zip"
        );
        let saved: Vec<RouteReport> = actor.store.get_json("diagnostics").unwrap().unwrap();
        assert_eq!(saved[0].checked_at, 123);
    }
}

#[cfg(test)]
mod recovery_state_tests {
    use super::*;

    #[test]
    fn rejected_route_change_retains_waiting_budget_and_due_order_uses_queue_position() {
        let root = tempfile::tempdir().unwrap();
        let mut actor = super::diagnostic_tests::actor(&root.path().join("recovery.sqlite3"));
        for filename in ["first.bin", "second.bin"] {
            actor
                .create(
                    format!("https://github.com/test/repo/releases/download/v1/{filename}"),
                    root.path().into(),
                    CreateOptions::default(),
                )
                .unwrap();
        }
        let now = Instant::now();
        for index in 0..2 {
            actor.records[index].task.status = TaskStatus::WaitingNetwork;
            let mut clock = crate::route_policy::RecoveryClock::new(now, &actor.route_ux.policy, 0);
            clock.wait = Duration::ZERO;
            actor
                .route_ux
                .recoveries
                .insert(actor.records[index].task.id.clone(), clock);
        }
        actor.tick_routes().unwrap();
        assert_eq!(actor.next_recovery(), Some(0));
        let id = actor.records[0].task.id.clone();
        actor.records[0].checkpoint = Some(crate::model::Checkpoint {
            route_id: actor.engine.network.routes[0].id.clone(),
            total: Some(100),
            etag: None,
            expected_sha256: None,
            range_supported: true,
            publication: None,
            parts: vec![crate::model::Part {
                start: 0,
                end: Some(99),
                downloaded: 10,
                sha256: String::new(),
            }],
        });
        let route = actor.engine.network.routes[1].id.clone();
        assert!(actor.change_route(&id, Some(route), false).is_err());
        assert_eq!(actor.records[0].task.status, TaskStatus::WaitingNetwork);
        assert!(actor.route_ux.recoveries.contains_key(&id));
        assert_eq!(actor.next_recovery(), Some(0));
        actor.action(&id, Action::Pause).unwrap();
        assert_eq!(actor.next_recovery(), Some(1));
    }
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
    ApplySuggestion(String, String),
    DismissSuggestion(String, String),
    Diagnose(ReleaseSource, DiagnosticSource),
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
        Self::start_with_policy(path, engine, emit, RoutePolicy::default())
    }

    pub(crate) fn start_with_policy(
        path: &Path,
        engine: Engine,
        emit: Arc<dyn Fn(Snapshot) + Send + Sync>,
        policy: RoutePolicy,
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
        let token = CancellationToken::new();
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
            diagnostic_context: None,
            diagnostic_generation: 0,
            diagnostic: None,
            token: token.clone(),
            notices: Vec::new(),
            favorites,
            route_ux: RouteRuntime::new(policy),
        };
        tokio::spawn(actor.run());
        Ok(Self { sender, token })
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
    pub async fn apply_route_suggestion(
        &self,
        id: String,
        suggestion_id: String,
    ) -> Result<Snapshot> {
        self.request(Operation::ApplySuggestion(id, suggestion_id))
            .await
    }
    pub async fn dismiss_route_suggestion(
        &self,
        id: String,
        suggestion_id: String,
    ) -> Result<Snapshot> {
        self.request(Operation::DismissSuggestion(id, suggestion_id))
            .await
    }
    pub async fn diagnose(&self, url: &str) -> Result<Snapshot> {
        let input = url.trim();
        let (target, origin) = if input.is_empty() {
            (DEFAULT_DIAGNOSTIC_URL, DiagnosticSource::Default)
        } else {
            (input, DiagnosticSource::Input)
        };
        let source = parse_release_url(target).map_err(|_| {
            DownloadError::new(
                ErrorKind::InvalidInput,
                "请输入附件直链，或清空输入后使用默认测试文件",
            )
        })?;
        self.request(Operation::Diagnose(source, origin)).await
    }
}

type DiagnosticResult = std::result::Result<Result<Vec<RouteReport>>, tokio::task::JoinError>;

struct DiagnosticRun {
    generation: u64,
    context: DiagnosticContext,
    worker: tokio::task::JoinHandle<Result<Vec<RouteReport>>>,
    reply: oneshot::Sender<Result<Snapshot>>,
}

struct Active {
    id: String,
    token: CancellationToken,
    stop: Option<Action>,
    updates: mpsc::Receiver<ProgressMessage>,
    updates_open: bool,
    worker: tokio::task::JoinHandle<Result<DownloadedFile>>,
    elapsed: ExecutionTime,
    last_saved: Instant,
    diagnostic_generation: Option<u64>,
    ux: ActiveRouteState,
}

struct ExecutionTime {
    base_ms: Option<u64>,
    started: Instant,
}

impl ExecutionTime {
    fn value_at(&self, now: Instant) -> Option<u64> {
        self.base_ms.map(|base| {
            base.saturating_add(
                now.saturating_duration_since(self.started)
                    .as_millis()
                    .min(u64::MAX as u128) as u64,
            )
        })
    }
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
    diagnostic_context: Option<DiagnosticContext>,
    diagnostic_generation: u64,
    diagnostic: Option<DiagnosticRun>,
    token: CancellationToken,
    notices: Vec<crate::model::Notice>,
    favorites: Vec<crate::model::Favorite>,
    route_ux: RouteRuntime,
}

enum ActorEvent {
    Request(Option<Request>),
    Progress(Option<ProgressMessage>),
    Finished(std::result::Result<Result<DownloadedFile>, tokio::task::JoinError>),
    Diagnosed(DiagnosticResult),
    Suggested(SuggestionResult),
    Tick,
}

impl Actor {
    async fn run(mut self) {
        let mut timer = tokio::time::interval(self.route_ux.policy.tick);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let event = {
                let suggestion = async {
                    match &mut self.route_ux.job {
                        Some(run) => (&mut run.worker).await,
                        None => std::future::pending().await,
                    }
                };
                let diagnostic = async {
                    match &mut self.diagnostic {
                        Some(run) => (&mut run.worker).await,
                        None => std::future::pending().await,
                    }
                };
                if let Some(active) = &mut self.active {
                    tokio::select! {
                        request = self.receiver.recv() => ActorEvent::Request(request),
                        update = active.updates.recv(), if active.updates_open => ActorEvent::Progress(update),
                        result = &mut active.worker => ActorEvent::Finished(result),
                        result = diagnostic => ActorEvent::Diagnosed(result),
                        result = suggestion => ActorEvent::Suggested(result),
                        _ = timer.tick() => ActorEvent::Tick,
                    }
                } else {
                    tokio::select! {
                        request = self.receiver.recv() => ActorEvent::Request(request),
                        result = diagnostic => ActorEvent::Diagnosed(result),
                        result = suggestion => ActorEvent::Suggested(result),
                        _ = timer.tick(), if !self.route_ux.recoveries.is_empty() => ActorEvent::Tick,
                    }
                }
            };
            self.account_recovery_time();
            match event {
                ActorEvent::Suggested(result) => self.finish_suggestion(result),
                ActorEvent::Diagnosed(result) => self.finish_diagnostic(result),
                ActorEvent::Tick => {
                    if self.error.is_none() {
                        if let Err(error) = self.tick() {
                            self.fail_storage(&error);
                        }
                    }
                }
                ActorEvent::Request(Some(request)) => self.handle(request),
                ActorEvent::Request(None) => {
                    self.token.cancel();
                    self.cancel_suggestion("任务管理器已关闭");
                    if let Some(active) = self.active.take() {
                        active.token.cancel();
                        drop(active.updates);
                        if let Err(error) = active.worker.await {
                            eprintln!("下载任务退出异常：{error}");
                        }
                    }
                    if let Some(run) = self.diagnostic.take() {
                        if let Err(error) = run.worker.await {
                            eprintln!("线路检测退出异常：{error}");
                        }
                        let _ = run.reply.send(Err(DownloadError::cancelled()));
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
            if self.closing && self.active.is_none() && self.diagnostic.is_none() {
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
            if !self.closing && self.error.is_none() && self.active.is_none() {
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
                .map(|record| {
                    let mut task = record.task.clone();
                    if let Some(active) = self.active.as_ref().filter(|a| a.id == task.id) {
                        task.details.elapsed_ms = active.elapsed.value_at(Instant::now());
                    }
                    self.project_route_state(&mut task);
                    task
                })
                .collect(),
            last_directory: self.last_directory.clone(),
            error: self.error.clone(),
            revision: self.revision,
            settings: self.settings.clone(),
            queue_revision: self.queue_revision,
            diagnostics: self.diagnostics.clone(),
            diagnostic_context: self.diagnostic_context.clone(),
            diagnosing: self.diagnostic.is_some(),
            notices: self.notices.clone(),
            favorites: self.favorites.clone(),
        }
    }

    fn publish(&mut self) {
        self.revision += 1;
        (self.emit)(self.snapshot());
    }

    fn persist(&mut self, index: usize, mut record: StoredTask) -> Result<()> {
        if let Some(active) = self.active.as_ref().filter(|a| a.id == record.task.id) {
            record.task.details.elapsed_ms = active.elapsed.value_at(Instant::now());
        }
        if self.records[index].task.status != record.task.status {
            self.queue_revision += 1;
        }
        record.task.revision += 1;
        self.store.save(&record)?;
        if let Some(active) = self.active.as_mut().filter(|a| a.id == record.task.id) {
            active.last_saved = Instant::now();
        }
        if self.records[index].task.status != record.task.status
            && matches!(
                record.task.status,
                TaskStatus::Completed | TaskStatus::Failed
            )
        {
            self.add_notice(
                if record.task.status == TaskStatus::Completed {
                    NoticeKind::DownloadCompleted
                } else {
                    NoticeKind::DownloadFailed
                },
                Some(record.task.id.clone()),
                format!(
                    "{}：{}",
                    if record.task.status == TaskStatus::Completed {
                        "下载完成"
                    } else {
                        "下载失败"
                    },
                    record.task.filename
                ),
            );
        }
        self.records[index] = record;
        self.publish();
        Ok(())
    }

    fn fail_storage(&mut self, error: &DownloadError) {
        self.error = Some(format!("任务状态无法可靠保存，已停止调度：{error}"));
        self.cancel_suggestion("任务状态无法可靠保存");
        if let Some(active) = &self.active {
            active.token.cancel();
        }
        self.publish();
    }

    fn tick(&mut self) -> Result<()> {
        self.tick_routes()?;
        if let Some(active) = &self.active {
            if active.last_saved.elapsed() >= Duration::from_secs(5) {
                let index = self.index(&active.id)?;
                return self.persist(index, self.records[index].clone());
            }
            self.publish();
        } else if !self.route_ux.recoveries.is_empty() {
            self.publish();
        }
        Ok(())
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
            Operation::ApplySuggestion(id, suggestion_id) => {
                self.confirm_suggestion(&id, &suggestion_id, request.reply);
                return;
            }
            Operation::DismissSuggestion(id, suggestion_id) => {
                self.dismiss_suggestion(&id, &suggestion_id)
            }
            Operation::Diagnose(source, origin) => {
                if self.diagnostic.is_some() {
                    Err(DownloadError::new(
                        ErrorKind::InvalidInput,
                        "线路检测正在进行，请等待本轮检测结束",
                    ))
                } else {
                    self.cancel_suggestion("手动线路检测优先，请稍后重新确认换线建议");
                    self.start_diagnostic(source, origin, request.reply);
                    return;
                }
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
                    elapsed_ms: Some(0),
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
    fn add_notice(&mut self, kind: NoticeKind, task_id: Option<String>, message: String) {
        if self.notices.len() >= 100 {
            self.notices.remove(0);
        }
        self.notices.push(crate::model::Notice {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            task_id,
            message,
            created_at: now_ms(),
        });
    }
    fn pending_diagnostics(&self) -> Vec<RouteReport> {
        self.engine
            .network
            .routes
            .iter()
            .map(|route| RouteReport {
                id: route.id.clone(),
                name: route.name.clone(),
                checked_at: 0,
                bytes_per_second: 0.0,
                available: false,
                error: None,
            })
            .collect()
    }

    fn start_diagnostic(
        &mut self,
        source: ReleaseSource,
        origin: DiagnosticSource,
        reply: oneshot::Sender<Result<Snapshot>>,
    ) {
        self.diagnostic_generation = self.diagnostic_generation.wrapping_add(1);
        let context = DiagnosticContext {
            source: origin,
            filename: source.filename.clone(),
        };
        let network = self.engine.network.clone();
        let token = self.token.clone();
        let worker = tokio::spawn(async move {
            let metadata = network.metadata(&source, &token).await;
            let mut reports = Vec::new();
            for route in &network.routes {
                if token.is_cancelled() {
                    return Err(DownloadError::cancelled());
                }
                let result = network
                    .probe(route.clone(), &source, metadata.as_ref(), &token)
                    .await;
                reports.push(crate::network::report(route, &result));
            }
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            Ok(reports)
        });
        self.diagnostic = Some(DiagnosticRun {
            generation: self.diagnostic_generation,
            context: context.clone(),
            worker,
            reply,
        });
        self.diagnostics = self.pending_diagnostics();
        self.diagnostic_context = Some(context);
        self.publish();
    }

    fn finish_diagnostic(&mut self, result: DiagnosticResult) {
        let Some(run) = self.diagnostic.take() else {
            return;
        };
        let result = result.unwrap_or_else(|error| {
            Err(DownloadError::new(
                ErrorKind::Network,
                format!("线路检测工作任务异常：{error}"),
            ))
        });
        let result = result.and_then(|reports| {
            if run.generation != self.diagnostic_generation {
                return Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    "该轮线路检测结果已过期，请重新检测",
                ));
            }
            self.save_diagnostics(reports, run.context)
        });
        // 即使检测异常或保存失败，也要先释放运行状态，允许用户再次检测。
        if result.is_err() {
            self.publish();
        }
        let _ = run.reply.send(result.map(|()| self.snapshot()));
    }

    fn save_diagnostics(
        &mut self,
        reports: Vec<RouteReport>,
        context: DiagnosticContext,
    ) -> Result<()> {
        let mut diagnostics = self.pending_diagnostics();
        for report in reports {
            if let Some(index) = diagnostics.iter().position(|r| r.id == report.id) {
                diagnostics[index] = report;
            }
        }
        self.store.set_json("diagnostics", &diagnostics)?;
        self.diagnostics = diagnostics;
        self.diagnostic_context = Some(context);
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
                self.add_notice(
                    NoticeKind::FavoriteUpdated,
                    None,
                    format!("项目有新正式版：{} · {}", favorite.repository, latest.tag),
                );
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
                "下载限速超出允许范围，请输入 0（不限速）或 0.001024 至 10240 MB/s 之间的数值",
            ));
        }
        self.store.set_json("preferences", &settings)?;
        self.engine.limiter.set(settings.limit_kib);
        if settings.limit_kib != 0 {
            self.cancel_suggestion("下载限速已启用，本次换线未执行");
            for record in &mut self.records {
                record.task.details.route_suggestion = None;
            }
        }
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
            TaskStatus::Paused
                | TaskStatus::Failed
                | TaskStatus::Queued
                | TaskStatus::WaitingNetwork
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
        if record.task.status == TaskStatus::WaitingNetwork {
            self.clear_recovery(id);
            record.task.status = TaskStatus::Paused;
            record.task.details.retry_info = None;
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
        if matches!(action, Action::Pause | Action::Cancel) {
            self.stop_route_work(id);
            record.task.details.retry_info = None;
            record.task.details.route_suggestion = None;
        }
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
            Action::Pause
                if matches!(
                    record.task.status,
                    TaskStatus::Queued | TaskStatus::WaitingNetwork
                ) =>
            {
                record.task.status = TaskStatus::Paused
            }
            Action::Pause if record.task.status == TaskStatus::Paused => return Ok(()),
            Action::Resume if record.task.status == TaskStatus::WaitingNetwork => {
                if let Some(clock) = self.route_ux.recoveries.get_mut(id) {
                    clock.wait = Duration::ZERO;
                }
                self.publish();
                return Ok(());
            }
            Action::Resume if record.task.status.resumable() => {
                record.task.status = TaskStatus::Queued;
                record.task.error = None;
                record.task.details.failure = None;
                record.task.details.retry_info = None;
                record.task.details.route_failures.clear();
            }
            Action::Resume
                if record.task.status == TaskStatus::Queued || record.task.status.running() =>
            {
                return Ok(())
            }
            Action::Cancel
                if matches!(
                    record.task.status,
                    TaskStatus::Queued
                        | TaskStatus::Paused
                        | TaskStatus::Failed
                        | TaskStatus::WaitingNetwork
                ) =>
            {
                if let Err(error) = Workspace::new(&record.task.directory, id)
                    .and_then(|workspace| workspace.cleanup())
                {
                    if record.task.status == TaskStatus::WaitingNetwork {
                        record.task.status = TaskStatus::Failed;
                        record.task.error =
                            Some(format!("已停止恢复，但临时数据清理失败：{error}"));
                        record.task.details.failure = Some(error.failure());
                        self.persist(index, record)?;
                    }
                    return Err(error);
                }
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
        let forced = self.route_ux.first_route.as_ref().and_then(|(id, _)| {
            self.records.iter().position(|record| {
                record.task.id == *id && record.task.status == TaskStatus::Queued
            })
        });
        let Some(index) = forced
            .or_else(|| {
                self.records
                    .iter()
                    .position(|record| record.task.status == TaskStatus::Queued)
            })
            .or_else(|| self.next_recovery())
        else {
            return Ok(());
        };
        let mut record = self.records[index].clone();
        record.task.status = TaskStatus::Probing;
        record.task.error = None;
        if self.route_ux.recoveries.contains_key(&record.task.id) {
            record.task.details.retry_info = Some(crate::model::RetryInfo {
                phase: "recovering".into(),
                attempt: 0,
                max_attempts: 0,
                reason: "正在重新检测线路；满足续传条件时保留进度，换线时从头下载".into(),
                retry_in_ms: 0,
            });
        }
        self.persist(index, record.clone())?;
        let engine = self.engine.clone();
        let token = CancellationToken::new();
        let cancel = token.clone();
        let (reporter, updates) = Reporter::channel();
        let id = record.task.id.clone();
        let started = Instant::now();
        let elapsed = ExecutionTime {
            base_ms: record.task.details.elapsed_ms,
            started,
        };
        // 手动检测拥有展示优先权；期间开始的下载仍自行选线，但不发布到线路面板。
        let diagnostic_generation = if self.diagnostic.is_none() {
            self.diagnostic_generation = self.diagnostic_generation.wrapping_add(1);
            Some(self.diagnostic_generation)
        } else {
            None
        };
        let ux = ActiveRouteState::new(started, record.task.downloaded, &self.route_ux.policy);
        let first_route = if self
            .route_ux
            .first_route
            .as_ref()
            .is_some_and(|(target, _)| *target == id)
        {
            self.route_ux.first_route.take().map(|(_, route)| route)
        } else {
            None
        };
        let options = crate::engine::RunOptions {
            first_route,
            cooldowns: self.route_ux.cooldowns.clone(),
        };
        let worker =
            tokio::spawn(async move { engine.run_with(record, cancel, reporter, options).await });
        self.active = Some(Active {
            id,
            token,
            stop: None,
            updates,
            updates_open: true,
            worker,
            elapsed,
            last_saved: started,
            diagnostic_generation,
            ux,
        });
        Ok(())
    }

    fn progress(&mut self, update: EngineUpdate) -> Result<()> {
        let Some(active) = &self.active else {
            return Err(DownloadError::cancelled());
        };
        let index = self.index(&active.id)?;
        let stopping = active.stop.is_some();
        let mut record = self.records[index].clone();
        match update {
            EngineUpdate::Cooldown(id, deadline) => {
                self.route_ux
                    .cooldowns
                    .entry(id)
                    .and_modify(|old| *old = (*old).max(deadline))
                    .or_insert(deadline);
                return Ok(());
            }
            EngineUpdate::DataReceived => {
                self.clear_recovery(&record.task.id);
                return Ok(());
            }
            EngineUpdate::Retry(info, wait) => {
                if !stopping {
                    record.task.details.retry_info = Some(info);
                    if let Some(active) = &mut self.active {
                        active.ux.retry_deadline = Instant::now().checked_add(wait);
                    }
                }
            }
            EngineUpdate::SelectedRoute(id, throughput, switching) => {
                self.cancel_suggestion("下载线路已变化，本次换线建议已失效");
                record.task.details.route_suggestion = None;
                if let Some(active) = &mut self.active {
                    active.ux.selected = Some((id.clone(), throughput));
                    active.ux.samples = crate::route_policy::SpeedWindow::new(
                        Instant::now(),
                        record.task.downloaded,
                        self.route_ux.policy.slow_window,
                    );
                }
                record.task.details.retry_info = switching.then(|| crate::model::RetryInfo {
                    phase: "switching".into(),
                    attempt: 1,
                    max_attempts: 3,
                    reason: format!(
                        "已改用 {}，从头重新下载",
                        self.engine
                            .network
                            .routes
                            .iter()
                            .find(|route| route.id == id)
                            .map(|route| route.name.as_str())
                            .unwrap_or(&id)
                    ),
                    retry_in_ms: 0,
                });
            }
            EngineUpdate::Metadata(metadata) => {
                record.task.total = Some(metadata.size);
                record.task.details.asset_id = metadata.asset_id;
                record.task.details.official_sha256 = metadata.sha256;
            }
            EngineUpdate::Diagnostics(reports) => {
                if self.diagnostic.is_none()
                    && active.diagnostic_generation == Some(self.diagnostic_generation)
                {
                    return self.save_diagnostics(
                        reports,
                        DiagnosticContext {
                            source: DiagnosticSource::Download,
                            filename: record.task.filename,
                        },
                    );
                }
                return Ok(());
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
                    self.cancel_suggestion("下载阶段已变化，本次换线建议已失效");
                    record.task.details.route_suggestion = None;
                } else if !stopping {
                    if record
                        .task
                        .details
                        .retry_info
                        .as_ref()
                        .is_some_and(|info| info.phase != "switching")
                    {
                        record.task.details.retry_info = None;
                    }
                    if let Some(active) = &mut self.active {
                        active.ux.retry_deadline = None;
                        active.ux.samples = crate::route_policy::SpeedWindow::new(
                            Instant::now(),
                            record.task.downloaded,
                            self.route_ux.policy.slow_window,
                        );
                    }
                }
            }
            EngineUpdate::Checkpoint(checkpoint) => {
                let downloaded = checkpoint.downloaded();
                if let Some(active) = &mut self.active {
                    active.ux.samples.record(Instant::now(), downloaded);
                }
                record.task.speed = if stopping {
                    0.0
                } else {
                    self.active
                        .as_ref()
                        .and_then(|active| {
                            active
                                .ux
                                .samples
                                .average(Instant::now(), Duration::from_secs(5), false)
                        })
                        .unwrap_or(0.0)
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
            }
        }
        self.persist(index, record)
    }

    fn finish(&mut self, outcome: Result<DownloadedFile>) -> Result<()> {
        let Some(mut active) = self.active.take() else {
            return Ok(());
        };
        let index = self.index(&active.id)?;
        let mut record = self.records[index].clone();
        record.task.speed = 0.0;
        record.task.eta = None;
        record.task.details.elapsed_ms = active.elapsed.value_at(Instant::now());
        self.cancel_suggestion("下载已结束，本次换线未执行");
        record.task.details.route_suggestion = None;
        record.task.details.retry_info = None;
        if let Some(pending) = active.ux.switch.take() {
            if outcome
                .as_ref()
                .is_err_and(|error| error.kind == ErrorKind::Cancelled)
                && !self.closing
            {
                match Workspace::new(&record.task.directory, &active.id)
                    .and_then(|workspace| workspace.clear_parts())
                {
                    Ok(()) => {
                        record.checkpoint = None;
                        record.task.downloaded = 0;
                        record.task.error = None;
                        record.task.status = TaskStatus::Queued;
                        if let Err(error) = self.persist(index, record) {
                            let _ = pending
                                .reply
                                .send(Err(DownloadError::new(error.kind, error.message.clone())));
                            return Err(error);
                        }
                        self.route_ux.first_route = Some((active.id, pending.route));
                        let _ = pending.reply.send(Ok(self.snapshot()));
                        return Ok(());
                    }
                    Err(error) => {
                        record.task.status = TaskStatus::Failed;
                        record.task.error = Some(error.message.clone());
                        record.task.details.failure = Some(error.failure());
                        let _ = pending.reply.send(Err(error));
                        return self.persist(index, record);
                    }
                }
            }
            let _ = pending.reply.send(Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "任务已结束，本次换线未执行",
            )));
        }
        let outcome = if active.ux.recovery_expired && outcome.is_err() {
            Err(route_ux::recovery_exhausted())
        } else {
            outcome
        };
        match outcome {
            Ok(file) => {
                self.clear_recovery(&active.id);
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
            Err(error)
                if error.recoverable
                    && active.stop.is_none()
                    && !self.closing
                    && !active.ux.recovery_expired =>
            {
                self.wait_for_recovery(&mut record, &error);
            }
            Err(error) => {
                record.task.status = TaskStatus::Failed;
                record.task.details.failure = Some(error.failure());
                if !error.route_failures.is_empty() {
                    record.task.details.route_failures = error.route_failures.clone();
                }
                if error.kind == ErrorKind::Integrity {
                    record.task.verification = Verification::Failed;
                }
                record.task.error = Some(error.to_string());
            }
        }
        if record.task.status != TaskStatus::WaitingNetwork {
            self.clear_recovery(&active.id);
        }
        self.persist(index, record)
    }

    fn begin_shutdown(&mut self) -> Result<()> {
        self.cancel_suggestion("应用正在退出，本次换线未执行");
        for index in 0..self.records.len() {
            if matches!(
                self.records[index].task.status,
                TaskStatus::Queued | TaskStatus::WaitingNetwork
            ) {
                let mut record = self.records[index].clone();
                record.task.status = TaskStatus::Paused;
                self.clear_recovery(&record.task.id);
                record.task.details.retry_info = None;
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

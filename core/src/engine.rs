use crate::{
    error::{DownloadError, ErrorKind, Result},
    files::Workspace,
    model::{
        Checkpoint, DownloadedFile, EngineUpdate, Part, RetryInfo, RouteFailure, StoredTask,
        TaskStatus,
    },
    network::{self, Network, Probe},
    source::parse_release_url,
};
use futures_util::future::join_all;
use reqwest::{header, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{mpsc, oneshot, Mutex},
};
use tokio_util::sync::CancellationToken;

pub struct ProgressMessage {
    pub update: EngineUpdate,
    pub ack: oneshot::Sender<Result<()>>,
}

#[derive(Clone, Default)]
pub struct Reporter {
    sender: Option<mpsc::Sender<ProgressMessage>>,
}

impl Reporter {
    pub fn channel() -> (Self, mpsc::Receiver<ProgressMessage>) {
        let (sender, receiver) = mpsc::channel(16);
        (
            Self {
                sender: Some(sender),
            },
            receiver,
        )
    }

    pub async fn emit(&self, update: EngineUpdate) -> Result<()> {
        if let Some(sender) = &self.sender {
            let (ack, received) = oneshot::channel();
            sender
                .send(ProgressMessage { update, ack })
                .await
                .map_err(|_| DownloadError::new(ErrorKind::Storage, "任务管理器已关闭"))?;
            received
                .await
                .map_err(|_| DownloadError::new(ErrorKind::Storage, "恢复信息未能确认保存"))??;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct Engine {
    pub(crate) network: Network,
    pub(crate) limiter: Arc<crate::limiter::RateLimiter>,
}

#[derive(Default)]
pub(crate) struct RunOptions {
    pub first_route: Option<String>,
    pub cooldowns: HashMap<String, Instant>,
}

async fn report_cooldown(
    route: &network::Route,
    error: &DownloadError,
    reporter: &Reporter,
) -> Result<()> {
    if let Some(wait) = error.retry_after {
        // 极端等待值也不能溢出单调时钟；无法表达时保守冷却一年。
        let now = Instant::now();
        let deadline = now
            .checked_add(wait)
            .unwrap_or(now + Duration::from_secs(365 * 86400));
        reporter
            .emit(EngineUpdate::Cooldown(route.id.clone(), deadline))
            .await?;
    }
    Ok(())
}

fn route_failure(route: &network::Route, error: &DownloadError) -> RouteFailure {
    RouteFailure {
        route_id: route.id.clone(),
        route_name: route.name.clone(),
        message: error.message.clone(),
        temporary: error.retryable(),
    }
}

impl Engine {
    pub fn network(&self) -> &Network {
        &self.network
    }

    pub fn production() -> Result<Self> {
        Ok(Self {
            network: Network::production()?,
            limiter: Arc::new(crate::limiter::RateLimiter::default()),
        })
    }

    pub async fn run(
        &self,
        record: StoredTask,
        token: CancellationToken,
        reporter: Reporter,
    ) -> Result<DownloadedFile> {
        self.run_with(record, token, reporter, RunOptions::default())
            .await
    }

    pub(crate) async fn run_with(
        &self,
        record: StoredTask,
        token: CancellationToken,
        reporter: Reporter,
        options: RunOptions,
    ) -> Result<DownloadedFile> {
        let source = parse_release_url(&record.task.url)?;
        let workspace = Workspace::new(&record.task.directory, &record.task.id)?;
        if let Some(publication) = record
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.publication.clone())
        {
            let work = workspace.clone();
            let cancel = token.clone();
            reporter
                .emit(EngineUpdate::Status(
                    TaskStatus::Verifying,
                    record.task.route.clone(),
                ))
                .await?;
            if let Some(file) =
                tokio::task::spawn_blocking(move || work.recover_published(&publication, &cancel))
                    .await
                    .map_err(join_error)??
            {
                return Ok(file);
            }
        }
        reporter
            .emit(EngineUpdate::Status(TaskStatus::Probing, None))
            .await?;
        crate::preflight::check_async(
            record.task.directory.clone(),
            record.task.total,
            record.task.downloaded,
        )
        .await?;
        let metadata = self.network.metadata(&source, &token).await.or_else(|| {
            record.task.total.map(|size| network::Metadata {
                asset_id: record.task.details.asset_id,
                size,
                sha256: record.task.details.official_sha256.clone(),
            })
        });
        if let Some(metadata) = &metadata {
            reporter
                .emit(EngineUpdate::Metadata(metadata.clone()))
                .await?;
        }
        if token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let routes: Vec<_> = self
            .network
            .routes
            .iter()
            .filter(|r| {
                record
                    .task
                    .details
                    .preferred_route
                    .as_ref()
                    .is_none_or(|id| *id == r.id)
            })
            .cloned()
            .collect();
        let probes = join_all(routes.iter().map(|route| async {
            if let Some(deadline) = options
                .cooldowns
                .get(&route.id)
                .filter(|deadline| **deadline > Instant::now())
            {
                let mut error =
                    DownloadError::new(ErrorKind::Network, "服务端要求稍后重试，线路正在冷却");
                error.retry_after = Some(deadline.saturating_duration_since(Instant::now()));
                return Err(error);
            }
            self.network
                .probe(route.clone(), &source, metadata.as_ref(), &token)
                .await
        }))
        .await;
        if token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let mut errors = Vec::new();
        let mut failures = Vec::new();
        let mut last_kind = ErrorKind::Network;
        let mut candidates = Vec::new();
        let reports = routes
            .iter()
            .zip(&probes)
            .map(|(r, p)| network::report(r, p))
            .collect();
        reporter.emit(EngineUpdate::Diagnostics(reports)).await?;
        for (route, result) in routes.iter().zip(probes) {
            match result {
                Ok(probe) => candidates.push(probe),
                Err(error) => {
                    report_cooldown(route, &error, &reporter).await?;
                    failures.push(route_failure(route, &error));
                    last_kind = error.kind;
                    errors.push(format!("{}：{}", route.name, error.message));
                }
            }
        }
        candidates.sort_by(|left, right| right.throughput().total_cmp(&left.throughput()));
        let expected = metadata.as_ref().and_then(|value| value.sha256.clone());
        let mut previous = record.checkpoint;
        let mut last_route = previous
            .as_ref()
            .map(|checkpoint| checkpoint.route_id.clone());
        if let Some(checkpoint) = &previous {
            if let Some(index) = candidates
                .iter()
                .position(|probe| can_resume(checkpoint, probe, expected.as_deref()))
            {
                // 已下载数据可以复用时，优先完成原线路，避免为短时测速结果丢弃进度。
                let candidate = candidates.remove(index);
                candidates.insert(0, candidate);
            }
        }
        if let Some(index) = options
            .first_route
            .as_ref()
            .and_then(|id| candidates.iter().position(|probe| &probe.route.id == id))
        {
            let candidate = candidates.remove(index);
            candidates.insert(0, candidate);
        }
        for probe in candidates {
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            let mut checkpoint = match previous.take() {
                Some(checkpoint) if can_resume(&checkpoint, &probe, expected.as_deref()) => {
                    let checked = checkpoint.clone();
                    let work = workspace.clone();
                    let cancel = token.clone();
                    if tokio::task::spawn_blocking(move || work.verify_parts(&checked, &cancel))
                        .await
                        .map_err(join_error)??
                    {
                        checkpoint
                    } else {
                        fresh_checkpoint(&probe, expected.clone())
                    }
                }
                _ => fresh_checkpoint(&probe, expected.clone()),
            };
            if let Some(expected) = &expected {
                checkpoint.expected_sha256 = Some(expected.clone());
            }
            if checkpoint.downloaded() == 0 {
                workspace.clear_parts()?;
            }
            crate::preflight::check_async(
                record.task.directory.clone(),
                probe.total,
                checkpoint.downloaded(),
            )
            .await?;
            let shared = Arc::new(Mutex::new(checkpoint));
            reporter
                .emit(EngineUpdate::Checkpoint(shared.lock().await.clone()))
                .await?;
            let switching = last_route.as_ref().is_some_and(|id| *id != probe.route.id);
            reporter
                .emit(EngineUpdate::SelectedRoute(
                    probe.route.id.clone(),
                    probe.throughput(),
                    switching,
                ))
                .await?;
            last_route = Some(probe.route.id.clone());
            for attempt in 0..3 {
                reporter
                    .emit(EngineUpdate::Status(
                        TaskStatus::Downloading,
                        Some(probe.route.name.clone()),
                    ))
                    .await?;
                let context = Transfer {
                    network: self.network.clone(),
                    url: probe.route.url(&source),
                    filename: source.filename.clone(),
                    workspace: workspace.clone(),
                    checkpoint: shared.clone(),
                    reporter: reporter.clone(),
                    limiter: self.limiter.clone(),
                };
                let outcome = context.download(token.clone()).await;
                match outcome {
                    Ok(()) => {
                        reporter
                            .emit(EngineUpdate::Status(
                                TaskStatus::Verifying,
                                Some(probe.route.name.clone()),
                            ))
                            .await?;
                        let checked = shared.lock().await.clone();
                        let work = workspace.clone();
                        let filename = source.filename.clone();
                        let cancel = token.clone();
                        let publisher = reporter.clone();
                        let publish_checkpoint = shared.clone();
                        let runtime = tokio::runtime::Handle::current();
                        let result = tokio::task::spawn_blocking(move || {
                            work.assemble(&checked, &filename, &cancel, |publication| {
                                runtime.block_on(async {
                                    let mut checkpoint = publish_checkpoint.lock().await;
                                    checkpoint.publication = Some(publication);
                                    publisher
                                        .emit(EngineUpdate::Checkpoint(checkpoint.clone()))
                                        .await
                                })
                            })
                        })
                        .await
                        .map_err(join_error)?;
                        match result {
                            Ok(file) => return Ok(file),
                            Err(error) if error.kind == ErrorKind::Integrity => {
                                failures.push(route_failure(&probe.route, &error));
                                last_kind = error.kind;
                                errors.push(format!("{}：{}", probe.route.name, error));
                                break;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind,
                            ErrorKind::Cancelled
                                | ErrorKind::Io
                                | ErrorKind::Storage
                                | ErrorKind::Permission
                                | ErrorKind::DiskSpace
                        ) =>
                    {
                        return Err(error)
                    }
                    Err(error) => {
                        report_cooldown(&probe.route, &error, &reporter).await?;
                        if last_kind != ErrorKind::Integrity {
                            last_kind = error.kind;
                        }
                        if !error.retryable()
                            || attempt == 2
                            || error
                                .retry_after
                                .is_some_and(|duration| duration > Duration::from_secs(30))
                        {
                            failures.push(route_failure(&probe.route, &error));
                            errors.push(format!("{}：{}", probe.route.name, error));
                            break;
                        }
                        reporter
                            .emit(EngineUpdate::Status(
                                TaskStatus::Retrying,
                                Some(probe.route.name.clone()),
                            ))
                            .await?;
                        let wait = error
                            .retry_after
                            .unwrap_or(self.network.retry_delay * (1 << attempt));
                        let preserves_progress = {
                            let checkpoint = shared.lock().await;
                            can_resume(&checkpoint, &probe, expected.as_deref())
                        };
                        reporter
                            .emit(EngineUpdate::Retry(
                                RetryInfo {
                                    phase: "retrying".into(),
                                    attempt: attempt + 2,
                                    max_attempts: 3,
                                    reason: format!(
                                        "{}；{}",
                                        error.message,
                                        if preserves_progress {
                                            "将尝试保留进度续传"
                                        } else {
                                            "当前线路不满足续传条件，将重新下载"
                                        }
                                    ),
                                    retry_in_ms: crate::route_policy::millis(wait),
                                },
                                wait,
                            ))
                            .await?;
                        tokio::select! { _ = token.cancelled() => return Err(DownloadError::cancelled()), _ = tokio::time::sleep(wait) => () }
                        let current = shared.lock().await.clone();
                        if !can_resume(&current, &probe, expected.as_deref()) {
                            workspace.clear_parts()?;
                            *shared.lock().await = fresh_checkpoint(&probe, expected.clone());
                            reporter
                                .emit(EngineUpdate::Checkpoint(shared.lock().await.clone()))
                                .await?;
                        }
                    }
                }
            }
        }
        let recoverable =
            last_kind != ErrorKind::Integrity && failures.iter().any(|failure| failure.temporary);
        let mut error = DownloadError::new(
            if recoverable {
                ErrorKind::Network
            } else {
                last_kind
            },
            format!(
                "{}。{}",
                if record.task.details.preferred_route.is_some() {
                    "指定线路未完成下载，可暂停后切回自动模式"
                } else {
                    "所有线路均未完成下载"
                },
                errors.join("；")
            ),
        );
        error.recoverable = recoverable;
        error.route_failures = failures;
        Err(error)
    }
}

pub fn can_resume(checkpoint: &Checkpoint, probe: &Probe, expected: Option<&str>) -> bool {
    if checkpoint.route_id != probe.route.id
        || !probe.range_supported
        || !checkpoint.range_supported
        || checkpoint.total.is_none()
        || checkpoint.total != probe.total
    {
        return false;
    }
    if let (Some(old), Some(new)) = (checkpoint.expected_sha256.as_deref(), expected) {
        return old == new;
    }
    checkpoint.etag.is_some() && checkpoint.etag == probe.etag
}

fn fresh_checkpoint(probe: &Probe, expected_sha256: Option<String>) -> Checkpoint {
    let empty_sha = format!("{:x}", Sha256::digest([]));
    let parts =
        if probe.range_supported && probe.total.is_some_and(|total| total >= 16 * 1024 * 1024) {
            let total = probe.total.unwrap_or_default();
            vec![
                Part {
                    start: 0,
                    end: Some(total / 2 - 1),
                    downloaded: 0,
                    sha256: empty_sha.clone(),
                },
                Part {
                    start: total / 2,
                    end: Some(total - 1),
                    downloaded: 0,
                    sha256: empty_sha,
                },
            ]
        } else {
            vec![Part {
                start: 0,
                end: probe.total.and_then(|total| total.checked_sub(1)),
                downloaded: 0,
                sha256: empty_sha,
            }]
        };
    Checkpoint {
        route_id: probe.route.id.clone(),
        total: probe.total,
        etag: probe.etag.clone(),
        expected_sha256,
        range_supported: probe.range_supported,
        parts,
        publication: None,
    }
}

struct Transfer {
    network: Network,
    url: String,
    filename: String,
    workspace: Workspace,
    checkpoint: Arc<Mutex<Checkpoint>>,
    reporter: Reporter,
    limiter: Arc<crate::limiter::RateLimiter>,
}

impl Transfer {
    async fn download(&self, token: CancellationToken) -> Result<()> {
        let group = token.child_token();
        let count = self.checkpoint.lock().await.parts.len();
        let work = (0..count).map(|index| {
            let group = group.clone();
            async move {
                let result = self.part(index, group.clone()).await;
                if result.is_err() {
                    group.cancel();
                }
                result
            }
        });
        // 等待所有写入结束后再重试或换线，取消时也不遗留后台写入。
        let outcomes = join_all(work).await;
        if token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let mut errors: Vec<_> = outcomes
            .into_iter()
            .filter_map(std::result::Result::err)
            .collect();
        if let Some(index) = errors
            .iter()
            .position(|error| error.kind != ErrorKind::Cancelled)
        {
            return Err(errors.remove(index));
        }
        errors.pop().map_or(Ok(()), Err)
    }

    async fn part(&self, index: usize, token: CancellationToken) -> Result<()> {
        let checkpoint = self.checkpoint.lock().await.clone();
        let mut part = checkpoint.parts[index].clone();
        let path = self.workspace.part(index)?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        file.set_len(part.downloaded).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 256 * 1024];
        let mut remaining = part.downloaded;
        while remaining > 0 {
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            let count = buffer.len().min(remaining as usize);
            file.read_exact(&mut buffer[..count]).await?;
            hasher.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if format!("{:x}", hasher.clone().finalize()) != part.sha256 {
            return Err(DownloadError::new(
                ErrorKind::Integrity,
                "分片在重试前发生变化，已停止续传",
            ));
        }
        if part
            .length()
            .is_some_and(|length| length == part.downloaded)
            || checkpoint.total == Some(0)
        {
            return self.commit(index, part, &hasher, &mut file).await;
        }
        file.seek(std::io::SeekFrom::Start(part.downloaded)).await?;
        let start = part.start + part.downloaded;
        let mut request = self
            .network
            .client
            .get(&self.url)
            .header(header::ACCEPT_ENCODING, "identity");
        let ranged = checkpoint.range_supported && part.end.is_some();
        if ranged {
            request = request.header(
                header::RANGE,
                format!("bytes={start}-{}", part.end.unwrap_or_default()),
            );
            if let Some(etag) = &checkpoint.etag {
                request = request.header(header::IF_RANGE, etag);
            }
        }
        let mut response = tokio::select! {
            _ = token.cancelled() => return Err(DownloadError::cancelled()),
            response = tokio::time::timeout(self.network.idle_timeout, request.send()) => response.map_err(|_| DownloadError::new(ErrorKind::Network, "等待下载响应超时"))??,
        };
        network::check_status(&response)?;
        network::check_encoding(&response)?;
        if ranged {
            if response.status() != StatusCode::PARTIAL_CONTENT {
                return Err(network::protocol("服务器忽略了分段请求，已停止追加内容"));
            }
            let range = network::content_range(&response)?;
            if range.start != start
                || Some(range.end) != part.end
                || Some(range.total) != checkpoint.total
            {
                return Err(network::protocol("服务器返回的字节范围或总长度发生变化"));
            }
            if response
                .content_length()
                .is_some_and(|length| length != range.end - range.start + 1)
            {
                return Err(network::protocol("分段响应长度不一致"));
            }
        } else {
            if response.status() != StatusCode::OK {
                return Err(network::protocol("完整下载收到意外的分段响应"));
            }
            if let (Some(length), Some(total)) = (response.content_length(), checkpoint.total) {
                if length != total {
                    return Err(network::protocol("文件大小在探测后发生变化"));
                }
            }
        }
        if let (Some(expected), Some(actual)) = (&checkpoint.etag, network::strong_etag(&response))
        {
            if *expected != actual {
                return Err(network::protocol("文件 ETag 在下载期间发生变化"));
            }
        }
        let html_type = network::is_html_type(&response);
        let mut first = true;
        let mut announced_data = false;
        let mut last_commit = Instant::now();
        let outcome = loop {
            let chunk = tokio::select! {
                _ = token.cancelled() => break Err(DownloadError::cancelled()),
                result = tokio::time::timeout(self.network.idle_timeout, response.chunk()) => match result {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => break Err(error.into()),
                    Err(_) => break Err(DownloadError::new(ErrorKind::Network, "连续 30 秒未收到下载数据")),
                }
            };
            let Some(chunk) = chunk else {
                break Ok(());
            };
            if first {
                if let Err(error) = network::reject_error_page(&chunk, html_type, &self.filename) {
                    break Err(error);
                }
                first = false;
            }
            if part
                .length()
                .is_some_and(|length| part.downloaded + chunk.len() as u64 > length)
            {
                break Err(network::protocol("收到的数据超出了请求范围"));
            }
            if let Err(error) = self.limiter.consume(chunk.len(), &token).await {
                break Err(error);
            }
            if let Err(error) = file.write_all(&chunk).await {
                break Err(error.into());
            }
            if !announced_data && !chunk.is_empty() {
                if let Err(error) = self.reporter.emit(EngineUpdate::DataReceived).await {
                    break Err(error);
                }
                announced_data = true;
            }
            hasher.update(&chunk);
            part.downloaded += chunk.len() as u64;
            if last_commit.elapsed() >= Duration::from_millis(500) {
                self.commit(index, part.clone(), &hasher, &mut file).await?;
                last_commit = Instant::now();
            }
        };
        self.commit(index, part.clone(), &hasher, &mut file).await?;
        outcome?;
        if part
            .length()
            .is_some_and(|length| part.downloaded != length)
        {
            return Err(network::protocol("下载连接提前结束，文件不完整"));
        }
        Ok(())
    }

    async fn commit(
        &self,
        index: usize,
        mut part: Part,
        hasher: &Sha256,
        file: &mut tokio::fs::File,
    ) -> Result<()> {
        file.flush().await?;
        file.sync_data().await?;
        part.sha256 = format!("{:x}", hasher.clone().finalize());
        let mut checkpoint = self.checkpoint.lock().await;
        checkpoint.parts[index] = part;
        // 先确保分片持久化，再等待数据库确认，防止恢复点超过实际落盘范围。
        self.reporter
            .emit(EngineUpdate::Checkpoint(checkpoint.clone()))
            .await
    }
}

fn join_error(error: tokio::task::JoinError) -> DownloadError {
    DownloadError::new(ErrorKind::Io, format!("文件处理任务异常：{error}"))
}

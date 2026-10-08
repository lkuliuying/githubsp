use super::*;
use crate::{
    model::{RecoveryInfo, RetryInfo, RouteSuggestion},
    network::{Metadata, Probe, Route},
    route_policy::{low_speed, millis, savings, RecoveryClock, SpeedWindow},
};
use std::collections::HashMap;

pub(super) struct RouteRuntime {
    pub policy: RoutePolicy,
    pub recoveries: HashMap<String, RecoveryClock>,
    pub cooldowns: HashMap<String, Instant>,
    ready_since: HashMap<String, Instant>,
    dismissed_until: HashMap<String, Instant>,
    next_probe: HashMap<String, Instant>,
    pub job: Option<SuggestionJob>,
    pub first_route: Option<(String, String)>,
}

impl RouteRuntime {
    pub fn new(policy: RoutePolicy) -> Self {
        Self {
            policy,
            recoveries: HashMap::new(),
            cooldowns: HashMap::new(),
            ready_since: HashMap::new(),
            dismissed_until: HashMap::new(),
            next_probe: HashMap::new(),
            job: None,
            first_route: None,
        }
    }
}

pub(super) struct PendingSwitch {
    pub route: String,
    pub reply: oneshot::Sender<Result<Snapshot>>,
}

pub(super) struct ActiveRouteState {
    pub generation: String,
    pub samples: SpeedWindow,
    pub selected: Option<(String, f64)>,
    pub retry_deadline: Option<Instant>,
    pub switch: Option<PendingSwitch>,
    pub recovery_expired: bool,
}

impl ActiveRouteState {
    pub fn new(now: Instant, bytes: u64, policy: &RoutePolicy) -> Self {
        Self {
            generation: uuid::Uuid::new_v4().to_string(),
            samples: SpeedWindow::new(now, bytes, policy.slow_window),
            selected: None,
            retry_deadline: None,
            switch: None,
            recovery_expired: false,
        }
    }
}

type ProbeBatch = Vec<(Route, Result<Probe>)>;
pub(super) type SuggestionResult = std::result::Result<Result<ProbeBatch>, tokio::task::JoinError>;

pub(super) struct SuggestionJob {
    pub worker: tokio::task::JoinHandle<Result<ProbeBatch>>,
    task_id: String,
    generation: String,
    current_route: String,
    token: CancellationToken,
    reply: Option<oneshot::Sender<Result<Snapshot>>>,
}

impl Actor {
    pub(super) fn account_recovery_time(&mut self) {
        let now = Instant::now();
        for (id, clock) in &mut self.route_ux.recoveries {
            clock.account(
                now,
                self.active.as_ref().is_some_and(|active| active.id != *id),
            );
        }
    }

    fn recovery_delay(&self, task: &Task, clock: &RecoveryClock) -> Duration {
        let now = Instant::now();
        let cooldown = self
            .engine
            .network
            .routes
            .iter()
            .filter(|route| {
                task.details
                    .preferred_route
                    .as_ref()
                    .is_none_or(|id| *id == route.id)
            })
            .map(|route| {
                self.route_ux
                    .cooldowns
                    .get(&route.id)
                    .map_or(Duration::ZERO, |until| until.saturating_duration_since(now))
            })
            .min()
            .unwrap_or(Duration::ZERO);
        clock.wait.max(cooldown)
    }

    pub(super) fn project_route_state(&self, task: &mut Task) {
        let now = Instant::now();
        if let Some(clock) = self.route_ux.recoveries.get(&task.id) {
            task.details.recovery_info = Some(RecoveryInfo {
                remaining_ms: millis(clock.remaining),
                retry_in_ms: millis(self.recovery_delay(task, clock)),
                waiting_for_slot: task.status == TaskStatus::WaitingNetwork
                    && (self.active.is_some()
                        || self
                            .records
                            .iter()
                            .any(|record| record.task.status == TaskStatus::Queued)),
            });
        }
        if let Some(active) = self.active.as_ref().filter(|active| active.id == task.id) {
            if let (Some(info), Some(deadline)) =
                (&mut task.details.retry_info, active.ux.retry_deadline)
            {
                info.retry_in_ms = millis(deadline.saturating_duration_since(now));
            }
            if task.status == TaskStatus::Downloading {
                task.speed = active
                    .ux
                    .samples
                    .average(now, Duration::from_secs(5), false)
                    .unwrap_or(0.0);
                task.eta = task.total.filter(|_| task.speed > 0.0).map(|total| {
                    (total.saturating_sub(task.downloaded) as f64 / task.speed).ceil() as u64
                });
            }
        }
    }

    pub(super) fn next_recovery(&self) -> Option<usize> {
        self.records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.task.status == TaskStatus::WaitingNetwork)
            .filter_map(|(index, record)| {
                let clock = self.route_ux.recoveries.get(&record.task.id)?;
                (!clock.remaining.is_zero() && self.recovery_delay(&record.task, clock).is_zero())
                    .then_some((
                        index,
                        self.route_ux
                            .ready_since
                            .get(&record.task.id)
                            .copied()
                            .unwrap_or_else(Instant::now),
                        record.task.details.queue_position,
                    ))
            })
            .min_by_key(|(_, ready, position)| (*ready, *position))
            .map(|(index, _, _)| index)
    }

    pub(super) fn wait_for_recovery(&mut self, record: &mut StoredTask, error: &DownloadError) {
        let jitter = uuid::Uuid::new_v4().as_bytes()[0];
        let policy = &self.route_ux.policy;
        self.route_ux
            .recoveries
            .entry(record.task.id.clone())
            .and_modify(|clock| clock.failed_round(policy, jitter))
            .or_insert_with(|| RecoveryClock::new(Instant::now(), policy, jitter));
        self.route_ux.ready_since.remove(&record.task.id);
        record.task.status = TaskStatus::WaitingNetwork;
        record.task.error = None;
        record.task.details.failure = None;
        record.task.details.route_failures = error.route_failures.clone();
        record.task.details.retry_info = Some(RetryInfo {
            phase: "recovering".into(),
            attempt: 0,
            max_attempts: 0,
            reason: "线路暂时不可用，正在等待自动恢复".into(),
            retry_in_ms: 0,
        });
    }

    pub(super) fn clear_recovery(&mut self, id: &str) {
        self.route_ux.recoveries.remove(id);
        self.route_ux.ready_since.remove(id);
    }

    pub(super) fn cancel_suggestion(&mut self, reason: &str) {
        if let Some(mut job) = self.route_ux.job.take() {
            job.token.cancel();
            // 工作任务只做只读探测，取消后不会留下文件写入或未回复的调用。
            job.worker.abort();
            if let Some(reply) = job.reply.take() {
                let _ = reply.send(Err(DownloadError::new(ErrorKind::InvalidInput, reason)));
            }
        }
    }

    pub(super) fn stop_route_work(&mut self, id: &str) {
        self.clear_recovery(id);
        if self
            .route_ux
            .job
            .as_ref()
            .is_some_and(|job| job.task_id == id)
        {
            self.cancel_suggestion("任务已停止，本次换线未执行");
        }
        if let Some(active) = self.active.as_mut().filter(|active| active.id == id) {
            if let Some(pending) = active.ux.switch.take() {
                let _ = pending.reply.send(Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    "任务已停止，本次换线未执行",
                )));
            }
        }
    }

    pub(super) fn tick_routes(&mut self) -> Result<()> {
        let now = Instant::now();
        self.route_ux
            .dismissed_until
            .retain(|_, until| *until > now);
        self.route_ux.next_probe.retain(|_, until| *until > now);
        let ids: Vec<_> = self.route_ux.recoveries.keys().cloned().collect();
        for id in ids {
            let index = self.index(&id)?;
            let clock = &self.route_ux.recoveries[&id];
            if clock.remaining.is_zero() {
                if let Some(active) = self.active.as_mut().filter(|active| active.id == id) {
                    active.ux.recovery_expired = true;
                    active.token.cancel();
                } else {
                    self.clear_recovery(&id);
                    let mut record = self.records[index].clone();
                    let error = recovery_exhausted();
                    record.task.status = TaskStatus::Failed;
                    record.task.error = Some(error.message.clone());
                    record.task.details.failure = Some(error.failure());
                    record.task.details.retry_info = None;
                    self.persist(index, record)?;
                }
            } else if self
                .recovery_delay(&self.records[index].task, clock)
                .is_zero()
            {
                self.route_ux.ready_since.entry(id).or_insert(now);
            } else {
                self.route_ux.ready_since.remove(&id);
            }
        }
        if let Some(active) = &mut self.active {
            let task = self
                .records
                .iter()
                .find(|record| record.task.id == active.id)
                .map(|record| &record.task);
            if let Some(task) = task.filter(|task| task.status == TaskStatus::Downloading) {
                active.ux.samples.record(now, task.downloaded);
            }
        }
        self.maybe_suggest();
        Ok(())
    }

    fn slow_context(&self) -> Option<(usize, String, f64)> {
        let active = self.active.as_ref()?;
        let index = self
            .records
            .iter()
            .position(|record| record.task.id == active.id)?;
        let task = &self.records[index].task;
        let (route, baseline) = active.ux.selected.as_ref()?;
        let speed =
            active
                .ux
                .samples
                .average(Instant::now(), self.route_ux.policy.slow_window, true);
        (task.status == TaskStatus::Downloading
            && task.details.preferred_route.is_none()
            && active.stop.is_none()
            && low_speed(
                task.total,
                task.downloaded,
                speed,
                *baseline,
                self.settings.limit_kib != 0,
            ))
        .then(|| (index, route.clone(), speed.unwrap_or(0.0)))
    }

    fn maybe_suggest(&mut self) {
        let Some((index, route, _)) = self.slow_context() else {
            if let Some(active) = &self.active {
                if let Some(record) = self
                    .records
                    .iter_mut()
                    .find(|record| record.task.id == active.id)
                {
                    record.task.details.route_suggestion = None;
                }
            }
            return;
        };
        let now = Instant::now();
        let task = &self.records[index].task;
        if self.diagnostic.is_some()
            || self.route_ux.job.is_some()
            || task.details.route_suggestion.is_some()
            || self
                .route_ux
                .next_probe
                .get(&task.id)
                .is_some_and(|until| *until > now)
            || self
                .route_ux
                .dismissed_until
                .get(&task.id)
                .is_some_and(|until| *until > now)
        {
            return;
        }
        let routes = self
            .engine
            .network
            .routes
            .iter()
            .filter(|candidate| {
                candidate.id != route
                    && self
                        .route_ux
                        .cooldowns
                        .get(&candidate.id)
                        .is_none_or(|until| *until <= now)
            })
            .cloned()
            .collect();
        self.spawn_suggestion(index, route, routes, None);
    }

    fn spawn_suggestion(
        &mut self,
        index: usize,
        current_route: String,
        routes: Vec<Route>,
        reply: Option<oneshot::Sender<Result<Snapshot>>>,
    ) {
        let task = &self.records[index].task;
        let source = match parse_release_url(&task.url) {
            Ok(source) => source,
            Err(error) => {
                if let Some(reply) = reply {
                    let _ = reply.send(Err(error));
                }
                return;
            }
        };
        let Some(active) = &mut self.active else {
            return;
        };
        self.route_ux.next_probe.insert(
            task.id.clone(),
            Instant::now() + self.route_ux.policy.probe_interval,
        );
        let metadata = task.total.map(|size| Metadata {
            size,
            asset_id: task.details.asset_id,
            sha256: task.details.official_sha256.clone(),
        });
        let token = active.token.child_token();
        let cancel = token.clone();
        let network = self.engine.network.clone();
        let worker = tokio::spawn(async move {
            let mut results = Vec::new();
            for route in routes {
                if cancel.is_cancelled() {
                    return Err(DownloadError::cancelled());
                }
                let result = network
                    .probe(route.clone(), &source, metadata.as_ref(), &cancel)
                    .await;
                results.push((route, result));
            }
            Ok(results)
        });
        self.route_ux.job = Some(SuggestionJob {
            worker,
            task_id: task.id.clone(),
            generation: active.ux.generation.clone(),
            current_route,
            token,
            reply,
        });
    }

    pub(super) fn confirm_suggestion(
        &mut self,
        id: &str,
        suggestion_id: &str,
        reply: oneshot::Sender<Result<Snapshot>>,
    ) {
        let selection = (|| {
            let (index, current, _) = self.slow_context().ok_or_else(stale_suggestion)?;
            let task = &self.records[index].task;
            let suggestion = task
                .details
                .route_suggestion
                .as_ref()
                .filter(|suggestion| task.id == id && suggestion.id == suggestion_id)
                .ok_or_else(stale_suggestion)?;
            if self.diagnostic.is_some() || self.route_ux.job.is_some() {
                return Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    "线路检测或换线确认正在进行，请稍后重试",
                ));
            }
            let route = self
                .engine
                .network
                .routes
                .iter()
                .find(|route| route.id == suggestion.route_id)
                .filter(|route| {
                    self.route_ux
                        .cooldowns
                        .get(&route.id)
                        .is_none_or(|until| *until <= Instant::now())
                })
                .cloned()
                .ok_or_else(stale_suggestion)?;
            Ok((index, current, route))
        })();
        match selection {
            Ok((index, current, route)) => {
                self.spawn_suggestion(index, current, vec![route], Some(reply))
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }

    pub(super) fn dismiss_suggestion(&mut self, id: &str, suggestion_id: &str) -> Result<()> {
        let index = self.index(id)?;
        if self.records[index]
            .task
            .details
            .route_suggestion
            .as_ref()
            .is_none_or(|suggestion| suggestion.id != suggestion_id)
        {
            return Err(stale_suggestion());
        }
        self.cancel_suggestion("已选择继续当前线路");
        self.route_ux.dismissed_until.insert(
            id.into(),
            Instant::now() + self.route_ux.policy.dismiss_interval,
        );
        self.records[index].task.details.route_suggestion = None;
        self.publish();
        Ok(())
    }

    pub(super) fn finish_suggestion(&mut self, result: SuggestionResult) {
        let Some(mut job) = self.route_ux.job.take() else {
            return;
        };
        let result = (|| {
            let batches = result.map_err(|_| {
                DownloadError::new(ErrorKind::Network, "备选线路检测意外停止，请重新检测")
            })??;
            let mut best: Option<Probe> = None;
            for (route, result) in batches {
                match result {
                    Ok(probe)
                        if best
                            .as_ref()
                            .is_none_or(|best| probe.throughput() > best.throughput()) =>
                    {
                        best = Some(probe)
                    }
                    Err(error) => {
                        if let Some(wait) = error.retry_after {
                            let now = Instant::now();
                            let until = now
                                .checked_add(wait)
                                .unwrap_or(now + Duration::from_secs(365 * 86400));
                            self.route_ux
                                .cooldowns
                                .entry(route.id)
                                .and_modify(|old| *old = (*old).max(until))
                                .or_insert(until);
                        }
                    }
                    _ => (),
                }
            }
            let best = best.ok_or_else(|| {
                DownloadError::new(ErrorKind::Network, "备选线路当前不可用，已保留原线路下载")
            })?;
            let (index, current, speed) = self.slow_context().ok_or_else(stale_suggestion)?;
            let task = &self.records[index].task;
            if task.id != job.task_id
                || current != job.current_route
                || self
                    .active
                    .as_ref()
                    .is_none_or(|active| active.ux.generation != job.generation)
            {
                return Err(stale_suggestion());
            }
            let (old, new) = savings(
                task.total.unwrap_or(0),
                task.downloaded,
                speed,
                best.throughput(),
            )
            .ok_or_else(stale_suggestion)?;
            if job.reply.is_some() {
                let mut record = self.records[index].clone();
                record.task.details.route_suggestion = None;
                record.task.details.retry_info = Some(RetryInfo {
                    phase: "switching".into(),
                    attempt: 1,
                    max_attempts: 3,
                    reason: format!("正在改用 {}，将重新下载", best.route.name),
                    retry_in_ms: 0,
                });
                record.task.status = TaskStatus::Pausing;
                self.persist(index, record)?;
                if let Some(active) = &mut self.active {
                    active.ux.switch = job.reply.take().map(|reply| PendingSwitch {
                        route: best.route.id,
                        reply,
                    });
                    active.stop = Some(Action::Pause);
                    active.token.cancel();
                }
            } else {
                self.records[index].task.details.route_suggestion = Some(RouteSuggestion {
                    id: uuid::Uuid::new_v4().to_string(),
                    route_id: best.route.id,
                    route_name: best.route.name,
                    current_seconds: old,
                    suggested_seconds: new,
                    restart_bytes: task.downloaded,
                });
                self.publish();
            }
            Ok(())
        })();
        if let Err(error) = result {
            if error.kind == ErrorKind::Storage {
                self.fail_storage(&error);
            }
            if let Some(reply) = job.reply.take() {
                if let Some(record) = self
                    .records
                    .iter_mut()
                    .find(|record| record.task.id == job.task_id)
                {
                    record.task.details.route_suggestion = None;
                    self.publish();
                }
                let _ = reply.send(Err(error));
            }
        }
    }
}

pub(super) fn recovery_exhausted() -> DownloadError {
    DownloadError::new(
        ErrorKind::Network,
        "自动恢复额度已用完，请检查网络后手动重试",
    )
}

fn stale_suggestion() -> DownloadError {
    DownloadError::new(
        ErrorKind::InvalidInput,
        "线路或下载进度已变化，当前换线收益不足；已保留原线路下载",
    )
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Probing,
    Downloading,
    Retrying,
    WaitingNetwork,
    Verifying,
    Pausing,
    Cancelling,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Probing => "probing",
            Self::Downloading => "downloading",
            Self::Retrying => "retrying",
            Self::WaitingNetwork => "waiting_network",
            Self::Verifying => "verifying",
            Self::Pausing => "pausing",
            Self::Cancelling => "cancelling",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn running(self) -> bool {
        matches!(
            self,
            Self::Probing
                | Self::Downloading
                | Self::Retrying
                | Self::Verifying
                | Self::Pausing
                | Self::Cancelling
        )
    }

    pub fn resumable(self) -> bool {
        matches!(self, Self::Paused | Self::Failed)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Pending,
    Verified,
    Unverified,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(default, flatten)]
    pub details: TaskDetails,
    pub id: String,
    pub url: String,
    pub filename: String,
    pub directory: PathBuf,
    pub status: TaskStatus,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: f64,
    pub eta: Option<u64>,
    pub route: Option<String>,
    pub verification: Verification,
    pub error: Option<String>,
    pub final_path: Option<PathBuf>,
    pub created_at: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub start: u64,
    pub end: Option<u64>,
    pub downloaded: u64,
    pub sha256: String,
}

impl Part {
    pub fn length(&self) -> Option<u64> {
        self.end.map(|end| end - self.start + 1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub route_id: String,
    pub total: Option<u64>,
    pub etag: Option<String>,
    pub expected_sha256: Option<String>,
    pub range_supported: bool,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub publication: Option<Publication>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub verified: bool,
}

impl Checkpoint {
    pub fn downloaded(&self) -> u64 {
        self.parts.iter().map(|part| part.downloaded).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTask {
    pub task: Task,
    pub checkpoint: Option<Checkpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub total_tasks: usize,
    #[serde(default)]
    pub completed_tasks: usize,
    #[serde(default)]
    pub history_revision: u64,
    pub last_directory: Option<String>,
    pub error: Option<String>,
    pub revision: u64,
    pub settings: Settings,
    pub queue_revision: u64,
    pub diagnostics: Vec<RouteReport>,
    #[serde(default)]
    pub diagnostic_context: Option<DiagnosticContext>,
    pub diagnosing: bool,
    pub notices: Vec<Notice>,
    pub favorites: Vec<Favorite>,
    #[serde(default)]
    pub watch_state: WatchState,
    #[serde(default)]
    pub checking: CheckProgress,
    #[serde(default)]
    pub api_status: ApiStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSource {
    Default,
    Input,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticContext {
    pub source: DiagnosticSource,
    pub filename: String,
}

#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub verified: bool,
}

#[derive(Debug, Clone)]
pub enum EngineUpdate {
    Status(TaskStatus, Option<String>),
    Checkpoint(Checkpoint),
    Diagnostics(Vec<RouteReport>),
    Metadata(crate::network::Metadata),
    Retry(RetryInfo, std::time::Duration),
    SelectedRoute(String, f64, bool),
    Cooldown(String, std::time::Instant),
    DataReceived,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TaskDetails {
    pub repository: Option<String>,
    pub tag: Option<String>,
    pub asset_id: Option<u64>,
    pub official_sha256: Option<String>,
    pub preferred_route: Option<String>,
    pub failure: Option<Failure>,
    pub completed_at: Option<u64>,
    pub elapsed_ms: Option<u64>,
    pub elapsed_is_partial: bool,
    pub queue_position: u64,
    pub retry_info: Option<RetryInfo>,
    pub recovery_info: Option<RecoveryInfo>,
    pub route_suggestion: Option<RouteSuggestion>,
    pub route_failures: Vec<RouteFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryInfo {
    pub phase: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub reason: String,
    pub retry_in_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInfo {
    pub remaining_ms: u64,
    pub retry_in_ms: u64,
    pub waiting_for_slot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSuggestion {
    pub id: String,
    pub route_id: String,
    pub route_name: String,
    pub current_seconds: u64,
    pub suggested_seconds: u64,
    pub restart_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteFailure {
    pub route_id: String,
    pub route_name: String,
    pub message: String,
    pub temporary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub category: String,
    pub message: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteReport {
    pub id: String,
    pub name: String,
    pub checked_at: u64,
    pub bytes_per_second: f64,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub limit_kib: u32,
    pub close_to_tray: bool,
    pub auto_check: bool,
    pub background_completion_notice: bool,
    /// 自动检查间隔，单位毫秒；只接受 [`Settings::CHECK_INTERVALS_MS`] 中的值。
    pub watch_interval_ms: u64,
    /// 发现新版本时是否弹窗、写托盘提示。
    pub watch_notifications: bool,
    /// 全局自动下载开关；单个收藏仍可通过关注规则覆盖。
    pub watch_auto_download: bool,
    /// 检查时是否把预发布版本纳入发现。
    pub watch_include_prerelease: bool,
    /// 自动下载附件模式：0 优先安装包，1 全部附件，2 不下载附件。
    pub watch_asset_mode: u8,
    /// 可选的附件名通配符，如 `*win*x64*.exe`。
    pub watch_asset_pattern: String,
    /// 是否写入本地日志文件。
    pub log_enabled: bool,
    /// 减少界面动效；系统关闭动画时也会生效。
    pub reduce_motion: bool,
    /// 打开版本页时按当前平台自动勾选附件。
    pub auto_match_platform: bool,
}

impl Settings {
    /// 6 小时，保持旧版本行为。
    pub const DEFAULT_WATCH_INTERVAL_MS: u64 = 6 * 60 * 60 * 1000;
    /// 15 分钟 / 1 小时 / 6 小时 / 24 小时。
    pub const CHECK_INTERVALS_MS: [u64; 4] = [900_000, 3_600_000, 21_600_000, 86_400_000];
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            limit_kib: 0,
            close_to_tray: false,
            auto_check: false,
            background_completion_notice: true,
            watch_interval_ms: Self::DEFAULT_WATCH_INTERVAL_MS,
            watch_notifications: true,
            watch_auto_download: false,
            watch_include_prerelease: false,
            watch_asset_mode: 0,
            watch_asset_pattern: String::new(),
            log_enabled: true,
            reduce_motion: false,
            auto_match_platform: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateOptions {
    pub asset_id: Option<u64>,
    pub size: Option<u64>,
    pub official_sha256: Option<String>,
    pub preferred_route: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub kind: NoticeKind,
    pub task_id: Option<String>,
    pub message: String,
    pub created_at: u64,
    #[serde(default)]
    pub completion: Option<CompletionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionSummary {
    pub filename: String,
    pub elapsed_ms: Option<u64>,
    pub elapsed_is_partial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    DownloadCompleted,
    DownloadFailed,
    FavoriteUpdated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    pub repository: String,
    pub latest: Option<ReleaseSummary>,
    pub seen: Vec<u64>,
    pub last_checked: Option<u64>,
    pub last_success: Option<u64>,
    pub next_check: Option<u64>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub id: u64,
    pub tag: String,
    pub url: String,
    #[serde(default)]
    pub published_at: Option<u64>,
    /// 来自 Atom 降级发现，附件需重新查询 API。
    #[serde(default)]
    pub atom: bool,
}

/// 单个收藏仓库的关注选项。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WatchRule {
    /// 新版本是否包含 GitHub 预发布版本。
    pub include_prerelease: bool,
    /// 命中新版本后自动把匹配附件加入下载队列（仅在“自动检查”开启时生效）。
    pub auto_download: bool,
    /// 自动下载的附件名匹配：精确、`prefix*`、`*suffix` 或 `*contains*`，不区分大小写。
    pub asset_pattern: Option<String>,
}

/// 收藏仓库的未读与已提醒状态，独立于 `Favorite` 持久化，避免旧数据库迁移。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WatchEntry {
    /// 最近一次被用户在当前主窗口确认过的版本 ID。
    pub acked_release_id: Option<u64>,
    /// 最近一次已弹窗/写托盘提示的版本 ID。
    pub notified_release_id: Option<u64>,
    /// 最近一次自动下载的附件标识（asset id 或附件名），防止重复入队。
    pub last_downloaded_asset: Option<String>,
    pub published_at: Option<u64>,
    pub notes_excerpt: Option<String>,
    /// 连续失败次数，用于指数退避。
    pub failures: u32,
    pub rule: WatchRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WatchState {
    /// 当前 schema；未知更高版本原样保留。
    pub schema: u32,
    /// key 为小写 `owner/repo`。
    pub repositories: std::collections::BTreeMap<String, WatchEntry>,
}

impl Default for WatchState {
    fn default() -> Self {
        Self {
            schema: 1,
            repositories: std::collections::BTreeMap::new(),
        }
    }
}

impl WatchState {
    pub fn entry(&self, repository: &str) -> Option<&WatchEntry> {
        self.repositories.get(&repository.to_ascii_lowercase())
    }

    pub fn entry_mut(&mut self, repository: &str) -> &mut WatchEntry {
        self.repositories
            .entry(repository.to_ascii_lowercase())
            .or_default()
    }

    pub fn is_unread(&self, favorite: &Favorite) -> bool {
        match (&favorite.latest, self.entry(&favorite.repository)) {
            (Some(latest), Some(entry)) => entry.acked_release_id != Some(latest.id),
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

/// 一轮收藏检查的进度；由 Manager 持久化到内存快照并广播。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CheckProgress {
    pub active: bool,
    pub total: u32,
    pub done: u32,
    pub current: Option<String>,
    pub cancelled: bool,
    pub finished_at: Option<u64>,
}

/// GitHub API 配额与降级状态。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ApiStatus {
    pub remaining: Option<u32>,
    pub limit: Option<u32>,
    /// Unix 毫秒；GitHub 的 `X-RateLimit-Reset` 是秒。
    pub reset_at: Option<u64>,
    /// 最近一次成功调用使用的路径：`api` 或 `atom`。
    pub source: Option<String>,
    pub updated_at: Option<u64>,
    pub error: Option<String>,
}

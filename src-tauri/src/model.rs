use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Probing,
    Downloading,
    Retrying,
    Verifying,
    Pausing,
    Cancelling,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
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
    pub last_directory: Option<String>,
    pub error: Option<String>,
    pub revision: u64,
    pub settings: Settings,
    pub queue_revision: u64,
    pub diagnostics: Vec<RouteReport>,
    pub diagnosing: bool,
    pub notices: Vec<Notice>,
    pub favorites: Vec<Favorite>,
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
    pub queue_position: u64,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub limit_kib: u32,
    pub close_to_tray: bool,
    pub auto_check: bool,
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
    pub message: String,
    pub created_at: u64,
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
}

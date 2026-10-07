use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidInput,
    Network,
    Protocol,
    Integrity,
    Io,
    Storage,
    Cancelled,
    Permission,
    DiskSpace,
    NotFound,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct DownloadError {
    pub kind: ErrorKind,
    pub message: String,
    pub retry_after: Option<Duration>,
}

pub type Result<T> = std::result::Result<T, DownloadError>;

impl DownloadError {
    pub fn failure(&self) -> crate::model::Failure {
        let (category, action) = match self.kind {
            ErrorKind::InvalidInput => ("input", "检查链接和输入内容"),
            ErrorKind::Network => ("network", "检查网络后重试；也可暂停后切回自动线路"),
            ErrorKind::Protocol => ("route", "暂停后选择其他线路或切回自动"),
            ErrorKind::Integrity => (
                "integrity",
                "校验失败，切换线路重新下载；不要运行不完整文件",
            ),
            ErrorKind::Permission => ("permission", "另选可写目录重新下载"),
            ErrorKind::DiskSpace => ("space", "释放磁盘空间后重试，或另选目录重新下载"),
            ErrorKind::NotFound => ("not_found", "返回项目版本列表重新选择附件"),
            ErrorKind::Io => ("file", "检查目录、文件占用和磁盘状态，或另选目录"),
            ErrorKind::Storage => ("storage", "检查本地数据目录，保留数据库与备份后重启"),
            ErrorKind::Cancelled => ("cancelled", "可继续已暂停任务"),
        };
        crate::model::Failure {
            category: category.into(),
            message: self.message.clone(),
            action: action.into(),
        }
    }
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "任务已停止")
    }

    pub fn retryable(&self) -> bool {
        self.kind == ErrorKind::Network
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(error: std::io::Error) -> Self {
        let kind = if cfg!(windows) && matches!(error.raw_os_error(), Some(39 | 112)) {
            ErrorKind::DiskSpace
        } else if error.kind() == std::io::ErrorKind::PermissionDenied
            && error.raw_os_error() != Some(32)
        {
            ErrorKind::Permission
        } else {
            ErrorKind::Io
        };
        Self::new(kind, format!("文件操作失败：{error}"))
    }
}

impl From<rusqlite::Error> for DownloadError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new(ErrorKind::Storage, format!("任务数据库操作失败：{error}"))
    }
}

impl From<serde_json::Error> for DownloadError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(ErrorKind::Storage, format!("任务数据格式错误：{error}"))
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(error: reqwest::Error) -> Self {
        let message = if error.is_timeout() {
            "网络请求超时"
        } else if error.is_connect() {
            "无法连接下载服务，请检查网络"
        } else {
            "网络传输中断或服务响应无效"
        };
        // 不保留 reqwest 的完整 URL，防止重定向签名进入错误记录。
        Self::new(ErrorKind::Network, message)
    }
}

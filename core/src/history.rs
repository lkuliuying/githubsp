use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::{Task, TaskStatus},
};
use serde::Serialize;

pub const PAGE_SIZE: usize = 20;
pub const RECENT_TASKS: usize = 50;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub task: Task,
    pub file_state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<HistoryEntry>,
    pub total: usize,
    pub page: u32,
    pub page_size: usize,
    pub revision: u64,
}

pub fn normalize_query(query: &str, page: u32) -> Result<String> {
    if page == 0 || query.len() > 1024 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "历史查询参数无效",
        ));
    }
    Ok(query.trim().to_lowercase())
}

pub(crate) fn search_text(task: &Task) -> String {
    format!(
        "{} {} {}",
        task.filename,
        task.details.repository.as_deref().unwrap_or_default(),
        task.details.tag.as_deref().unwrap_or_default()
    )
    .to_lowercase()
}

pub(crate) fn changed(old: &Task, next: &Task) -> bool {
    old.status != next.status
        || old.filename != next.filename
        || old.details.repository != next.details.repository
        || old.details.tag != next.details.tag
        || old.created_at != next.created_at
        || old.directory != next.directory
        || old.final_path != next.final_path
        || old.details.completed_at != next.details.completed_at
}

pub(crate) fn entry(task: Task) -> HistoryEntry {
    let file_state = if task.status == TaskStatus::Completed {
        match task.final_path.as_ref().map(std::fs::metadata) {
            Some(Ok(metadata)) if metadata.is_file() => "present",
            Some(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => "missing",
            Some(Err(_)) => "inaccessible",
            _ => "missing",
        }
    } else {
        "unfinished"
    };
    HistoryEntry {
        task,
        file_state: file_state.into(),
    }
}

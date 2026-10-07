use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::{Snapshot, Task, TaskStatus},
};
use serde::Serialize;

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

pub fn query(
    snapshot: Snapshot,
    query: &str,
    status: Option<TaskStatus>,
    page: u32,
) -> Result<HistoryPage> {
    if page == 0 || query.len() > 1024 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "历史查询参数无效",
        ));
    }
    let query = query.trim().to_lowercase();
    let mut tasks: Vec<_> = snapshot
        .tasks
        .into_iter()
        .filter(|t| {
            status.is_none_or(|s| s == t.status)
                && format!(
                    "{} {} {}",
                    t.filename,
                    t.details.repository.as_deref().unwrap_or_default(),
                    t.details.tag.as_deref().unwrap_or_default()
                )
                .to_lowercase()
                .contains(&query)
        })
        .collect();
    tasks.sort_by_key(|t| std::cmp::Reverse(t.created_at));
    let total = tasks.len();
    let items = tasks
        .into_iter()
        .skip((page as usize - 1).saturating_mul(20))
        .take(20)
        .map(|task| {
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
        })
        .collect();
    Ok(HistoryPage {
        items,
        total,
        page,
        page_size: 20,
        revision: snapshot.revision,
    })
}

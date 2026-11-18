use crate::{
    history::HistoryPage,
    model::{Favorite, Task, Verification},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// 「我的软件」汇总最多读取的已完成任务数，避免超长历史拖慢首页。
pub const PROJECT_SCAN_LIMIT: usize = 5000;

/// 一个项目在本机的最近一次下载。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSoftware {
    pub repository: String,
    pub task_id: String,
    pub tag: Option<String>,
    pub filename: String,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    pub verification: Verification,
    pub completed_at: u64,
    pub file_state: String,
    pub file_count: usize,
}

/// 按仓库（不区分大小写）分组，取完成时间最新的任务作为本地版本。
pub fn projects(mut tasks: Vec<Task>) -> Vec<LocalSoftware> {
    tasks.sort_by_key(|task| {
        std::cmp::Reverse(task.details.completed_at.unwrap_or(task.created_at))
    });
    let mut projects: Vec<LocalSoftware> = Vec::new();
    let mut index = HashMap::new();
    for task in tasks {
        let Some(repository) = task.details.repository.clone() else {
            continue;
        };
        let key = repository.to_ascii_lowercase();
        if let Some(&position) = index.get(&key) {
            let project: &mut LocalSoftware = &mut projects[position];
            project.file_count += 1;
            continue;
        }
        index.insert(key, projects.len());
        projects.push(LocalSoftware {
            repository,
            completed_at: task.details.completed_at.unwrap_or(task.created_at),
            task_id: task.id,
            tag: task.details.tag,
            filename: task.filename,
            path: task.final_path,
            bytes: task.downloaded,
            verification: task.verification,
            file_state: String::new(),
            file_count: 1,
        });
    }
    // 只访问每个项目最近一次下载的文件，历史很长时也不会逐个读磁盘。
    for project in &mut projects {
        project.file_state = file_state(project.path.as_deref(), project.bytes).into();
    }
    projects
}

fn file_state(path: Option<&Path>, expected: u64) -> &'static str {
    match path.map(std::fs::metadata) {
        Some(Ok(metadata)) if metadata.is_file() && metadata.len() != expected => "changed_size",
        Some(Ok(metadata)) if metadata.is_file() => "present",
        Some(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => "inaccessible",
        _ => "missing",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareFile {
    pub task_id: String,
    pub repository: Option<String>,
    pub tag: Option<String>,
    pub latest_tag: Option<String>,
    pub differs_from_latest: Option<bool>,
    pub filename: String,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub verification_at_download: Verification,
    pub file_state: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwarePage {
    pub items: Vec<SoftwareFile>,
    pub total: usize,
    pub page: u32,
    pub page_size: usize,
}

pub fn page(history: HistoryPage, favorites: &[Favorite]) -> SoftwarePage {
    let items = history
        .items
        .into_iter()
        .map(|entry| {
            let task = entry.task;
            let latest = favorites
                .iter()
                .find(|favorite| {
                    task.details
                        .repository
                        .as_deref()
                        .is_some_and(|repository| {
                            repository.eq_ignore_ascii_case(&favorite.repository)
                        })
                })
                .and_then(|favorite| favorite.latest.as_ref());
            let latest_tag = latest.map(|release| release.tag.clone());
            let differs_from_latest = task
                .details
                .tag
                .as_ref()
                .zip(latest_tag.as_ref())
                .map(|(downloaded, latest)| downloaded != latest);
            let file_state = if entry.file_state == "present"
                && task
                    .final_path
                    .as_ref()
                    .and_then(|path| std::fs::metadata(path).ok())
                    .is_some_and(|metadata| metadata.len() != task.downloaded)
            {
                "changed_size".into()
            } else {
                entry.file_state
            };
            SoftwareFile {
                task_id: task.id,
                repository: task.details.repository,
                tag: task.details.tag,
                latest_tag,
                differs_from_latest,
                filename: task.filename,
                path: task.final_path,
                bytes: task.downloaded,
                sha256: task.details.sha256,
                verification_at_download: task.verification,
                file_state,
            }
        })
        .collect();
    SoftwarePage {
        items,
        total: history.total,
        page: history.page,
        page_size: history.page_size,
    }
}

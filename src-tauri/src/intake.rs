use crate::{
    catalog::Catalog,
    error::{DownloadError, ErrorKind, Result},
    manager::Manager,
    model::{CreateOptions, Snapshot, TaskStatus},
    preflight::{self, Preflight},
    source::parse_release_url,
};
use serde::Serialize;
use std::{collections::HashSet, path::Path};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem {
    pub input: String,
    pub url: Option<String>,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub status: String,
    pub message: Option<String>,
    pub task_id: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPreview {
    pub items: Vec<BatchItem>,
    pub known_size: u64,
    pub unknown_count: usize,
    pub preflight: Preflight,
}
#[derive(Serialize)]
pub struct BatchResult {
    pub items: Vec<BatchItem>,
    pub snapshot: Snapshot,
}

fn validate_count(urls: &[String]) -> Result<()> {
    if urls.is_empty() || urls.len() > 100 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "每批请选择 1 至 100 个附件",
        ));
    }
    Ok(())
}

pub async fn preview(
    manager: &Manager,
    catalog: &Catalog,
    urls: Vec<String>,
    directory: &Path,
) -> Result<BatchPreview> {
    validate_count(&urls)?;
    let directory = preflight::check_async(directory.to_owned(), None, 0)
        .await?
        .directory;
    let snapshot = manager.snapshot().await?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut known_size = 0_u64;
    let mut unknown_count = 0;
    for input in urls {
        let mut item = BatchItem {
            input: input.clone(),
            url: None,
            filename: None,
            size: None,
            status: "invalid".into(),
            message: None,
            task_id: None,
        };
        match catalog.resolve(&input).await {
            Ok((url, asset)) => {
                item.filename = Some(parse_release_url(&url)?.filename);
                item.size = asset.map(|a| a.size);
                let existing = snapshot.tasks.iter().find(|t| {
                    parse_release_url(&t.url).is_ok_and(|s| s.url.as_str() == url)
                        && t.directory == directory
                        && !matches!(t.status, TaskStatus::Completed | TaskStatus::Cancelled)
                });
                if !seen.insert(url.clone()) || existing.is_some() {
                    item.status = "duplicate".into();
                    item.message = Some("重复附件；已有未完成任务请继续或重试".into());
                    item.task_id = existing.map(|t| t.id.clone());
                } else {
                    item.status = "valid".into();
                    if let Some(size) = item.size {
                        known_size = known_size.saturating_add(size);
                    } else {
                        unknown_count += 1;
                    }
                }
                item.url = Some(url);
            }
            Err(error) => item.message = Some(error.message),
        }
        items.push(item);
    }
    let mut preflight = preflight::check_async(
        directory.clone(),
        if unknown_count == 0 {
            Some(known_size)
        } else {
            None
        },
        0,
    )
    .await?;
    if unknown_count > 0 {
        preflight.required = Some(known_size.saturating_mul(2));
        preflight::ensure_space(preflight.available, preflight.required)?;
    }
    Ok(BatchPreview {
        items,
        known_size,
        unknown_count,
        preflight,
    })
}

pub async fn create_batch(
    manager: &Manager,
    catalog: &Catalog,
    urls: Vec<String>,
    directory: &Path,
    route: Option<String>,
) -> Result<BatchResult> {
    validate_count(&urls)?;
    let mut items = Vec::new();
    for input in urls {
        let result = async {
            let (url, asset) = catalog.resolve(&input).await?;
            let options = CreateOptions {
                asset_id: asset.as_ref().map(|a| a.id),
                size: asset.as_ref().map(|a| a.size),
                official_sha256: asset.and_then(|a| a.sha256),
                preferred_route: route.clone(),
            };
            let snapshot = manager
                .create_with(url.clone(), directory.to_owned(), options)
                .await?;
            let task = snapshot
                .tasks
                .iter()
                .rev()
                .find(|t| t.url == url)
                .ok_or_else(|| DownloadError::new(ErrorKind::Storage, "任务创建后未找到记录"))?;
            Ok::<_, DownloadError>(BatchItem {
                input: input.clone(),
                url: Some(url),
                filename: Some(task.filename.clone()),
                size: task.total,
                status: "created".into(),
                message: None,
                task_id: Some(task.id.clone()),
            })
        }
        .await;
        items.push(result.unwrap_or_else(|error| BatchItem {
            input,
            url: None,
            filename: None,
            size: None,
            status: "failed".into(),
            message: Some(error.message),
            task_id: None,
        }));
    }
    Ok(BatchResult {
        items,
        snapshot: manager.snapshot().await?,
    })
}

use crate::{
    catalog::Catalog,
    error::{DownloadError, ErrorKind, Result},
    manager::Manager,
    model::{CreateOptions, Snapshot, TaskStatus},
    preflight::{self, Preflight},
    source::parse_release_url,
};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

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

/// `hints` 是界面已经知道的附件大小（例如版本列表），键为附件下载地址。
pub async fn preview(
    manager: &Manager,
    catalog: &Catalog,
    urls: Vec<String>,
    hints: &HashMap<String, u64>,
    directory: &Path,
) -> Result<BatchPreview> {
    validate_count(&urls)?;
    let directory = preflight::check_async(directory.to_owned(), None, 0)
        .await?
        .directory;
    let snapshot = manager.snapshot().await?;
    let hints: HashMap<String, u64> = hints
        .iter()
        .filter_map(|(url, size)| {
            parse_release_url(url)
                .ok()
                .map(|source| (source.url.to_string(), *size))
        })
        .collect();
    let mut seen = HashSet::new();
    let mut items = Vec::new();
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
        let direct = parse_release_url(&input)
            .ok()
            .map(|source| (source.url.to_string(), source.filename));
        let resolved = match direct {
            Some((url, filename)) => {
                // 直链不再等待 API 补全元数据：文件名可本地解析，大小交给下载探测。
                Ok((url, None, Some(filename)))
            }
            None => catalog
                .resolve(&input)
                .await
                .map(|(url, asset)| (url, asset, None)),
        };
        match resolved {
            Ok((url, asset, direct_filename)) => {
                item.filename = match direct_filename {
                    Some(filename) => Some(filename),
                    None => Some(parse_release_url(&url)?.filename),
                };
                item.size = asset.map(|a| a.size).or_else(|| hints.get(&url).copied());
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
                }
                item.url = Some(url);
            }
            Err(error) => item.message = Some(error.message),
        }
        items.push(item);
    }
    // 仍不知道大小的有效附件并行查询一次，单个最多等待 5 秒；查不到时留到下载时确定。
    let pending: Vec<_> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.status == "valid" && item.size.is_none())
        .filter_map(|(index, item)| {
            parse_release_url(item.url.as_deref()?)
                .ok()
                .map(|source| (index, source))
        })
        .collect();
    let sizes: Vec<_> = stream::iter(pending)
        .map(|(index, source)| async move { (index, catalog.asset_size(&source).await) })
        .buffer_unordered(8)
        .collect()
        .await;
    for (index, size) in sizes {
        match size {
            crate::catalog::AssetSize::Known(size) => items[index].size = Some(size),
            crate::catalog::AssetSize::Missing => {
                items[index].status = "invalid".into();
                items[index].message = Some("GitHub 上找不到这个附件，请检查链接是否正确".into());
            }
            crate::catalog::AssetSize::Unknown => {}
        }
    }
    let mut known_size = 0_u64;
    let mut unknown_count = 0;
    for item in items.iter().filter(|item| item.status == "valid") {
        match item.size {
            Some(size) => known_size = known_size.saturating_add(size),
            None => unknown_count += 1,
        }
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
            let direct = parse_release_url(&input).ok();
            let (url, asset) = match direct {
                Some(source) => {
                    let asset = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        catalog.resolve(&input),
                    )
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .and_then(|(_, asset)| asset);
                    (source.url.to_string(), asset)
                }
                None => catalog.resolve(&input).await?,
            };
            let options = CreateOptions {
                asset_id: asset.as_ref().map(|a| a.id),
                size: asset.as_ref().map(|a| a.size),
                official_sha256: asset.and_then(|a| a.sha256),
                preferred_route: route.clone(),
                ..Default::default()
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

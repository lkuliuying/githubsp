#[cfg(test)]
use crate::selection::matches_pattern;
use crate::{
    catalog::{Asset, Catalog, Release},
    error::{DownloadError, ErrorKind, Result},
    manager::Manager,
    model::{
        now_ms, CheckProgress, Favorite, ReleaseSummary, Settings, Snapshot, WatchEntry, WatchRule,
    },
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// 旧版本默认检查间隔，保留给外部测试与兼容引用。
pub const CHECK_INTERVAL: u64 = Settings::DEFAULT_WATCH_INTERVAL_MS;
const RETRY_BASE_MS: u64 = 60_000;
const RETRY_MAX_MS: u64 = 60 * 60 * 1000;
const CHECK_CONCURRENCY: usize = 3;

#[derive(Clone)]
pub struct Library {
    catalog: Catalog,
    manager: Manager,
    gate: Arc<Mutex<()>>,
    token: CancellationToken,
    cancelled: Arc<AtomicBool>,
}

struct Checked {
    release: Option<Release>,
    favorite: Favorite,
    entry: WatchEntry,
    notify: bool,
    assets: Vec<Asset>,
}

/// 从版本附件里挑选自动下载目标。
struct Selection {
    mode: u8,
    pattern: Option<String>,
    preference: Option<crate::selection::AssetPreference>,
}

impl Selection {
    fn new(settings: &Settings, rule: &WatchRule) -> Self {
        Self {
            mode: settings.watch_asset_mode,
            preference: rule.preference.clone(),
            pattern: rule
                .asset_pattern
                .clone()
                .filter(|pattern| !pattern.trim().is_empty())
                .or_else(|| {
                    (!settings.watch_asset_pattern.trim().is_empty())
                        .then(|| settings.watch_asset_pattern.clone())
                }),
        }
    }

    fn manual(settings: &Settings, rule: &WatchRule) -> Self {
        let mut selection = Self::new(settings, rule);
        // “不自动下载附件”不应阻止用户主动点“下载最新版”。
        if selection.mode == 2 {
            selection.mode = 0;
        }
        selection
    }

    fn select(&self, release: &Release) -> Result<Vec<Asset>> {
        if release.atom || self.mode == 2 {
            return Ok(Vec::new());
        }
        let available = release
            .assets
            .iter()
            .filter(|asset| asset.unavailable.is_none());
        if self.mode == 1 {
            return Ok(available.take(20).cloned().collect());
        }
        let result = crate::selection::recommend(
            release,
            self.preference.as_ref(),
            self.pattern.as_deref(),
        )?;
        Ok(available
            .filter(|asset| result.selected_urls.contains(&asset.url))
            .cloned()
            .collect())
    }
}

impl Library {
    pub fn new(catalog: Catalog, manager: Manager) -> Self {
        Self {
            catalog,
            manager,
            gate: Arc::new(Mutex::new(())),
            token: CancellationToken::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.token.cancel();
    }

    /// 请求停止当前一轮检查；正在进行的单个网络请求结束后生效。
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn start(&self) {
        let library = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(30));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { _ = library.token.cancelled() => break, _ = timer.tick() => () }
                let Ok(snapshot) = library.manager.snapshot().await else {
                    break;
                };
                if !snapshot.settings.auto_check || snapshot.checking.active {
                    continue;
                }
                let now = now_ms();
                let interval = snapshot.settings.watch_interval_ms;
                let due: Vec<String> = snapshot
                    .favorites
                    .iter()
                    .filter(|favorite| {
                        let due = favorite.next_check.map_or_else(
                            || {
                                favorite
                                    .last_checked
                                    .is_none_or(|t| now.saturating_sub(t) >= interval)
                            },
                            |t| t <= now,
                        );
                        due && favorite
                            .last_checked
                            .is_none_or(|t| now.saturating_sub(t) >= 60_000)
                    })
                    .map(|favorite| favorite.repository.clone())
                    .collect();
                if due.is_empty() {
                    continue;
                }
                if let Err(error) = library.run(Some(due), false).await {
                    if error.kind != ErrorKind::Cancelled {
                        eprintln!("收藏检查未完成：{error}");
                    }
                }
            }
        });
    }

    pub async fn add(&self, input: String) -> Result<Snapshot> {
        let input = crate::paste::normalize_input(&input);
        let repository = crate::source::parse_resource(&input)?.repository()?;
        self.manager.add_favorite(input).await?;
        self.run(Some(vec![repository]), true).await
    }

    /// 手动检查全部收藏或指定仓库。
    pub async fn check(&self, repository: Option<String>) -> Result<Snapshot> {
        self.run(repository.map(|repository| vec![repository]), true)
            .await
    }

    /// 标记某个收藏的最新版本为已读；`None` 表示全部已读。
    pub async fn dismiss(&self, repository: Option<String>) -> Result<Snapshot> {
        self.manager.ack_watch(repository).await
    }

    /// 停止本轮检查，并立即把进度标记为已取消。
    pub async fn cancel_checks(&self) -> Result<Snapshot> {
        self.cancel();
        let snapshot = self.manager.snapshot().await?;
        self.manager
            .checking(CheckProgress {
                active: false,
                total: snapshot.checking.total,
                done: snapshot.checking.done,
                current: None,
                cancelled: true,
                finished_at: Some(now_ms()),
            })
            .await
    }

    /// 从收藏卡直接下载最新版本：附件按通配符/安装包模式匹配。
    pub async fn download_latest(&self, repository: String) -> Result<Snapshot> {
        let snapshot = self.manager.snapshot().await?;
        let favorite = snapshot
            .favorites
            .iter()
            .find(|favorite| favorite.repository.eq_ignore_ascii_case(&repository))
            .cloned()
            .ok_or_else(|| DownloadError::new(ErrorKind::NotFound, "收藏项目不存在"))?;
        let entry = snapshot
            .watch_state
            .entry(&favorite.repository)
            .cloned()
            .unwrap_or_default();
        let include_prerelease =
            entry.rule.include_prerelease || snapshot.settings.watch_include_prerelease;
        let page = self
            .catalog
            .browse(
                &format!("https://github.com/{}", favorite.repository),
                0,
                include_prerelease,
            )
            .await?;
        let release = page
            .releases
            .into_iter()
            .find(|release| include_prerelease || !release.prerelease)
            .ok_or_else(|| DownloadError::new(ErrorKind::NotFound, "仓库暂无公开版本"))?;
        if release.atom {
            return Err(DownloadError::new(
                ErrorKind::Network,
                "该版本来自降级更新源，暂时无法直接下载；请打开仓库页面选择附件",
            ));
        }
        let selection = Selection::manual(&snapshot.settings, &entry.rule);
        let assets = selection.select(&release)?;
        if assets.is_empty() {
            return Err(DownloadError::new(
                ErrorKind::NotFound,
                "未找到匹配当前下载规则的附件，请到版本页手动选择",
            ));
        }
        let directory = entry
            .rule
            .download_directory
            .clone()
            .or_else(|| snapshot.last_directory.as_deref().map(PathBuf::from))
            .filter(|path| path.is_dir())
            .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "请先设置下载目录"))?;
        let mut last = None;
        for asset in assets.into_iter().take(20) {
            let options = crate::model::CreateOptions {
                asset_id: Some(asset.id),
                size: Some(asset.size),
                official_sha256: asset.sha256.clone(),
                preferred_route: None,
                ..Default::default()
            };
            last = Some(
                self.manager
                    .create_with(asset.url, directory.clone(), options)
                    .await?,
            );
        }
        self.manager.ack_watch(Some(favorite.repository)).await?;
        if let Some(snapshot) = last {
            Ok(snapshot)
        } else {
            self.manager.snapshot().await
        }
    }

    async fn run(&self, only: Option<Vec<String>>, manual: bool) -> Result<Snapshot> {
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let _guard = tokio::select! { _ = self.token.cancelled() => return Err(DownloadError::cancelled()), guard = self.gate.lock() => guard };
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        self.cancelled.store(false, Ordering::SeqCst);
        let snapshot = self.manager.snapshot().await?;
        let interval = snapshot.settings.watch_interval_ms;
        let settings = snapshot.settings.clone();
        let mut favorites: Vec<_> = snapshot
            .favorites
            .into_iter()
            .filter(|favorite| {
                only.as_ref().is_none_or(|repositories| {
                    repositories
                        .iter()
                        .any(|repository| favorite.repository.eq_ignore_ascii_case(repository))
                })
            })
            .filter(|favorite| {
                if manual {
                    return true;
                }
                let fresh = favorite
                    .last_checked
                    .is_none_or(|t| now_ms().saturating_sub(t) >= 60_000);
                let due = favorite.next_check.is_none_or(|next| next <= now_ms());
                fresh && due
            })
            .collect();
        if favorites.is_empty() {
            self.manager
                .checking(CheckProgress {
                    active: false,
                    finished_at: Some(now_ms()),
                    ..CheckProgress::default()
                })
                .await?;
            return self.manager.snapshot().await;
        }
        let total = favorites.len() as u32;
        self.manager
            .checking(CheckProgress {
                active: true,
                total,
                done: 0,
                current: Some("准备检查".into()),
                cancelled: false,
                finished_at: None,
            })
            .await?;

        let mut done = 0u32;
        let mut cancelled = false;
        let mut first_error = None;
        while !favorites.is_empty() {
            if self.cancelled.load(Ordering::SeqCst) || self.token.is_cancelled() {
                cancelled = true;
                break;
            }
            let chunk: Vec<_> = favorites
                .drain(..favorites.len().min(CHECK_CONCURRENCY))
                .collect();
            let current = chunk
                .first()
                .map(|favorite| favorite.repository.clone())
                .unwrap_or_default();
            self.manager
                .checking(CheckProgress {
                    active: true,
                    total,
                    done,
                    current: Some(current),
                    cancelled: false,
                    finished_at: None,
                })
                .await?;
            let mut results = Vec::new();
            let futures = chunk
                .into_iter()
                .map(|favorite| self.check_one(favorite, interval, &settings));
            for result in futures_util::future::join_all(futures).await {
                if self.cancelled.load(Ordering::SeqCst) {
                    cancelled = true;
                    break;
                }
                done += 1;
                results.push(result);
            }
            for result in results {
                match result {
                    Ok(checked) => {
                        let repository = checked.favorite.repository.clone();
                        let entry = checked.entry.clone();
                        let had_assets = !checked.assets.is_empty();
                        let full = checked.release.clone();
                        let notify = checked.notify;
                        let favorite = checked.favorite.clone();
                        let fresh = self
                            .manager
                            .update_watch_release(favorite.clone(), entry, notify, checked.release)
                            .await?;
                        let mut entry = fresh
                            .watch_state
                            .entry(&repository)
                            .cloned()
                            .unwrap_or_default();
                        let assets = if had_assets
                            && entry.rule.automatic(fresh.settings.watch_auto_download)
                            && !favorite.latest.as_ref().is_some_and(|latest| {
                                entry.skipped_release_ids.contains(&latest.id)
                            }) {
                            full.as_ref()
                                .map(|release| {
                                    Selection::new(&fresh.settings, &entry.rule).select(release)
                                })
                                .transpose()?
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        let key = assets
                            .iter()
                            .map(|asset| asset.id.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        let directory = entry
                            .rule
                            .download_directory
                            .clone()
                            .or_else(|| fresh.last_directory.as_deref().map(PathBuf::from))
                            .filter(|path| path.is_dir());
                        let mut failure = None;
                        if !assets.is_empty() && directory.is_none() {
                            failure = Some("自动下载未入队：请为项目选择有效的下载目录".to_owned());
                        }
                        for asset in assets {
                            if let Some(directory) = directory.clone() {
                                let options = crate::model::CreateOptions {
                                    asset_id: Some(asset.id),
                                    size: Some(asset.size),
                                    official_sha256: asset.sha256.clone(),
                                    preferred_route: None,
                                    ..Default::default()
                                };
                                if let Err(error) = self
                                    .manager
                                    .create_with(asset.url.clone(), directory, options)
                                    .await
                                {
                                    failure = Some(format!(
                                        "自动下载入队失败：{error}；可手动下载最新版重试"
                                    ));
                                }
                            }
                        }
                        if let Some(error) = failure {
                            let mut favorite = favorite;
                            favorite.error = Some(error);
                            self.manager.update_favorite(favorite, false).await?;
                        } else if !key.is_empty() {
                            entry.last_downloaded_asset = Some(key);
                            self.manager.update_watch(favorite, entry, false).await?;
                        }
                    }
                    Err(error) => {
                        if error.kind == ErrorKind::Cancelled {
                            cancelled = true;
                        } else {
                            first_error.get_or_insert(error);
                        }
                    }
                }
            }
            self.manager
                .checking(CheckProgress {
                    active: true,
                    total,
                    done,
                    current: None,
                    cancelled: false,
                    finished_at: None,
                })
                .await?;
        }
        self.manager
            .checking(CheckProgress {
                active: false,
                total,
                done,
                current: None,
                cancelled,
                finished_at: Some(now_ms()),
            })
            .await?;
        self.manager
            .api_status(self.catalog.api_status().await)
            .await?;
        if let Some(error) = first_error {
            eprintln!("部分收藏检查失败：{error}");
        }
        self.manager.snapshot().await
    }

    async fn check_one(
        &self,
        mut favorite: Favorite,
        interval: u64,
        settings: &Settings,
    ) -> Result<Checked> {
        let input = format!("https://github.com/{}", favorite.repository);
        let rule = self
            .manager
            .snapshot()
            .await
            .ok()
            .and_then(|snapshot| snapshot.watch_state.entry(&favorite.repository).cloned())
            .unwrap_or_default();
        let include_prerelease = rule.rule.include_prerelease || settings.watch_include_prerelease;
        let result = tokio::select! {
            _ = self.token.cancelled() => return Err(DownloadError::cancelled()),
            result = self.catalog.browse(&input, 0, include_prerelease) => result
        };
        let mut entry = self
            .manager
            .snapshot()
            .await
            .ok()
            .and_then(|snapshot| snapshot.watch_state.entry(&favorite.repository).cloned())
            .unwrap_or_default();
        favorite.last_checked = Some(now_ms());
        let release = match result {
            Ok(page) => page
                .releases
                .into_iter()
                .find(|release| include_prerelease || !release.prerelease),
            Err(error) if error.kind != ErrorKind::Cancelled => {
                match self.catalog.atom_latest(&favorite.repository).await {
                    Ok(release) => {
                        entry.failures = 0;
                        Some(release)
                    }
                    Err(atom_error) => {
                        entry.failures = entry.failures.saturating_add(1);
                        let backoff = RETRY_BASE_MS
                            .saturating_mul(1u64 << entry.failures.saturating_sub(1).min(6))
                            .min(RETRY_MAX_MS);
                        let retry = error
                            .retry_after
                            .map(|delay| delay.as_millis() as u64)
                            .unwrap_or(backoff)
                            .max(backoff);
                        favorite.next_check = Some(
                            now_ms()
                                .saturating_add(retry)
                                .max(self.catalog.next_allowed().await),
                        );
                        favorite.error = Some(format!(
                            "{}；版本源也不可用：{}",
                            error.message, atom_error.message
                        ));
                        None
                    }
                }
            }
            Err(error) => {
                return Err(error);
            }
        };
        let Some(release) = release else {
            favorite
                .error
                .get_or_insert_with(|| "仓库暂无公开正式版".into());
            entry.failures = entry.failures.saturating_add(1);
            favorite.next_check = Some(now_ms().saturating_add(RETRY_BASE_MS.max(interval)));
            self.manager
                .api_status(self.catalog.api_status().await)
                .await?;
            return Ok(Checked {
                release: None,
                favorite,
                entry,
                notify: false,
                assets: Vec::new(),
            });
        };
        let auto = favorite.latest.is_some()
            && favorite.latest.as_ref().map(|latest| &latest.tag) != Some(&release.tag)
            && !entry.skipped_release_ids.contains(&release.id)
            && entry.rule.automatic(settings.watch_auto_download);
        let mut assets = if auto {
            Selection::new(settings, &entry.rule).select(&release)?
        } else {
            Vec::new()
        };
        let full = release.clone();
        let notify = apply_watch_release(&mut favorite, &mut entry, release, interval);
        self.manager
            .api_status(self.catalog.api_status().await)
            .await?;
        if !assets.is_empty() {
            let key = assets
                .iter()
                .map(|asset| asset.id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            if entry.last_downloaded_asset.as_deref() == Some(key.as_str()) {
                assets.clear();
            }
        }
        Ok(Checked {
            release: Some(full),
            favorite,
            entry,
            notify,
            assets,
        })
    }
}

/// 保留旧签名，供现有测试与调用方使用。
pub fn apply_release(favorite: &mut Favorite, summary: ReleaseSummary) -> bool {
    let mut entry = WatchEntry::default();
    let release = Release {
        id: summary.id,
        tag: summary.tag,
        name: String::new(),
        prerelease: false,
        url: summary.url,
        notes: String::new(),
        assets: Vec::new(),
        published_at: summary.published_at,
        atom: summary.atom,
    };
    apply_watch_release(favorite, &mut entry, release, CHECK_INTERVAL)
}

fn apply_watch_release(
    favorite: &mut Favorite,
    entry: &mut WatchEntry,
    release: Release,
    interval: u64,
) -> bool {
    let summary = ReleaseSummary {
        id: release.id,
        tag: release.tag,
        url: release.url,
        published_at: release.published_at,
        atom: release.atom,
    };
    let previous = favorite.latest.as_ref().map(|latest| latest.id);
    let same_tag = favorite
        .latest
        .as_ref()
        .is_some_and(|latest| latest.tag == summary.tag);
    let notify = previous.is_some() && !same_tag && !favorite.seen.contains(&summary.id);
    if same_tag {
        if entry.acked_release_id == previous {
            entry.acked_release_id = Some(summary.id);
        }
        if entry.notified_release_id == previous {
            entry.notified_release_id = Some(summary.id);
        }
        if previous.is_some_and(|id| entry.skipped_release_ids.contains(&id))
            && !entry.skipped_release_ids.contains(&summary.id)
        {
            entry.skipped_release_ids.push(summary.id);
        }
    }
    if !favorite.seen.contains(&summary.id) {
        favorite.seen.push(summary.id);
    }
    if previous.is_none() {
        // 首次检查只建立基线，不把历史版本标记成未读。
        entry.acked_release_id = Some(summary.id);
        entry.notified_release_id = Some(summary.id);
    }
    entry.published_at = summary.published_at;
    entry.notes_excerpt = (!release.notes.is_empty()).then(|| {
        let notes: String = release.notes.chars().take(280).collect();
        notes
    });
    favorite.latest = Some(summary);
    favorite.last_success = Some(now_ms());
    favorite.next_check = Some(now_ms().saturating_add(interval));
    favorite.error = None;
    entry.failures = 0;
    notify
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WatchState;

    fn asset(id: u64, name: &str, size: u64) -> Asset {
        Asset {
            id,
            name: name.into(),
            size,
            url: format!("https://github.com/o/r/releases/download/v1/{name}"),
            sha256: None,
            hints: Vec::new(),
            unavailable: None,
        }
    }

    fn release(id: u64, assets: Vec<Asset>) -> Release {
        Release {
            id,
            tag: format!("v{id}"),
            name: String::new(),
            prerelease: false,
            url: "https://github.com/o/r/releases/tag/x".into(),
            notes: "notes".into(),
            assets,
            published_at: Some(1),
            atom: false,
        }
    }

    fn favorite() -> Favorite {
        Favorite {
            id: "f".into(),
            repository: "o/r".into(),
            latest: None,
            seen: Vec::new(),
            last_checked: None,
            last_success: None,
            next_check: None,
            error: None,
        }
    }

    #[test]
    fn wildcard_supports_star_and_question_mark() {
        assert!(matches_pattern("*win*x64*.exe", "Tool-Win-x64-Setup.exe"));
        assert!(matches_pattern("tool-?.exe", "tool-a.exe"));
        assert!(!matches_pattern("tool-?.exe", "tool-ab.exe"));
        assert!(matches_pattern("*", "anything.bin"));
        assert!(!matches_pattern("", "anything.bin"));
    }

    #[test]
    fn selection_prefers_one_installer_and_skips_ambiguous_patterns() {
        let settings = Settings::default();
        let mut rule = WatchRule::default();
        let release = release(
            1,
            vec![
                asset(1, "Tool-win-x64.exe", 10),
                asset(2, "Tool-win-x64.zip", 20),
                asset(3, "Tool.dmg", 5),
            ],
        );
        assert_eq!(
            Selection::new(&settings, &rule)
                .select(&release)
                .unwrap()
                .len(),
            1
        );
        rule.asset_pattern = Some("*win*.zip".into());
        assert_eq!(
            Selection::new(&settings, &rule).select(&release).unwrap()[0].name,
            "Tool-win-x64.zip"
        );
        rule.asset_pattern = Some("*tool*".into());
        assert!(Selection::new(&settings, &rule)
            .select(&release)
            .unwrap()
            .is_empty());
        rule.asset_pattern = None;
        let mut off = settings.clone();
        off.watch_asset_mode = 2;
        assert!(Selection::new(&off, &rule)
            .select(&release)
            .unwrap()
            .is_empty());

        let mut versioned = release.clone();
        versioned.id = 2;
        versioned.tag = "v2.0.0".into();
        versioned.assets = vec![asset(4, "Tool-2.0.0.zip", 30)];
        rule.asset_pattern = Some("Tool-{version}.zip".into());
        let selected = Selection::new(&settings, &rule).select(&versioned).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "Tool-2.0.0.zip");
    }

    #[test]
    fn first_check_establishes_baseline_then_new_release_is_unread() {
        let mut favorite = favorite();
        let mut entry = WatchEntry::default();
        assert!(!apply_watch_release(
            &mut favorite,
            &mut entry,
            release(1, Vec::new()),
            CHECK_INTERVAL
        ));
        assert_eq!(entry.acked_release_id, Some(1));
        assert!(apply_watch_release(
            &mut favorite,
            &mut entry,
            release(2, Vec::new()),
            CHECK_INTERVAL
        ));
        assert_eq!(favorite.latest.as_ref().map(|latest| latest.id), Some(2));
        let state = WatchState {
            schema: 1,
            repositories: [("o/r".to_string(), entry.clone())].into(),
        };
        assert!(state.is_unread(&favorite));
        assert!(!apply_watch_release(
            &mut favorite,
            &mut entry,
            release(2, Vec::new()),
            CHECK_INTERVAL
        ));
    }
}

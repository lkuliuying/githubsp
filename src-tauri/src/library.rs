use crate::{
    catalog::Catalog,
    error::{DownloadError, ErrorKind, Result},
    manager::Manager,
    model::{now_ms, Favorite, ReleaseSummary, Snapshot},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub const CHECK_INTERVAL: u64 = 6 * 60 * 60 * 1000;
#[derive(Clone)]
pub struct Library {
    catalog: Catalog,
    manager: Manager,
    gate: Arc<Mutex<()>>,
    token: CancellationToken,
}
impl Library {
    pub fn new(catalog: Catalog, manager: Manager) -> Self {
        Self {
            catalog,
            manager,
            gate: Arc::new(Mutex::new(())),
            token: CancellationToken::new(),
        }
    }
    pub fn stop(&self) {
        self.token.cancel();
    }
    pub fn start(&self) {
        let library = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(60));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { _ = library.token.cancelled() => break, _ = timer.tick() => () }
                let Ok(snapshot) = library.manager.snapshot().await else {
                    break;
                };
                if !snapshot.settings.auto_check {
                    continue;
                }
                for favorite in snapshot.favorites {
                    if library.token.is_cancelled() {
                        return;
                    }
                    if !library
                        .manager
                        .snapshot()
                        .await
                        .is_ok_and(|state| state.settings.auto_check)
                    {
                        break;
                    }
                    if now_ms().saturating_sub(favorite.last_checked.unwrap_or(0)) >= CHECK_INTERVAL
                        && favorite.next_check.is_none_or(|t| t <= now_ms())
                    {
                        if let Err(error) = library.check(Some(favorite.repository)).await {
                            if error.kind != ErrorKind::Cancelled {
                                eprintln!("收藏检查未完成：{error}");
                            }
                        }
                    }
                }
            }
        });
    }
    pub async fn add(&self, input: String) -> Result<Snapshot> {
        let repository = crate::source::parse_resource(&input)?.repository()?;
        self.manager.add_favorite(input).await?;
        self.check(Some(repository)).await
    }
    pub async fn check(&self, repository: Option<String>) -> Result<Snapshot> {
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let _guard = tokio::select! { _ = self.token.cancelled() => return Err(DownloadError::cancelled()), guard = self.gate.lock() => guard };
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let snapshot = self.manager.snapshot().await?;
        let favorites: Vec<_> = snapshot
            .favorites
            .into_iter()
            .filter(|f| {
                repository
                    .as_ref()
                    .is_none_or(|repo| f.repository.eq_ignore_ascii_case(repo))
            })
            .collect();
        for mut favorite in favorites {
            if self.token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            if favorite
                .last_checked
                .is_some_and(|t| now_ms().saturating_sub(t) < 60_000)
                || (favorite.error.is_some() && favorite.next_check.is_some_and(|t| t > now_ms()))
            {
                continue;
            }
            let input = format!("https://github.com/{}", favorite.repository);
            let result = tokio::select! { _ = self.token.cancelled() => return Err(DownloadError::cancelled()), result = self.catalog.browse(&input, 0, false) => result };
            favorite.last_checked = Some(now_ms());
            let mut notify = false;
            match result {
                Ok(page) => {
                    if let Some(release) = page.releases.into_iter().find(|r| !r.prerelease) {
                        notify = apply_release(
                            &mut favorite,
                            ReleaseSummary {
                                id: release.id,
                                tag: release.tag,
                                url: release.url,
                            },
                        );
                    } else {
                        favorite.error = Some("仓库暂无公开正式版".into());
                        favorite.next_check = Some(now_ms() + 60_000);
                    }
                }
                Err(error) => {
                    favorite.next_check = Some(
                        (now_ms()
                            + error
                                .retry_after
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(60_000))
                        .max(self.catalog.next_allowed().await),
                    );
                    favorite.error = Some(error.message);
                }
            }
            self.manager.update_favorite(favorite, notify).await?;
        }
        self.manager.snapshot().await
    }
}

pub fn apply_release(favorite: &mut Favorite, release: ReleaseSummary) -> bool {
    let notify = favorite.latest.is_some() && !favorite.seen.contains(&release.id);
    if !favorite.seen.contains(&release.id) {
        favorite.seen.push(release.id);
    }
    favorite.latest = Some(release);
    favorite.last_success = Some(now_ms());
    favorite.next_check = Some(now_ms() + CHECK_INTERVAL);
    favorite.error = None;
    notify
}

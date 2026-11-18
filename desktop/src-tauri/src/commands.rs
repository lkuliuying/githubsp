use crate::{log, platform, service::Service, system_actions};
use githubsp_lib::{
    history::HistoryPage,
    intake,
    manager::Action,
    model::{Settings, Snapshot, TaskStatus, WatchRule},
    preflight, relocation, updates,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
use tauri::{AppHandle, Manager, State};

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Request {
    Bootstrap,
    Snapshot,
    Software {
        query: String,
        page: u32,
    },
    SoftwareProjects,
    WatchlistExport,
    WatchlistExportFile,
    WatchlistPreview {
        manifest: String,
        directory: PathBuf,
        policy: String,
    },
    WatchlistImport {
        manifest: String,
        directory: PathBuf,
        policy: String,
        fingerprint: String,
    },
    Updates {
        cursor: Option<String>,
        limit: u32,
        repository: Option<String>,
        #[serde(default)]
        recent: bool,
    },
    UpdateDetail {
        id: u64,
    },
    UpdateAction {
        id: u64,
        action: String,
    },
    ProjectLabels {
        repository: String,
        group: String,
        tags: Vec<String>,
    },
    Browse {
        input: String,
        page: u32,
        include_prerelease: bool,
    },
    SelectAssets {
        release: githubsp_lib::catalog::Release,
        preference: Option<githubsp_lib::selection::AssetPreference>,
        pattern: Option<String>,
    },
    Preview {
        urls: Vec<String>,
        #[serde(default)]
        sizes: std::collections::HashMap<String, u64>,
        directory: PathBuf,
    },
    Create {
        urls: Vec<String>,
        directory: PathBuf,
        route: Option<String>,
    },
    Action {
        id: String,
        action: String,
    },
    BatchAction {
        action: String,
    },
    Reorder {
        id: String,
        direction: String,
    },
    Route {
        id: String,
        route: Option<String>,
        restart: bool,
    },
    Suggestion {
        id: String,
        suggestion_id: String,
        apply: bool,
    },
    History {
        query: String,
        status: Option<TaskStatus>,
        page: u32,
        #[serde(default)]
        page_size: Option<u32>,
    },
    FavoriteAdd {
        input: String,
    },
    FavoriteRemove {
        repository: String,
    },
    FavoriteCheck {
        repository: Option<String>,
    },
    FavoriteCancel,
    FavoriteDismiss {
        repository: Option<String>,
    },
    FavoriteDownload {
        repository: String,
    },
    WatchRule {
        repository: String,
        rule: WatchRule,
    },
    SaveSettings {
        settings: Settings,
    },
    Diagnose {
        url: String,
    },
    Acknowledge,
    CheckUpdate,
    UpdateVersions {
        page: u32,
    },
    ChooseDirectory {
        initial: Option<PathBuf>,
    },
    InspectDirectory {
        directory: PathBuf,
    },
    CreateDirectory {
        directory: PathBuf,
    },
    OpenDirectory {
        task_id: Option<String>,
    },
    OpenLink {
        url: String,
    },
    Copy {
        text: String,
    },
    Diagnostics,
    ExportBackup,
    InspectMigration {
        directory: PathBuf,
    },
    Migrate {
        directory: PathBuf,
        keep_backup: bool,
    },
    Close,
    HideNotice,
    ShowMain {
        view: String,
    },
    Exit,
    Restart,
}

fn value(data: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(data).map_err(|e| format!("无法编码界面数据：{e}"))
}
fn action(input: &str) -> Result<Action, String> {
    match input {
        "pause" => Ok(Action::Pause),
        "resume" => Ok(Action::Resume),
        "cancel" => Ok(Action::Cancel),
        "remove" => Ok(Action::Remove),
        _ => Err("无效的任务操作".into()),
    }
}

pub fn reordered(snapshot: &Snapshot, id: &str, direction: &str) -> Result<Vec<String>, String> {
    let mut ids: Vec<_> = snapshot
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Queued)
        .map(|t| t.id.clone())
        .collect();
    let index = ids
        .iter()
        .position(|v| v == id)
        .ok_or("任务已经不在等待队列")?;
    let target = match direction {
        "up" => index.saturating_sub(1),
        "down" => (index + 1).min(ids.len() - 1),
        "top" => 0,
        _ => return Err("无效排序操作".into()),
    };
    let id = ids.remove(index);
    ids.insert(target, id);
    Ok(ids)
}

pub async fn quit(app: AppHandle, service: Arc<Service>) {
    if service.closing.swap(true, Ordering::AcqRel) {
        return;
    }
    // 先取消目录查询与收藏请求，再等待命令排空，避免退出被网络读取阻塞。
    service.catalog.stop();
    service.library.stop();
    let _gate = service.gate.write().await;
    match service.shutdown().await {
        Ok(()) => {
            service.exit_ready.store(true, Ordering::Release);
            app.exit(0);
        }
        Err(error) => {
            use tauri::Emitter;
            log::write("error", &format!("退出保存失败：{error}"));
            let _ = app.emit(
                "workspace-error",
                format!("保存进度失败，请保留数据后重新启动：{error}"),
            );
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
        }
    }
}

async fn migrate(
    app: AppHandle,
    service: Arc<Service>,
    directory: PathBuf,
    keep: bool,
) -> Result<Value, String> {
    let _gate = service.gate.write().await;
    if service.closing.load(Ordering::Acquire) {
        return Err("应用正在退出或迁移".into());
    }
    let locations = service
        .locations
        .clone()
        .ok_or("隔离运行或随身数据模式不能修改正式数据位置")?;
    let source = service.directory.clone();
    let target = relocation::validate_target(&source, &directory).map_err(|e| e.to_string())?;
    let lock = platform::DirectoryLock::acquire(&target)?;
    *service.migration_lock.lock().map_err(|_| "迁移锁异常")? = Some(lock);
    service.shutdown().await?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        locations
            .migrate(&source, &target, keep)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(error) = result {
        return Err(format!("迁移未完成，服务已停止，请重新启动以恢复：{error}"));
    }
    platform::restart()?;
    service.exit_ready.store(true, Ordering::Release);
    app.exit(0);
    Ok(Value::Null)
}

#[tauri::command]
pub async fn dispatch(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    request: Request,
) -> Result<Value, String> {
    let service = state.inner().clone();
    match request {
        Request::Exit => {
            tauri::async_runtime::spawn(quit(app, service));
            return Ok(Value::Null);
        }
        Request::Restart => {
            if !service.closing.load(Ordering::Acquire) {
                return Err("请先保存并停止当前服务".into());
            }
            platform::restart()?;
            service.exit_ready.store(true, Ordering::Release);
            app.exit(0);
            return Ok(Value::Null);
        }
        Request::Migrate {
            directory,
            keep_backup,
        } => return migrate(app, service, directory, keep_backup).await,
        _ => (),
    }
    let _gate = service.gate.read().await;
    if service.closing.load(Ordering::Acquire) {
        return Err("服务已停止，请重新启动 GitHubSP".into());
    }
    let manager = &service.manager;
    let catalog = &service.catalog;
    let result: githubsp_lib::error::Result<Snapshot> = match request {
        Request::Bootstrap => {
            let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
            let location_state = service
                .locations
                .as_ref()
                .map(|l| l.load())
                .transpose()
                .map_err(|e| e.to_string())?;
            let migration_allowed = location_state.as_ref().is_some_and(|s| s.pending.is_none());
            return Ok(
                json!({ "snapshot": snapshot, "directory": service.directory, "version": env!("CARGO_PKG_VERSION"), "migrationAllowed": migration_allowed, "locationState": location_state, "systemReduceMotion": platform::system_reduce_motion() }),
            );
        }
        Request::Snapshot => manager.snapshot().await,
        Request::SelectAssets {
            release,
            preference,
            pattern,
        } => {
            return value(
                githubsp_lib::selection::recommend(
                    &release,
                    preference.as_ref(),
                    pattern.as_deref(),
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Request::Browse {
            input,
            page,
            include_prerelease,
        } => {
            let result = catalog.browse(&input, page, include_prerelease).await;
            let _ = manager.api_status(catalog.api_status().await).await;
            return value(result.map_err(|e| e.to_string())?);
        }
        Request::Preview {
            urls,
            sizes,
            directory,
        } => {
            return value(
                intake::preview(manager, catalog, urls, &sizes, &directory)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::Create {
            urls,
            directory,
            route,
        } => {
            return value(
                intake::create_batch(manager, catalog, urls, &directory, route)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::Action {
            id,
            action: operation,
        } => manager.action(id, action(&operation)?).await,
        Request::BatchAction { action: operation } => {
            let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
            if operation == "clear" {
                manager.remove_completed(snapshot.history_revision).await
            } else {
                let selected = snapshot.tasks.iter().filter(|t| match operation.as_str() {
                    "resume" => t.status.resumable(),
                    "pause" => {
                        t.status.running()
                            || matches!(t.status, TaskStatus::Queued | TaskStatus::WaitingNetwork)
                    }
                    _ => false,
                });
                action(&operation)?;
                for task in selected {
                    manager
                        .action(task.id.clone(), action(&operation)?)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                manager.snapshot().await
            }
        }
        Request::Reorder { id, direction } => {
            let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
            manager
                .reorder(
                    reordered(&snapshot, &id, &direction)?,
                    snapshot.queue_revision,
                )
                .await
        }
        Request::Route { id, route, restart } => manager.route(id, route, restart).await,
        Request::Suggestion {
            id,
            suggestion_id,
            apply,
        } => {
            if apply {
                manager.apply_route_suggestion(id, suggestion_id).await
            } else {
                manager.dismiss_route_suggestion(id, suggestion_id).await
            }
        }
        Request::History {
            query,
            status,
            page,
            page_size,
        } => {
            let result: HistoryPage = manager
                .history_sized(
                    query,
                    status,
                    page,
                    page_size.map_or(githubsp_lib::history::PAGE_SIZE, |size| size as usize),
                )
                .await
                .map_err(|e| e.to_string())?;
            return value(result);
        }
        Request::FavoriteAdd { input } => service.library.add(input).await,
        Request::FavoriteRemove { repository } => manager.remove_favorite(repository).await,
        Request::FavoriteCheck { repository } => service.library.check(repository).await,
        Request::FavoriteCancel => service.library.cancel_checks().await,
        Request::FavoriteDismiss { repository } => service.library.dismiss(repository).await,
        Request::FavoriteDownload { repository } => {
            service.library.download_latest(repository).await
        }
        Request::WatchRule { repository, rule } => manager.watch_rule(repository, rule).await,
        Request::Updates {
            cursor,
            limit,
            repository,
            recent,
        } => {
            return value(
                manager
                    .update_page(cursor, limit, repository, recent)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::UpdateDetail { id } => {
            return value(manager.update_event(id).await.map_err(|e| e.to_string())?)
        }
        Request::Software { query, page } => {
            return value(
                manager
                    .software(query, page)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::SoftwareProjects => {
            return value(
                manager
                    .software_projects()
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::WatchlistExport => {
            return value(
                manager
                    .watchlist_export()
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::WatchlistPreview {
            manifest,
            directory,
            policy,
        } => {
            return value(
                manager
                    .watchlist_preview(manifest, directory, policy)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::WatchlistImport {
            manifest,
            directory,
            policy,
            fingerprint,
        } => {
            manager
                .import_watchlist(manifest, directory, policy, fingerprint)
                .await
        }
        Request::WatchlistExportFile => {
            let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("导出可迁移关注清单")
                .set_file_name("GitHubSP-watchlist.json")
                .add_filter("JSON", &["json"])
                .save_file()
                .await
            else {
                return Ok(Value::Null);
            };
            let manifest = manager
                .watchlist_export()
                .await
                .map_err(|e| e.to_string())?;
            let path = file.path().to_owned();
            let target = path.clone();
            tauri::async_runtime::spawn_blocking(move || {
                githubsp_lib::watchlist::write_file(&manifest, &target)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            return value(path);
        }
        Request::UpdateAction { id, action } => manager.update_action(id, action).await,
        Request::ProjectLabels {
            repository,
            group,
            tags,
        } => manager.project_labels(repository, group, tags).await,
        Request::SaveSettings { settings } => manager.settings(settings).await,
        Request::Diagnose { url } => manager.diagnose(&url).await,
        Request::Acknowledge => manager.acknowledge().await,
        Request::CheckUpdate => {
            return value(
                updates::check(
                    catalog,
                    updates::configured_repository(),
                    env!("CARGO_PKG_VERSION"),
                )
                .await,
            )
        }
        Request::UpdateVersions { page } => {
            return value(
                updates::list_releases(
                    catalog,
                    updates::configured_repository(),
                    env!("CARGO_PKG_VERSION"),
                    page,
                )
                .await
                .map_err(|e| e.to_string())?,
            )
        }
        Request::ChooseDirectory { initial } => {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("选择文件夹");
            if let Some(initial) = initial {
                if initial.is_dir() {
                    dialog = dialog.set_directory(initial);
                }
            }
            return value(
                dialog
                    .pick_folder()
                    .await
                    .map(|file| file.path().to_owned()),
            );
        }
        Request::InspectDirectory { directory } => {
            return value(
                preflight::inspect_directory_async(directory)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::CreateDirectory { directory } => {
            return value(
                preflight::create_directory_async(directory)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::OpenDirectory { task_id } => {
            let path = if let Some(id) = task_id {
                manager.task(id).await.map_err(|e| e.to_string())?.directory
            } else {
                service.directory.clone()
            };
            return tauri::async_runtime::spawn_blocking(move || {
                system_actions::open_directory(&path)
            })
            .await
            .map_err(|e| e.to_string())?
            .map(|_| Value::Null);
        }
        Request::OpenLink { url } => {
            return system_actions::open_link(&url, false).map(|_| Value::Null)
        }
        Request::Copy { text } => {
            if text.len() > 1_048_576 {
                return Err("复制内容过长".into());
            }
            return system_actions::copy(&text, std::ptr::null_mut()).map(|_| Value::Null);
        }
        Request::Diagnostics => {
            let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
            return value(log::diagnostics(&snapshot, &service.directory));
        }
        Request::ExportBackup => {
            let file = rfd::AsyncFileDialog::new()
                .set_title("导出数据备份")
                .set_file_name(format!(
                    "GitHubSP-{}.sqlite3",
                    githubsp_lib::model::now_ms()
                ))
                .add_filter("SQLite", &["sqlite3"])
                .save_file()
                .await;
            let Some(file) = file else {
                return Ok(Value::Null);
            };
            manager
                .export_backup(file.path().to_owned())
                .await
                .map_err(|e| e.to_string())?;
            return value(file.path());
        }
        Request::InspectMigration { directory } => {
            return value(
                relocation::inspect_target(&service.directory, &directory)
                    .map_err(|e| e.to_string())?,
            )
        }
        Request::HideNotice => {
            if let Some(window) = app.get_webview_window("notice") {
                window.hide().map_err(|e| e.to_string())?;
            }
            return Ok(Value::Null);
        }
        Request::ShowMain { view } => {
            if !matches!(view.as_str(), "queue" | "software") {
                return Err("无效的目标页面".into());
            }
            crate::show_main(&app);
            use tauri::Emitter;
            app.emit_to("main", "workspace-navigate", view)
                .map_err(|e| e.to_string())?;
            return Ok(Value::Null);
        }
        Request::Close => {
            let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
            if snapshot.settings.close_to_tray {
                app.get_webview_window("main")
                    .ok_or("主窗口已关闭")?
                    .hide()
                    .map_err(|e| e.to_string())?;
            } else {
                tauri::async_runtime::spawn(quit(app, service.clone()));
            }
            return Ok(Value::Null);
        }
        Request::Exit | Request::Restart | Request::Migrate { .. } => unreachable!(),
    };
    value(result.map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_contract_rejects_unknown_actions_and_preserves_camel_case() {
        assert!(action("execute").is_err());
        let request: Request = serde_json::from_value(json!({"type":"browse","input":"https://github.com/a/b","page":1,"includePrerelease":false})).unwrap();
        assert!(matches!(
            request,
            Request::Browse {
                page: 1,
                include_prerelease: false,
                ..
            }
        ));
        assert!(serde_json::from_value::<Request>(
            json!({"type":"history","query":"","page":1,"status":"invalid"})
        )
        .is_err());
    }
}

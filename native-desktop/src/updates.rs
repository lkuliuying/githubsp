use crate::{
    bridge::{self, Area, Epoch},
    notes,
    presentation::{self, model},
    service::Service,
    system_actions, MainWindow, Updates, Workspace,
};
use githubsp_lib::{
    catalog::{Asset, Release},
    intake,
    updates::{self, UpdateResult},
};
use slint::ComponentHandle;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    result: Option<UpdateResult>,
    releases: Vec<Release>,
    selected: usize,
    page: u32,
    has_more: bool,
}

pub fn attachment(release: &Release) -> Result<&Asset, String> {
    let expected = format!(
        "GitHubSP-v{}-windows-x64.exe",
        release.tag.strip_prefix('v').unwrap_or(&release.tag)
    );
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|a| a.name == expected)
        .collect();
    match matches.as_slice() {
        [] => Err("此版本未提供 Windows x64 便携程序，请选择其他版本或打开官方发布页。".into()),
        [asset] => {
            if let Some(error) = &asset.unavailable {
                Err(format!("此版本的便携程序不可下载：{error}"))
            } else {
                Ok(asset)
            }
        }
        _ => Err("此版本的便携程序附件不唯一，请打开官方发布页确认。".into()),
    }
}

fn render_selection(window: &MainWindow, state: &State) {
    let ui = window.global::<Updates>();
    ui.set_versions(model(
        state
            .releases
            .iter()
            .map(|r| {
                let latest = state
                    .result
                    .as_ref()
                    .and_then(|r| r.latest_release.as_ref())
                    .is_some_and(|latest| latest.id == r.id);
                format!("{}{}", r.tag, if latest { "（最新正式版）" } else { "" }).into()
            })
            .collect(),
    ));
    ui.set_selected(state.selected as i32);
    ui.set_has_more(state.has_more);
    ui.set_dialog_message("".into());
    ui.set_duplicate_task("".into());
    if let Some(release) = state.releases.get(state.selected) {
        ui.set_selected_notes(model(notes::render(&release.notes)));
        match attachment(release) {
            Ok(asset) => {
                ui.set_downloadable(true);
                ui.set_attachment(asset.name.as_str().into());
                ui.set_attachment_detail(
                    format!(
                        "{} · Windows x64 免安装程序",
                        presentation::bytes(asset.size)
                    )
                    .into(),
                );
            }
            Err(error) => {
                ui.set_downloadable(false);
                ui.set_attachment(error.into());
                ui.set_attachment_detail("".into());
            }
        }
    } else {
        ui.set_downloadable(false);
        ui.set_attachment("请选择要下载的版本".into());
        ui.set_selected_notes(model(vec![]));
    }
}

fn show(window: &MainWindow, state: &Arc<Mutex<State>>, epoch: &Epoch) {
    let ui = window.global::<Updates>();
    if ui.get_dialog() || ui.get_stage() != 0 || ui.get_listing() {
        return;
    }
    match state.lock() {
        Ok(mut state) => {
            let Some(release) = state
                .result
                .as_ref()
                .filter(|r| r.status == "available")
                .and_then(|r| r.latest_release.clone())
            else {
                return;
            };
            epoch.invalidate();
            state.releases = vec![release];
            state.selected = 0;
            state.page = 0;
            state.has_more = true;
            render_selection(window, &state);
            ui.set_dialog(true);
        }
        Err(_) => ui.set_message("更新状态无法访问，请重新启动应用。".into()),
    }
}

fn merge_releases(existing: &mut Vec<Release>, next: Vec<Release>) {
    let mut ids = existing
        .iter()
        .map(|r| r.id)
        .collect::<std::collections::HashSet<_>>();
    existing.extend(next.into_iter().filter(|r| ids.insert(r.id)));
}

pub(crate) fn may_show_result(window: &MainWindow, view_revision: i32) -> bool {
    let workspace = window.global::<Workspace>();
    workspace.get_page() == 3
        && window.global::<crate::Preferences>().get_section() == 4
        && workspace.get_view_revision() == view_revision
}

pub fn bind(window: &MainWindow, api: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    let state = Arc::new(Mutex::new(State::default()));
    let epoch = Epoch::default();
    {
        let (weak, state, epoch) = (window.as_weak(), state.clone(), epoch.clone());
        window.global::<Updates>().on_show(move || {
            if let Some(window) = weak.upgrade() {
                show(&window, &state, &epoch);
            }
        });
    }
    {
        let (weak, epoch) = (window.as_weak(), epoch.clone());
        window.global::<Updates>().on_close(move || {
            if let Some(window) = weak.upgrade() {
                let ui = window.global::<Updates>();
                if ui.get_stage() != 2 {
                    epoch.invalidate();
                    ui.set_dialog(false);
                }
            }
        });
    }
    {
        let (weak, api, runtime, state) = (
            window.as_weak(),
            api.clone(),
            runtime.clone(),
            state.clone(),
        );
        window.global::<Updates>().on_check(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if window.global::<Updates>().get_dialog() {
                return;
            }
            let (catalog, state) = (api.catalog.clone(), state.clone());
            let view_revision = window.global::<Workspace>().get_view_revision();
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::UpdateCheck,
                Epoch::default().ticket(),
                async move {
                    Ok(updates::check(
                        &catalog,
                        updates::configured_repository(),
                        env!("CARGO_PKG_VERSION"),
                    )
                    .await)
                },
                move |window, result| {
                    let ui = window.global::<Updates>();
                    ui.set_checked(
                        system_actions::date(Some(githubsp_lib::model::now_ms())).into(),
                    );
                    ui.set_latest(result.latest.as_deref().unwrap_or("").into());
                    ui.set_message(
                        format!(
                            "{}{}",
                            result.message,
                            result
                                .next_check
                                .map(|time| format!(
                                    "\n下次可检查：{}",
                                    system_actions::date(Some(time))
                                ))
                                .unwrap_or_default()
                        )
                        .into(),
                    );
                    ui.set_can_open(result.url.is_some());
                    ui.set_available(
                        result.status == "available" && result.latest_release.is_some(),
                    );
                    ui.set_notes(model(notes::render(result.notes.as_deref().unwrap_or(""))));
                    match state.lock() {
                        Ok(mut state) => state.result = Some(result),
                        Err(_) => {
                            ui.set_message("更新状态无法访问".into());
                            return;
                        }
                    }
                    if may_show_result(window, view_revision) {
                        ui.invoke_show();
                    }
                },
            );
        });
    }
    {
        let (weak, state, api, runtime, epoch) = (
            window.as_weak(),
            state.clone(),
            api.clone(),
            runtime.clone(),
            epoch.clone(),
        );
        window.global::<Updates>().on_load_more(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Updates>();
            if !ui.get_dialog() || !ui.get_has_more() {
                return;
            }
            let next = match state.lock() {
                Ok(state) => state.page + 1,
                Err(_) => {
                    ui.set_dialog_message("更新状态无法访问".into());
                    return;
                }
            };
            let (catalog, state) = (api.catalog.clone(), state.clone());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::UpdateList,
                epoch.ticket(),
                async move {
                    updates::list_releases(
                        &catalog,
                        updates::configured_repository(),
                        env!("CARGO_PKG_VERSION"),
                        next,
                    )
                    .await
                    .map_err(|e| e.to_string())
                },
                move |window, page| match state.lock() {
                    Ok(mut state) => {
                        merge_releases(&mut state.releases, page.releases);
                        state.page = page.page;
                        state.has_more = page.has_more;
                        render_selection(window, &state);
                    }
                    Err(_) => window
                        .global::<Updates>()
                        .set_dialog_message("更新状态无法访问".into()),
                },
            );
        });
    }
    {
        let (weak, state) = (window.as_weak(), state.clone());
        window.global::<Updates>().on_select(move |index| {
            if let Some(window) = weak.upgrade() {
                let ui = window.global::<Updates>();
                if ui.get_stage() != 0 || ui.get_listing() {
                    return;
                }
                match state.lock() {
                    Ok(mut state) => {
                        if index >= 0 && (index as usize) < state.releases.len() {
                            state.selected = index as usize;
                            render_selection(&window, &state);
                        }
                    }
                    Err(_) => ui.set_dialog_message("更新状态无法访问".into()),
                }
            }
        });
    }
    {
        let (weak, state) = (window.as_weak(), state.clone());
        window.global::<Updates>().on_official(move |selected| {
            if let Some(window) = weak.upgrade() {
                let url = match state.lock() {
                    Ok(state) => {
                        if selected {
                            state.releases.get(state.selected).map(|r| r.url.clone())
                        } else {
                            state.result.as_ref().and_then(|r| r.url.clone())
                        }
                    }
                    Err(_) => {
                        window.set_message("更新状态无法访问".into());
                        return;
                    }
                };
                if let Some(url) = url {
                    if let Err(error) = system_actions::open_link(&url, true) {
                        if selected {
                            window.global::<Updates>().set_dialog_message(error.into());
                        } else {
                            window.global::<Updates>().set_message(error.into());
                        }
                    }
                }
            }
        });
    }
    {
        let weak = window.as_weak();
        window.global::<Updates>().on_open_link(move |url| {
            if let Some(window) = weak.upgrade() {
                if let Err(error) = system_actions::open_link(&url, false) {
                    if window.global::<Updates>().get_dialog() {
                        window.global::<Updates>().set_dialog_message(error.into());
                    } else {
                        window.global::<Updates>().set_message(error.into());
                    }
                }
            }
        });
    }
    {
        let (weak, api, runtime, state, epoch) = (
            window.as_weak(),
            api.clone(),
            runtime.clone(),
            state,
            epoch.clone(),
        );
        window.global::<Updates>().on_download(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Updates>();
            if !ui.get_dialog() {
                return;
            }
            let asset = match state.lock() {
                Ok(state) => state
                    .releases
                    .get(state.selected)
                    .ok_or_else(|| "请选择要下载的版本".into())
                    .and_then(attachment)
                    .cloned(),
                Err(_) => Err("更新状态无法访问".into()),
            };
            let asset = match asset {
                Ok(asset) => asset,
                Err(error) => {
                    ui.set_dialog_message(error.into());
                    return;
                }
            };
            let directory = api
                .state
                .borrow()
                .as_ref()
                .and_then(|s| s.last_directory.clone())
                .unwrap_or_default();
            let (api1, ticket, update) = (api.clone(), epoch.ticket(), window.as_weak());
            ui.set_duplicate_task("".into());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::UpdateDownload,
                ticket.clone(),
                async move {
                    let Some(directory) = bridge::choose_directory(&directory).await? else {
                        return Ok(None);
                    };
                    ticket.ensure_current(&api1)?;
                    // 先在界面线程锁定“创建中”，再向任务管理器提交；此后关闭弹窗不能撤销创建。
                    let (send, receive) = tokio::sync::oneshot::channel();
                    let current = ticket.clone();
                    update
                        .upgrade_in_event_loop(move |window| {
                            let ui = window.global::<Updates>();
                            let valid = current.current() && ui.get_dialog();
                            if valid {
                                ui.set_stage(2);
                            }
                            let _ = send.send(valid);
                        })
                        .map_err(|e| e.to_string())?;
                    if !receive.await.map_err(|e| e.to_string())? {
                        return Ok(None);
                    }
                    ticket.ensure_current(&api1)?;
                    let result = intake::create_batch(
                        &api1.manager,
                        &api1.catalog,
                        vec![asset.url.clone()],
                        &directory,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    Ok(Some((result, asset.url, directory)))
                },
                |window, result| {
                    if let Some((result, url, directory)) = result {
                        let ui = window.global::<Updates>();
                        let created = result.items.len() == 1
                            && result.items[0].status == "created"
                            && result.items[0].task_id.is_some()
                            && result.items[0].url.as_deref() == Some(url.as_str());
                        if created {
                            ui.invoke_close();
                            window.global::<Workspace>().invoke_navigate(0);
                            window.global::<Workspace>().set_queue_message(
                                "新版下载任务已加入队列，下载完成后请自行启动新版。".into(),
                            );
                        } else {
                            ui.set_dialog_message(
                                result
                                    .items
                                    .first()
                                    .and_then(|r| r.message.as_deref())
                                    .unwrap_or("未能确认任务创建结果，请先查看下载队列。")
                                    .into(),
                            );
                            if let Some(task) = result.snapshot.tasks.iter().find(|t| {
                                t.url == url && t.directory == directory && t.status.resumable()
                            }) {
                                ui.set_duplicate_task(task.id.as_str().into());
                            }
                        }
                    }
                },
            );
        });
    }
    let weak = window.as_weak();
    window.global::<Updates>().on_resume_duplicate(move || {
        if let Some(window) = weak.upgrade() {
            let ui = window.global::<Updates>();
            let id = ui.get_duplicate_task();
            if !id.is_empty() {
                ui.invoke_close();
                window.global::<Workspace>().invoke_navigate(0);
                window
                    .global::<Workspace>()
                    .invoke_task_action(id, "resume".into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(id: u64) -> Release {
        Release {
            id,
            tag: "v1.2.3".into(),
            name: "".into(),
            prerelease: false,
            url: "https://github.com/test/repo/releases/tag/v1.2.3".into(),
            notes: "".into(),
            assets: vec![],
        }
    }
    fn asset(name: &str) -> Asset {
        Asset {
            id: 1,
            name: name.into(),
            size: 1,
            url: "https://github.com/test/repo/releases/download/v1.2.3/app.exe".into(),
            sha256: None,
            hints: vec![],
            unavailable: None,
        }
    }
    #[test]
    fn attachment_requires_exact_unique_available_portable_executable() {
        let mut release = release(1);
        assert!(attachment(&release).is_err());
        release
            .assets
            .push(asset("GitHubSP-v1.2.3-windows-x64.exe"));
        assert!(attachment(&release).is_ok());
        release.assets.push(release.assets[0].clone());
        assert!(attachment(&release).is_err());
        release.assets.pop();
        release.assets[0].unavailable = Some("不可下载".into());
        assert!(attachment(&release).is_err());
        release.assets[0] = asset("GitHubSP-v1.2.3-windows-arm64.exe");
        assert!(attachment(&release).is_err());
    }
    #[test]
    fn pagination_deduplicates_without_discarding_selected_release() {
        let mut releases = vec![release(3)];
        merge_releases(&mut releases, vec![release(3), release(2)]);
        merge_releases(&mut releases, vec![]);
        merge_releases(&mut releases, vec![release(2), release(1)]);
        assert_eq!(releases.iter().map(|r| r.id).collect::<Vec<_>>(), [3, 2, 1]);
    }
}

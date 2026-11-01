use crate::{
    bridge::{self, Area, Epoch},
    presentation::{model, update_model},
    service::Service,
    system_actions::date,
    FavoriteRow, Favorites, MainWindow, Sources, Workspace,
};
use githubsp_lib::model::{Favorite, Snapshot};
use slint::ComponentHandle;
use std::sync::Arc;

fn visible(snapshot: &Snapshot, filter: i32, sort: i32) -> Vec<&Favorite> {
    let mut rows: Vec<_> = snapshot
        .favorites
        .iter()
        .filter(|f| match filter {
            1 => snapshot.watch_state.is_unread(f),
            2 => f.last_success.is_some() && f.error.is_none(),
            3 => f.error.is_some(),
            4 => f.last_success.is_none() && f.error.is_none(),
            _ => true,
        })
        .collect();
    rows.sort_by(|a, b| {
        if sort == 1 {
            a.repository
                .to_lowercase()
                .cmp(&b.repository.to_lowercase())
        } else {
            b.last_checked.cmp(&a.last_checked)
        }
    });
    rows
}

pub fn apply(window: &MainWindow, snapshot: &Snapshot) {
    let ui = window.global::<Favorites>();
    ui.set_total(snapshot.favorites.len() as i32);
    ui.set_successful(
        snapshot
            .favorites
            .iter()
            .filter(|f| f.last_success.is_some() && f.error.is_none())
            .count() as i32,
    );
    ui.set_failed(
        snapshot
            .favorites
            .iter()
            .filter(|f| f.error.is_some())
            .count() as i32,
    );
    ui.set_pending(ui.get_total() - ui.get_successful() - ui.get_failed());
    ui.set_updates(
        snapshot
            .favorites
            .iter()
            .filter(|f| snapshot.watch_state.is_unread(f))
            .count() as i32,
    );
    ui.set_checking(snapshot.checking.active);
    ui.set_checking_text(
        if snapshot.checking.active {
            if snapshot
                .checking
                .current
                .as_deref()
                .unwrap_or("")
                .is_empty()
            {
                format!(
                    "正在检查 {}/{}",
                    snapshot.checking.done, snapshot.checking.total
                )
            } else {
                format!(
                    "正在检查 {}/{} · {}",
                    snapshot.checking.done,
                    snapshot.checking.total,
                    snapshot.checking.current.as_deref().unwrap_or("")
                )
            }
        } else if snapshot.checking.cancelled {
            "上次检查已取消".to_string()
        } else {
            String::new()
        }
        .into(),
    );
    ui.set_checking_progress(if snapshot.checking.total == 0 {
        0.0
    } else {
        snapshot.checking.done as f32 / snapshot.checking.total as f32
    });
    ui.set_auto_download(snapshot.settings.watch_auto_download);
    ui.set_checked(
        snapshot
            .favorites
            .iter()
            .filter_map(|f| f.last_checked)
            .max()
            .map(|v| date(Some(v)))
            .unwrap_or_else(|| "尚未检查".into())
            .into(),
    );
    let rows = visible(snapshot, ui.get_filter(), ui.get_sort())
        .into_iter()
        .map(|f| {
            let entry = snapshot.watch_state.entry(&f.repository);
            let published = entry
                .and_then(|entry| entry.published_at)
                .or_else(|| f.latest.as_ref().and_then(|latest| latest.published_at));
            let summary = entry
                .and_then(|entry| entry.notes_excerpt.as_deref())
                .map(|notes| {
                    notes
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(140)
                        .collect::<String>()
                })
                .unwrap_or_default();
            FavoriteRow {
                repository: f.repository.as_str().into(),
                tag: f
                    .latest
                    .as_ref()
                    .map(|r| r.tag.as_str())
                    .unwrap_or("未知")
                    .into(),
                state: if f.error.is_some() {
                    "检查失败"
                } else if f.last_success.is_some() {
                    "检查成功"
                } else {
                    "待检查"
                }
                .into(),
                tone: if f.error.is_some() {
                    2
                } else if f.last_success.is_some() {
                    1
                } else {
                    0
                },
                color_index: (f.repository.encode_utf16().map(u32::from).sum::<u32>() % 5) as i32,
                checked: if f.last_checked.is_some() {
                    date(f.last_checked)
                } else {
                    "尚未检查".into()
                }
                .into(),
                unread: snapshot.watch_state.is_unread(f),
                published: date(published).into(),
                summary: summary.into(),
                downloadable: f.latest.is_some()
                    && !f.latest.as_ref().is_some_and(|latest| latest.atom)
                    && f.error.is_none(),
                error: f
                    .error
                    .as_ref()
                    .map(|e| {
                        format!(
                            "{e}\n下次可检查：{} · 上次成功：{}",
                            date(f.next_check),
                            date(f.last_success)
                        )
                    })
                    .unwrap_or_else(|| {
                        if f.last_success.is_none() {
                            "尚未成功检查正式版本".into()
                        } else {
                            String::new()
                        }
                    })
                    .into(),
            }
        })
        .collect();
    ui.set_rows(update_model(ui.get_rows(), rows));
}

pub fn bind(window: &MainWindow, api: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    let epoch = Epoch::default();
    let (weak, api1) = (window.as_weak(), api.clone());
    window.global::<Favorites>().on_filter_changed(move || {
        if let (Some(window), Some(snapshot)) = (weak.upgrade(), api1.state.borrow().clone()) {
            apply(&window, &snapshot);
        }
    });
    let (weak, api1, runtime1, epoch1) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window.global::<Favorites>().on_add(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let input = window.global::<Favorites>().get_input().to_string();
        let (library, original) = (api1.library.clone(), input.clone());
        bridge::run(
            &window,
            &api1,
            &runtime1,
            Area::Favorites,
            epoch1.ticket(),
            async move { library.add(input).await.map_err(|e| e.to_string()) },
            move |window, snapshot| {
                apply(window, &snapshot);
                let ui = window.global::<Favorites>();
                if ui.get_input() == original {
                    ui.set_input("".into());
                }
            },
        );
    });
    let (weak, api2, runtime2, epoch2) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window
        .global::<Favorites>()
        .on_action(move |repository, action| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if action == "copy" {
                let ui = window.global::<Favorites>();
                match crate::log::copy(&format!("https://github.com/{repository}")) {
                    Ok(()) => ui.set_message("仓库链接已复制".into()),
                    Err(error) => ui.set_message(error.into()),
                }
                return;
            }
            if action == "browse" {
                window.global::<Workspace>().invoke_navigate(0);
                window
                    .global::<Workspace>()
                    .invoke_select_download_section(1);
                window.global::<Workspace>().invoke_select_creation_mode(1);
                let ui = window.global::<Sources>();
                ui.set_repository(format!("https://github.com/{repository}").into());
                ui.invoke_edited();
                ui.invoke_browse(0);
                return;
            }
            let (repository, action, service) =
                (repository.to_string(), action.to_string(), api2.clone());
            bridge::run(
                &window,
                &api2,
                &runtime2,
                Area::Favorites,
                epoch2.ticket(),
                async move {
                    match action.as_str() {
                        "check" => {
                            service
                                .library
                                .check(if repository.is_empty() {
                                    None
                                } else {
                                    Some(repository)
                                })
                                .await
                        }
                        "remove" => service.manager.remove_favorite(repository).await,
                        "dismiss" => service.library.dismiss(Some(repository)).await,
                        _ => return Err("无效收藏操作".into()),
                    }
                    .map_err(|e| e.to_string())
                },
                |window, snapshot| apply(window, &snapshot),
            );
        });
    let (weak, api3, runtime3, epoch3) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window.global::<Favorites>().on_download(move |repository| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let (repository, service) = (repository.to_string(), api3.clone());
        bridge::run(
            &window,
            &api3,
            &runtime3,
            Area::Favorites,
            epoch3.ticket(),
            async move {
                service
                    .library
                    .download_latest(repository)
                    .await
                    .map_err(|e| e.to_string())
            },
            |window, snapshot| apply(window, &snapshot),
        );
    });
    let (weak, api4, runtime4, epoch4) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window.global::<Favorites>().on_cancel_check(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let service = api4.clone();
        bridge::run(
            &window,
            &api4,
            &runtime4,
            Area::Favorites,
            epoch4.ticket(),
            async move {
                service
                    .library
                    .cancel_checks()
                    .await
                    .map_err(|e| e.to_string())
            },
            |window, snapshot| apply(window, &snapshot),
        );
    });
    let (weak, api5) = (window.as_weak(), api.clone());
    window
        .global::<Favorites>()
        .on_edit_rule(move |repository| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Favorites>();
            let snapshot = api5.state.borrow().clone();
            let rule = snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.watch_state.entry(&repository))
                .map(|entry| entry.rule.clone())
                .unwrap_or_default();
            ui.set_rule_repository(repository);
            ui.set_rule_prerelease(rule.include_prerelease);
            ui.set_rule_auto(rule.auto_download);
            ui.set_rule_pattern(rule.asset_pattern.unwrap_or_default().into());
        });
    let (weak, api6, runtime6, epoch6) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window.global::<Favorites>().on_save_rule(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let ui = window.global::<Favorites>();
        let repository = ui.get_rule_repository().to_string();
        if repository.is_empty() {
            return;
        }
        let rule = githubsp_lib::model::WatchRule {
            include_prerelease: ui.get_rule_prerelease(),
            auto_download: ui.get_rule_auto(),
            asset_pattern: {
                let pattern = ui.get_rule_pattern().trim().to_string();
                (!pattern.is_empty()).then_some(pattern)
            },
        };
        let service = api6.clone();
        bridge::run(
            &window,
            &api6,
            &runtime6,
            Area::Favorites,
            epoch6.ticket(),
            async move {
                service
                    .manager
                    .watch_rule(repository, rule)
                    .await
                    .map_err(|e| e.to_string())
            },
            |window, snapshot| {
                apply(window, &snapshot);
                let ui = window.global::<Favorites>();
                ui.set_rule_repository("".into());
                ui.set_message("关注规则已保存".into());
            },
        );
    });
    let weak = window.as_weak();
    window.global::<Favorites>().on_cancel_rule(move || {
        if let Some(window) = weak.upgrade() {
            window.global::<Favorites>().set_rule_repository("".into());
        }
    });
    window.global::<Favorites>().set_rows(model(vec![]));
}

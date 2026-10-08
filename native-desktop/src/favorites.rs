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

fn visible(favorites: &[Favorite], filter: i32, sort: i32) -> Vec<&Favorite> {
    let mut rows: Vec<_> = favorites
        .iter()
        .filter(|f| match filter {
            1 => f.last_success.is_some() && f.error.is_none(),
            2 => f.error.is_some(),
            3 => f.last_success.is_none() && f.error.is_none(),
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
    let rows = visible(&snapshot.favorites, ui.get_filter(), ui.get_sort())
        .into_iter()
        .map(|f| FavoriteRow {
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
    let (weak, api, runtime) = (window.as_weak(), api.clone(), runtime.clone());
    window
        .global::<Favorites>()
        .on_action(move |repository, action| {
            let Some(window) = weak.upgrade() else {
                return;
            };
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
                (repository.to_string(), action.to_string(), api.clone());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Favorites,
                epoch.ticket(),
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
                        _ => return Err("无效收藏操作".into()),
                    }
                    .map_err(|e| e.to_string())
                },
                |window, snapshot| apply(window, &snapshot),
            );
        });
    window.global::<Favorites>().set_rows(model(vec![]));
}

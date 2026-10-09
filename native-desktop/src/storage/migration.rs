use crate::{
    bridge::{self, Area, Epoch},
    preferences,
    service::Service,
    MainWindow, Storage, Updates,
};
use githubsp_lib::{preflight::DirectoryState, relocation};
use slint::ComponentHandle;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub(super) fn bind(window: &MainWindow, service: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    if let Some(locations) = &service.locations {
        match locations.load() {
            Ok(state) => {
                window
                    .global::<Storage>()
                    .set_allowed(state.pending.is_none());
                window
                    .global::<Storage>()
                    .set_message(state.message.unwrap_or_default().into());
            }
            Err(error) => window
                .global::<Storage>()
                .set_message(error.to_string().into()),
        }
    }
    let epoch = Epoch::default();
    let selected = Arc::new(AtomicBool::new(false));
    let prepared = Arc::new(Mutex::new(None::<PathBuf>));
    let (editing, picked) = (epoch.clone(), selected.clone());
    window.global::<Storage>().on_edit_target(move || {
        editing.invalidate();
        picked.store(false, Ordering::Release);
    });
    let (weak, api, handle, picking, picked) = (
        window.as_weak(),
        service.clone(),
        runtime.clone(),
        epoch.clone(),
        selected.clone(),
    );
    window.global::<Storage>().on_choose_target(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let ui = window.global::<Storage>();
        if !ui.get_allowed() || ui.get_dialog() || window.global::<Updates>().get_dialog() {
            return;
        }
        let start = if ui.get_target().is_empty() {
            ui.get_directory().to_string()
        } else {
            ui.get_target().to_string()
        };
        let picked = picked.clone();
        bridge::run(
            &window,
            &api,
            &handle,
            Area::Storage,
            picking.ticket(),
            async move { bridge::choose_directory(&start).await },
            move |window, directory| {
                if let Some(directory) = directory {
                    picked.store(true, Ordering::Release);
                    window
                        .global::<Storage>()
                        .set_target(directory.to_string_lossy().as_ref().into());
                }
            },
        );
    });
    let (weak, api, handle, preparing, picked, pending) = (
        window.as_weak(),
        service.clone(),
        runtime.clone(),
        epoch.clone(),
        selected,
        prepared.clone(),
    );
    window.global::<Storage>().on_request_migration(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let ui = window.global::<Storage>();
        if !ui.get_allowed() || ui.get_dialog() || window.global::<Updates>().get_dialog() {
            return;
        }
        if preferences::has_unsaved(&window, &api) {
            ui.set_message("有尚未保存的设置，请先点击“保存全部设置”，再迁移数据目录。".into());
            return;
        }
        let input = ui.get_target().to_string();
        let selected = picked.load(Ordering::Acquire);
        let source = api.data_directory.clone();
        let ticket = preparing.ticket();
        let check = ticket.clone();
        let api1 = api.clone();
        let pending = pending.clone();
        bridge::run(
            &window,
            &api,
            &handle,
            Area::Storage,
            ticket,
            async move {
                let inspect_source = source.clone();
                let target = PathBuf::from(&input);
                let inspected = tokio::task::spawn_blocking(move || {
                    relocation::inspect_target(&inspect_source, &target)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
                check.ensure_current(&api1)?;
                if selected && inspected.state != DirectoryState::Existing {
                    return Err("所选目录已不存在，请重新选择目录".into());
                }
                let Some(target) =
                    bridge::prepare_directory(input, selected, &check, &api1).await?
                else {
                    return Ok(None);
                };
                let target = tokio::task::spawn_blocking(move || {
                    relocation::validate_target(&source, &target)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
                check.ensure_current(&api1)?;
                Ok(Some(target))
            },
            move |window, target| {
                if let Some(target) = target {
                    let ui = window.global::<Storage>();
                    match pending.lock() {
                        Ok(mut pending) => {
                            ui.set_target(target.to_string_lossy().as_ref().into());
                            *pending = Some(target);
                            ui.set_dialog(true);
                        }
                        Err(_) => ui.set_message("迁移状态异常，请重新启动程序。".into()),
                    }
                }
            },
        );
    });
    let (weak, pending, cancelling) = (window.as_weak(), prepared.clone(), epoch);
    window.global::<Storage>().on_cancel_migration(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if window.global::<Storage>().get_migrating() {
            return;
        }
        cancelling.invalidate();
        if let Ok(mut pending) = pending.lock() {
            *pending = None;
        }
        window.global::<Storage>().set_dialog(false);
    });
    let (weak, api, handle) = (window.as_weak(), service.clone(), runtime.clone());
    window
        .global::<Storage>()
        .on_confirm_migration(move |keep| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Storage>();
            if !ui.get_dialog() || !ui.get_allowed() || ui.get_migrating() || api.is_closing() {
                return;
            }
            if preferences::has_unsaved(&window, &api) {
                ui.set_dialog(false);
                ui.set_message("设置已变化，请先保存设置再迁移。".into());
                return;
            }
            let target = match prepared.lock() {
                Ok(mut prepared) => prepared.take(),
                Err(_) => None,
            };
            let Some(target) = target else {
                ui.set_dialog(false);
                ui.set_message("迁移目标已失效，请重新操作。".into());
                return;
            };
            if api
                .closing
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            ui.set_migrating(true);
            ui.set_dialog(false);
            ui.set_busy(true);
            ui.set_message("正在保存下载进度、校验并迁移数据，请勿关闭程序…".into());
            window.set_busy(true);
            let (api, weak) = (api.clone(), weak.clone());
            handle.spawn(async move {
                let result = crate::data_location::relocate(&api, target, keep).await;
                let _ = weak.upgrade_in_event_loop(move |window| {
                    crate::data_location::request_restart(&window, &api, result)
                });
            });
        });
}

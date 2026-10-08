use crate::{
    bridge::{self, Area, Epoch},
    presentation,
    service::Service,
    sources::Inputs,
    History, MainWindow, Preferences, Sources, Updates, Workspace,
};
use githubsp_lib::{
    manager::Action,
    model::{Snapshot, TaskStatus},
};
use slint::ComponentHandle;
use std::sync::{atomic::Ordering, Arc};

pub use presentation::apply;

fn mark_navigation(window: &MainWindow) {
    let ui = window.global::<Workspace>();
    ui.set_view_revision(ui.get_view_revision().wrapping_add(1));
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
        .position(|task| task == id)
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

pub fn bind(window: &MainWindow, service: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    let inputs = Inputs::default();
    let history_epoch = Epoch::default();
    crate::sources::bind(window, service, runtime, &inputs);
    crate::history::bind(window, service, runtime, &history_epoch);
    crate::favorites::bind(window, service, runtime);
    crate::preferences::bind(window, service, runtime);
    crate::storage::bind(window, service, runtime);
    crate::updates::bind(window, service, runtime);
    {
        let (weak, inputs, history_epoch) =
            (window.as_weak(), inputs.clone(), history_epoch.clone());
        window.global::<Workspace>().on_navigate(move |page| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if !(0..=3).contains(&page) || window.global::<Updates>().get_stage() == 2 {
                return;
            }
            let ui = window.global::<Workspace>();
            if ui.get_page() == page && (page != 0 || ui.get_download_section() == 0) {
                return;
            }
            inputs.epoch.invalidate();
            history_epoch.invalidate();
            window.global::<Updates>().invoke_close();
            mark_navigation(&window);
            ui.set_page(page);
            if page == 0 {
                ui.set_download_section(0);
            }
            if page == 1 {
                window.global::<History>().invoke_load(1);
            }
        });
    }
    {
        let (weak, inputs) = (window.as_weak(), inputs.clone());
        window
            .global::<Workspace>()
            .on_select_download_section(move |section| {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let ui = window.global::<Workspace>();
                if !(0..=2).contains(&section)
                    || ui.get_download_section() == section
                    || window.global::<Updates>().get_dialog()
                {
                    return;
                }
                // 离开表单后，迟到的目录选择和附件请求不能继续创建或覆盖当前内容。
                inputs.epoch.invalidate();
                mark_navigation(&window);
                ui.set_download_section(section);
            });
    }
    {
        let (weak, inputs) = (window.as_weak(), inputs.clone());
        window
            .global::<Workspace>()
            .on_select_creation_mode(move |mode| {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let ui = window.global::<Workspace>();
                if !(0..=2).contains(&mode)
                    || ui.get_creation_mode() == mode
                    || window.global::<Updates>().get_dialog()
                {
                    return;
                }
                inputs.epoch.invalidate();
                mark_navigation(&window);
                window.global::<Sources>().invoke_dismiss_preview();
                ui.set_creation_mode(mode);
            });
    }
    {
        let weak = window.as_weak();
        window
            .global::<Workspace>()
            .on_select_settings_section(move |section| {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let ui = window.global::<Preferences>();
                if !(0..=4).contains(&section)
                    || ui.get_section() == section
                    || window.global::<Updates>().get_dialog()
                {
                    return;
                }
                mark_navigation(&window);
                ui.set_section(section);
            });
    }
    {
        let (weak, inputs) = (window.as_weak(), inputs.clone());
        window
            .global::<Workspace>()
            .on_input_edited(move |directory_changed| {
                inputs.epoch.invalidate();
                if directory_changed {
                    inputs.selected_directory.store(false, Ordering::Release);
                }
                if let Some(window) = weak.upgrade() {
                    window.global::<Sources>().invoke_edited();
                }
            });
    }
    {
        let (weak, api, runtime, inputs) = (
            window.as_weak(),
            service.clone(),
            runtime.clone(),
            inputs.clone(),
        );
        window.global::<Workspace>().on_create(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Workspace>();
            let url = ui.get_url().trim().to_owned();
            let directory = ui.get_directory().trim().to_owned();
            if !matches!(
                githubsp_lib::source::parse_resource(&url),
                Ok(githubsp_lib::source::Resource::Asset(_))
            ) {
                ui.invoke_select_creation_mode(1);
                let source = window.global::<Sources>();
                source.set_repository(url.into());
                source.invoke_edited();
                source.invoke_browse(0);
                return;
            }
            let (api1, ticket, original) = (api.clone(), inputs.epoch.ticket(), url.clone());
            let selected = inputs.selected_directory.load(Ordering::Acquire);
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Commands,
                ticket.clone(),
                async move {
                    let Some(directory) =
                        bridge::prepare_directory(directory, selected, &ticket, &api1).await?
                    else {
                        return Ok(false);
                    };
                    ticket.ensure_current(&api1)?;
                    api1.manager
                        .create(url, directory)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(true)
                },
                move |window, created| {
                    let ui = window.global::<Workspace>();
                    if created && ui.get_url() == original {
                        ui.set_url("".into());
                    }
                    if created {
                        ui.invoke_select_download_section(0);
                    }
                },
            );
        });
    }
    {
        let (weak, api, runtime, inputs) = (
            window.as_weak(),
            service.clone(),
            runtime.clone(),
            inputs.clone(),
        );
        window.global::<Workspace>().on_pick_directory(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let start = window.get_directory().to_string();
            let ticket = inputs.epoch.ticket();
            let inputs = inputs.clone();
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Commands,
                ticket,
                async move { bridge::choose_directory(&start).await },
                move |window, directory| {
                    if let Some(directory) = directory {
                        inputs.epoch.invalidate();
                        inputs.selected_directory.store(true, Ordering::Release);
                        window.set_directory(directory.to_string_lossy().as_ref().into());
                        window.global::<Sources>().invoke_edited();
                    }
                },
            );
        });
    }
    {
        let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
        window.global::<Workspace>().on_task_action(move |id, action| {
            let Some(window) = weak.upgrade() else { return; };
            if window.get_busy() { return; }
            let cached = api.state.borrow().clone();
            if action == "copy" {
                let (manager,id) = (api.manager.clone(),id.to_string());
                bridge::run(&window,&api,&runtime,Area::Commands,Epoch::default().ticket(),async move {
                    manager.task(id).await.map_err(|error|error.to_string())
                }, |window,task| {
                    let result = crate::system_actions::copy(&task.url, crate::desktop::hwnd(window.window()).unwrap_or(std::ptr::null_mut()));
                    match result {
                        Ok(()) => {
                            window.global::<History>().set_message("已复制来源链接".into());
                            window.global::<Workspace>().set_queue_message("已复制来源链接".into());
                        }
                        Err(error) => window.set_message(error.into()),
                    }
                });
                return;
            }
            let order = if matches!(action.as_str(), "up" | "down" | "top") {
                match cached.as_ref().ok_or_else(||"任务管理器尚未就绪".to_owned()).and_then(|s|reordered(s,&id,&action).map(|ids|(ids,s.queue_revision))) {
                    Ok(value) => Some(value), Err(error) => { window.set_message(error.into()); return; }
                }
            } else { None };
            let (id, action, api1) = (id.to_string(), action.to_string(), api.clone());
            bridge::run(&window, &api, &runtime, Area::Commands, Epoch::default().ticket(), async move {
                let task = api1.manager.task(id.clone()).await.map_err(|e|e.to_string())?;
                let mut refresh_history = false;
                match action.as_str() {
                    "open" => {
                        let directory = task.directory.clone();
                        tokio::task::spawn_blocking(move ||crate::system_actions::open_directory(&directory)).await.map_err(|e|e.to_string())??;
                    }
                    "pause" | "resume" | "cancel" | "remove" => {
                        if action == "cancel" && !bridge::confirmed("取消这个下载？", format!("{}\n将停止下载并删除该任务的临时分片。已完成的文件不受影响。\n如果之后还要继续，请选择“暂停下载”。",task.filename)).await { return Ok(false); }
                        if api1.is_closing() { return Ok(false); }
                        let action = match action.as_str() { "pause" => Action::Pause, "resume" => Action::Resume, "remove" => Action::Remove, _ => Action::Cancel };
                        api1.manager.action(id, action).await.map_err(|e|e.to_string())?;
                        refresh_history = true;
                    }
                    "redownload" => {
                        let Some(directory) = bridge::choose_directory(&task.directory.to_string_lossy()).await? else { return Ok(false); };
                        if api1.is_closing() { return Ok(false); }
                        api1.manager.create(task.url.clone(), directory).await.map_err(|e|e.to_string())?;
                    }
                    "favorite" => {
                        let repository = task.details.repository.as_ref().ok_or("任务未包含仓库信息")?;
                        api1.library.add(format!("https://github.com/{repository}")).await.map_err(|e|e.to_string())?;
                    }
                    "up" | "down" | "top" => {
                        let (ids, revision) = order.ok_or("等待队列已变化，请重试")?;
                        api1.manager.reorder(ids, revision).await.map_err(|e|e.to_string())?;
                    }
                    action if action.starts_with("suggest:") || action.starts_with("dismiss:") => {
                        let (kind, suggestion_id) = action.split_once(':').ok_or("换线建议无效")?;
                        let suggestion = task.details.route_suggestion.as_ref().filter(|s|s.id == suggestion_id).ok_or("下载状态或建议已变化，本次建议已失效。")?;
                        if kind == "dismiss" { api1.manager.dismiss_route_suggestion(id, suggestion.id.clone()).await.map_err(|e|e.to_string())?; }
                        else if bridge::confirmed("确认换线并重新下载？", format!("将先复核 {} 的可用性和收益，再停止当前写入。\n确认切换后会丢弃 {} 的已有分片并从头下载，任务仍保持自动选线。", suggestion.route_name, presentation::bytes(suggestion.restart_bytes))).await && !api1.is_closing() {
                            api1.manager.apply_route_suggestion(id, suggestion.id.clone()).await.map_err(|e|e.to_string())?;
                        }
                    }
                    _ => return Err("无效任务操作".into()),
                }
                Ok(refresh_history)
            }, |window, refresh| { if refresh && window.global::<Workspace>().get_page() == 1 { let history = window.global::<History>(); history.invoke_load(history.get_page()); } });
        });
    }
    {
        let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
        window.global::<Workspace>().on_task_route(move |id, index| {
            let Some(window) = weak.upgrade() else { return; };
            let Some((route,_)) = presentation::ROUTES.get(index as usize) else { window.set_message("无效下载线路".into()); return; };
            let route = if route.is_empty() {None} else {Some((*route).to_owned())};
            let (api1, id) = (api.clone(), id.to_string());
            bridge::run(&window, &api, &runtime, Area::Commands, Epoch::default().ticket(), async move {
                let task = api1.manager.task(id.clone()).await.map_err(|e|e.to_string())?;
                let restart = task.downloaded > 0;
                if restart && !bridge::confirmed("更改下载线路？", "需要换线时会丢弃原线路分片并从头下载。自动模式会先尝试安全复用原线路进度。").await { return Ok(()); }
                if !api1.is_closing() { api1.manager.route(id,route,restart).await.map_err(|e|e.to_string())?; }
                Ok(())
            }, |_,()|{});
        });
    }
    {
        let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
        window.global::<Workspace>().on_batch(move |action| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let (action, api1) = (action.to_string(), api.clone());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Commands,
                Epoch::default().ticket(),
                async move {
                    if action == "remove" {
                        let snapshot = api1.manager.snapshot().await.map_err(|e| e.to_string())?;
                        if snapshot.completed_tasks == 0 { return Ok(()); }
                        if bridge::confirmed(
                            "清空已完成的记录？",
                            format!("将移除全部 {} 条已完成记录，包括历史页中的记录。下载文件会保留在原目录。", snapshot.completed_tasks),
                        ).await && !api1.is_closing() {
                            api1.manager.remove_completed(snapshot.history_revision).await.map_err(|e|e.to_string())?;
                        }
                        return Ok(());
                    }
                    let eligible = |status: TaskStatus| match action.as_str() {
                        "pause" => presentation::pausable(status),
                        "resume" => status.resumable(),
                        _ => false,
                    };
                    let snapshot = api1.manager.snapshot().await.map_err(|e| e.to_string())?;
                    for task in snapshot.tasks.iter().filter(|t| eligible(t.status)) {
                        if api1.is_closing() {
                            break;
                        }
                        let current = api1.manager.snapshot().await.map_err(|e| e.to_string())?;
                        if !current
                            .tasks
                            .iter()
                            .any(|t| t.id == task.id && eligible(t.status))
                        {
                            continue;
                        }
                        let action = match action.as_str() {
                            "pause" => Action::Pause,
                            "resume" => Action::Resume,
                            _ => return Err("无效批量操作".into()),
                        };
                        api1.manager
                            .action(task.id.clone(), action)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                },
                |_, ()| {},
            );
        });
    }
    {
        let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
        window.global::<Workspace>().on_diagnose(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let (url, manager) = (
                window
                    .global::<Workspace>()
                    .get_diagnostic_input()
                    .to_string(),
                api.manager.clone(),
            );
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Diagnostics,
                Epoch::default().ticket(),
                async move { manager.diagnose(&url).await.map_err(|e| e.to_string()) },
                |_, _| {},
            );
        });
    }
    {
        let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
        window.global::<Workspace>().on_acknowledge(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let manager = api.manager.clone();
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Commands,
                Epoch::default().ticket(),
                async move { manager.acknowledge().await.map_err(|e| e.to_string()) },
                |_, _| {},
            );
        });
    }
    let weak = window.as_weak();
    window.global::<Workspace>().on_about(move || {
        if let Some(window) = weak.upgrade() {
            window.invoke_about();
        }
    });
}

pub fn bind_shutdown(
    window: &MainWindow,
    service: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
) {
    let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
    window.on_exit(move || {
        if api.closing.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(window) = weak.upgrade() {
            window.set_busy(true);
            window.set_message("正在保存下载进度，请稍候…".into());
        }
        let (api, weak) = (api.clone(), weak.clone());
        runtime.spawn(async move {
            let result = api.shutdown().await;
            let _ = weak.upgrade_in_event_loop(move |window| match result {
                Ok(()) => {
                    if let Err(error) = slint::quit_event_loop() {
                        eprintln!("退出事件循环失败：{error}");
                    }
                }
                Err(error) => {
                    api.closing.store(false, Ordering::Release);
                    window.set_busy(false);
                    window.set_message(format!("保存失败，程序尚未退出：{error}").into());
                    window.invoke_show_main();
                }
            });
        });
    });
}

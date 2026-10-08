use crate::{
    bridge::{self, Area, Epoch},
    presentation::{self, model},
    service::Service,
    system_actions::date,
    History, HistoryRow, MainWindow,
};
use slint::ComponentHandle;
use std::sync::Arc;

fn load(
    window: &MainWindow,
    api: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
    epoch: &Epoch,
    page: i32,
) {
    let ui = window.global::<History>();
    if page < 1 || page > ui.get_pages().max(1) {
        ui.set_message(format!("请输入 1 至 {} 之间的页码", ui.get_pages()).into());
        return;
    }
    if ui.get_busy() {
        epoch.invalidate();
        return;
    }
    epoch.invalidate();
    let query = ui.get_query().to_string();
    let status = ui
        .get_filter()
        .checked_sub(1)
        .and_then(|i| presentation::STATUSES.get(i as usize))
        .copied();
    let manager = api.manager.clone();
    let latest = api.clone();
    if let Some(snapshot) = api.state.borrow().as_ref() {
        ui.set_revision(snapshot.history_revision.to_string().into());
    }
    bridge::run(
        window,
        api,
        runtime,
        Area::History,
        epoch.ticket(),
        async move {
            manager
                .history(query, status, page as u32)
                .await
                .map_err(|e| e.to_string())
        },
        move |window, result| {
            let ui = window.global::<History>();
            if latest
                .state
                .borrow()
                .as_ref()
                .is_some_and(|state| state.history_revision != result.revision)
            {
                ui.invoke_load(result.page as i32);
                return;
            }
            ui.set_total(result.total as i32);
            ui.set_revision(result.revision.to_string().into());
            ui.set_page(result.page as i32);
            ui.set_pages(result.total.div_ceil(result.page_size).max(1) as i32);
            ui.set_jump(result.page.to_string().into());
            ui.set_completed(
                result
                    .items
                    .iter()
                    .filter(|r| r.task.status == githubsp_lib::model::TaskStatus::Completed)
                    .count() as i32,
            );
            ui.set_failed(
                result
                    .items
                    .iter()
                    .filter(|r| r.task.status == githubsp_lib::model::TaskStatus::Failed)
                    .count() as i32,
            );
            ui.set_missing(
                result
                    .items
                    .iter()
                    .filter(|r| matches!(r.file_state.as_str(), "missing" | "inaccessible"))
                    .count() as i32,
            );
            ui.set_rows(model(
                result
                    .items
                    .into_iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        let task = entry.task;
                        let (file_state, tone) = match entry.file_state.as_str() {
                            "present" => ("正常", 1),
                            "missing" => ("文件缺失", 2),
                            "inaccessible" => ("路径不可访问", 3),
                            _ => ("尚未完成", 0),
                        };
                        HistoryRow {
                            id: task.id.into(),
                            number: ((result.page as usize - 1) * result.page_size + index + 1)
                                .to_string()
                                .into(),
                            name: task.filename.into(),
                            state: presentation::status(task.status).into(),
                            repository: task
                                .details
                                .repository
                                .unwrap_or_else(|| "未知仓库".into())
                                .into(),
                            tag: task.details.tag.unwrap_or_else(|| "未知版本".into()).into(),
                            date: date(Some(task.created_at)).into(),
                            completed: date(task.details.completed_at).into(),
                            path: task
                                .final_path
                                .unwrap_or(task.directory)
                                .to_string_lossy()
                                .as_ref()
                                .into(),
                            file_state: file_state.into(),
                            tone,
                            removable: matches!(
                                task.status,
                                githubsp_lib::model::TaskStatus::Completed
                                    | githubsp_lib::model::TaskStatus::Cancelled
                            ),
                        }
                    })
                    .collect(),
            ));
        },
    );
}

pub fn bind(
    window: &MainWindow,
    api: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
    epoch: &Epoch,
) {
    let (weak, api, runtime, epoch) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        epoch.clone(),
    );
    window.global::<History>().on_load(move |page| {
        if let Some(window) = weak.upgrade() {
            load(&window, &api, &runtime, &epoch, page);
        }
    });
    let weak = window.as_weak();
    window.global::<History>().on_reset(move || {
        if let Some(window) = weak.upgrade() {
            let ui = window.global::<History>();
            ui.set_query("".into());
            ui.set_filter(0);
            ui.invoke_load(1);
        }
    });
    let weak = window.as_weak();
    window.global::<History>().on_jump_to(move || {
        if let Some(window) = weak.upgrade() {
            let ui = window.global::<History>();
            match ui.get_jump().trim().parse::<i32>() {
                Ok(page) => ui.invoke_load(page),
                Err(_) => {
                    ui.set_message(format!("请输入 1 至 {} 之间的页码", ui.get_pages()).into())
                }
            }
        }
    });
}

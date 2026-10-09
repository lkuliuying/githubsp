use crate::{platform, service::Service, MainWindow, Storage};
use githubsp_lib::relocation::{Locations, Phase};
use slint::ComponentHandle;
use std::sync::{atomic::Ordering, Arc};

#[cfg(test)]
mod tests;

pub fn recover_interrupted(locations: &Locations) -> Result<(), String> {
    let state = locations.load().map_err(|e| e.to_string())?;
    if state
        .pending
        .as_ref()
        .is_some_and(|p| matches!(p.phase, Phase::Prepared | Phase::Verified))
    {
        locations
            .rollback("上次迁移在切换前中断")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn rollback_failed_start(locations: &Locations, reason: &str) -> Result<bool, String> {
    let state = locations.load().map_err(|e| e.to_string())?;
    let Some(pending) = state.pending else {
        return Ok(false);
    };
    if pending.phase != Phase::Switched {
        return Ok(false);
    }
    let _target = if pending.target.try_exists().map_err(|e| e.to_string())? {
        Some(platform::DirectoryLock::acquire(&pending.target)?)
    } else {
        None
    };
    let _source = platform::DirectoryLock::acquire(&pending.source)?;
    let current = locations.load().map_err(|e| e.to_string())?;
    if !current.pending.as_ref().is_some_and(|p| {
        p.phase == Phase::Switched && p.source == pending.source && p.target == pending.target
    }) {
        return Err("迁移记录在取得目录锁前已变化，未回退数据位置".into());
    }
    locations.rollback(reason).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn finish_startup(
    window: &MainWindow,
    service: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
) {
    let Some(locations) = service.locations.clone() else {
        return;
    };
    let state = match locations.load() {
        Ok(state) => state,
        Err(error) => {
            window
                .global::<Storage>()
                .set_message(error.to_string().into());
            window.global::<Storage>().set_allowed(false);
            return;
        }
    };
    let Some(pending) = state.pending else {
        return;
    };
    window.global::<Storage>().set_allowed(false);
    window.global::<Storage>().set_busy(true);
    window
        .global::<Storage>()
        .set_message("新数据目录已启动，正在处理原目录…".into());
    let weak = window.as_weak();
    let api = service.clone();
    runtime.spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            finish_with_source_lock(&locations, &pending.source)
        })
        .await
        .map_err(|e| format!("迁移收尾任务异常：{e}"))
        .and_then(|v| v);
        if api.is_closing() {
            return;
        }
        let _ = weak.upgrade_in_event_loop(move |window| {
            let ui = window.global::<Storage>();
            ui.set_busy(false);
            ui.set_allowed(result.is_ok());
            ui.set_message(match result {
                Ok(message) | Err(message) => message.into(),
            });
        });
    });
}

fn finish_with_source_lock(
    locations: &Locations,
    source: &std::path::Path,
) -> Result<String, String> {
    let finish = || {
        let _source = if source.try_exists().map_err(|e| e.to_string())? {
            Some(platform::DirectoryLock::acquire(source)?)
        } else {
            None
        };
        locations.finish().map_err(|e| e.to_string())
    };
    finish().map_err(|error| {
        format!(
            "新数据目录已启用，原目录处理未完成：{}\n{error}\n请保留原目录；下次启动会重新检查。",
            source.display()
        )
    })
}

pub async fn relocate(
    service: &Arc<Service>,
    target: std::path::PathBuf,
    keep: bool,
) -> Result<(), String> {
    let source = service.data_directory.clone();
    // 先保留目标的进程锁，再停止当前服务；锁一直持有到旧进程退出。
    let lock = platform::DirectoryLock::acquire(&target);
    if let Err(error) = service.shutdown().await {
        return Err(format!("保存下载进度失败，未开始迁移：{error}"));
    }
    let locations = service
        .locations
        .clone()
        .ok_or("隔离运行不修改正式数据位置")?;
    *service
        .migration_lock
        .lock()
        .map_err(|_| "迁移锁状态异常")? = Some(lock?);
    tokio::task::spawn_blocking(move || {
        locations
            .migrate(&source, &target, keep)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("迁移任务异常：{e}"))?
}

pub fn request_restart(window: &MainWindow, service: &Service, result: Result<(), String>) {
    if let Err(error) = result {
        if let Ok(mut message) = service.restart_message.lock() {
            *message = Some(format!(
                "数据目录修改未完成：{error}\n原数据保留。程序将重新启动并检查迁移状态。"
            ));
        }
    }
    service.restart.store(true, Ordering::Release);
    window
        .global::<Storage>()
        .set_message("正在重新启动…".into());
    if let Err(error) = slint::quit_event_loop() {
        window.global::<Storage>().set_message(
            format!("无法结束当前窗口：{error}。请退出后重新打开程序。原数据已保留。").into(),
        );
    }
}

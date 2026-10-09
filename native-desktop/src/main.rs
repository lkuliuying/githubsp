#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();
mod about;
mod bridge;
mod completion;
mod controller;
mod data_location;
mod desktop;
mod favorites;
mod graphics;
mod history;
mod notes;
mod platform;
mod preferences;
mod presentation;
mod service;
mod sources;
mod storage;
mod system_actions;
#[cfg(test)]
mod ui_tests;
mod updates;

use slint::ComponentHandle;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("GitHubSP 启动失败：{error}");
        rfd::MessageDialog::new()
            .set_title("GitHubSP 启动失败")
            .set_description(format!("{error}\n原有任务数据未被删除。"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let startup = platform::startup()?;
    let locations = startup.locations.clone();
    let result = run_with_startup(startup);
    if let (Err(error), Some(locations)) = (&result, locations) {
        // 所有初始化出口统一恢复尚未投入使用的迁移；此时运行期资源已经释放。
        match data_location::rollback_failed_start(&locations, &error.to_string()) {
            Ok(true) => return Err(format!("{error}\n已恢复原数据位置，请重新打开程序。").into()),
            Err(recovery) => {
                return Err(
                    format!("{error}\n数据位置恢复未完成：{recovery}。原数据已保留。").into(),
                )
            }
            Ok(false) => (),
        }
    }
    result
}

fn run_with_startup(startup: platform::Startup) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(locations) = &startup.locations {
        if startup
            .state
            .pending
            .as_ref()
            .is_some_and(|p| p.phase == githubsp_lib::relocation::Phase::Switched)
        {
            let _target = if startup.directory.try_exists()? {
                Some(platform::DirectoryLock::acquire(&startup.directory)?)
            } else {
                None
            };
            locations.verify_before_start()?;
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let graphics = runtime.block_on(graphics::shared_device())?;
    let completion_creation = completion::CreationContext::default();
    let creation_hook = completion_creation.clone();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(graphics)
        .with_winit_window_attributes_hook(move |attributes| creation_hook.attributes(attributes))
        .select()?;
    let window = MainWindow::new()?;
    let directory = if startup.state.active.is_some() {
        githubsp_lib::relocation::existing_database(&startup.directory)?
    } else {
        std::fs::create_dir_all(&startup.directory)?;
        startup.directory
    };
    let id = platform::instance_id(&directory)?;
    let weak = window.as_weak();
    let Some(_instance) = platform::Instance::acquire(&id, move || {
        let _ = weak.upgrade_in_event_loop(|window| window.invoke_show_main());
    })?
    else {
        return Ok(());
    };
    let _data_guard = githubsp_lib::relocation::DirectoryGuard::acquire(&directory)?;
    if let Some(locations) = &startup.locations {
        let selected = locations.selected(&locations.load()?);
        if std::fs::canonicalize(&selected)? != std::fs::canonicalize(&directory)? {
            return Err("数据位置在启动期间发生变化，请重新打开程序。".into());
        }
        data_location::recover_interrupted(locations)?;
    }
    let mut service = runtime.block_on(service::Service::start(&directory))?;
    Arc::get_mut(&mut service)
        .ok_or("服务已被提前共享")?
        .locations = startup.locations;
    let snapshot = runtime.block_on(service.manager.snapshot())?;
    if let Some(directory) = &snapshot.last_directory {
        window.set_directory(directory.as_str().into());
    }
    controller::bind(&window, &service, runtime.handle());
    controller::apply(&window, &snapshot);
    controller::bind_shutdown(&window, &service, runtime.handle());
    let _tray = desktop::install(&window, &service, runtime.handle())?;
    let notice = completion::Host::new(&window, completion_creation);
    notice.update(&snapshot);
    let notice_events = notice.clone();
    let (weak, api) = (window.as_weak(), service.clone());
    window.on_refresh_state(move || {
        let snapshot = api.state.borrow().clone();
        if let (Some(window), Some(snapshot)) = (weak.upgrade(), snapshot) {
            if api.visible.load(Ordering::Acquire) {
                controller::apply(&window, &snapshot);
            }
            notice_events.update(&snapshot);
        }
    });
    about::install(&window);
    let weak = window.as_weak();
    let api = service.clone();
    let pump = runtime.spawn(async move {
        let pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut receiver = api.state.clone();
        let mut previous: Option<Arc<githubsp_lib::model::Snapshot>> = None;
        let mut last = tokio::time::Instant::now() - Duration::from_millis(250);
        while receiver.changed().await.is_ok() {
            let mut next = receiver.borrow_and_update().clone();
            loop {
                let critical = next.as_ref().is_some_and(|next| service::urgent(previous.as_deref(), next));
                if critical || last.elapsed() >= Duration::from_millis(250) { break; }
                tokio::select! {
                    result = receiver.changed() => { if result.is_err() { return; } next = receiver.borrow_and_update().clone(); }
                    _ = tokio::time::sleep_until(last + Duration::from_millis(250)) => break,
                }
            }
            let critical = next.as_ref().is_some_and(|next| service::urgent(previous.as_deref(), next));
            if (api.visible.load(Ordering::Acquire) || critical) && !pending.swap(true, Ordering::AcqRel) {
                let pending = pending.clone();
                // 队列最多保留一次刷新；执行时读取最新快照，避免主线程阻塞后的事件堆积。
                if weak.upgrade_in_event_loop(move |window| {
                    pending.store(false, Ordering::Release);
                    window.invoke_refresh_state();
                }).is_err() { break; }
            }
            previous = next;
            last = tokio::time::Instant::now();
        }
    });
    // 收起最后一个窗口时仍需处理下载和托盘事件，只在保存成功后退出。
    let shown = window.show();
    if shown.is_ok() {
        if let Some(locations) = &service.locations {
            if let Err(error) = locations.mark_started() {
                runtime.block_on(service.shutdown())?;
                return Err(error.into());
            }
        }
    }
    let result = shown.and_then(|()| {
        {
            let _context = runtime.enter();
            service.library.start();
        }
        data_location::finish_startup(&window, &service, runtime.handle());
        slint::run_event_loop_until_quit()
    });
    notice.shutdown();
    pump.abort();
    if !service.is_closing() {
        runtime.block_on(service.shutdown())?;
    }
    runtime.shutdown_timeout(Duration::from_secs(10));
    result?;
    if service.restart.load(Ordering::Acquire) {
        if let Some(message) = service
            .restart_message
            .lock()
            .map_err(|_| "重启状态异常")?
            .take()
        {
            rfd::MessageDialog::new()
                .set_title("数据目录迁移结果")
                .set_description(message)
                .show();
        }
        if let Err(error) = platform::restart() {
            if let Some(locations) = &service.locations {
                let state = locations.load()?;
                if state
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.phase != githubsp_lib::relocation::Phase::Cleanup)
                {
                    locations.rollback(&error)?;
                }
            }
            return Err(error.into());
        }
    }
    Ok(())
}

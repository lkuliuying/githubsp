#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();
mod about;
mod bridge;
mod completion;
mod controller;
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let graphics = runtime.block_on(graphics::shared_device())?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(graphics)
        .with_winit_window_attributes_hook(|attributes| {
            use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
            if attributes.title == "GitHubSP · 下载完成" {
                attributes.with_active(false).with_skip_taskbar(true)
            } else {
                attributes
            }
        })
        .select()?;
    let window = MainWindow::new()?;
    let directory = platform::data_directory()?;
    std::fs::create_dir_all(&directory)?;
    let id = platform::instance_id(&directory)?;
    let weak = window.as_weak();
    let Some(_instance) = platform::Instance::acquire(&id, move || {
        let _ = weak.upgrade_in_event_loop(|window| window.invoke_show_main());
    })?
    else {
        return Ok(());
    };
    let service = runtime.block_on(service::Service::start(&directory))?;
    let snapshot = runtime.block_on(service.manager.snapshot())?;
    if let Some(directory) = &snapshot.last_directory {
        window.set_directory(directory.as_str().into());
    }
    controller::bind(&window, &service, runtime.handle());
    controller::apply(&window, &snapshot);
    controller::bind_shutdown(&window, &service, runtime.handle());
    let _tray = desktop::install(&window, &service, runtime.handle())?;
    let notice = completion::Host::new(&window);
    notice.update(&snapshot);
    let (weak, api) = (window.as_weak(), service.clone());
    window.on_refresh_state(move || {
        let snapshot = api.state.borrow().clone();
        if let (Some(window), Some(snapshot)) = (weak.upgrade(), snapshot) {
            if api.visible.load(Ordering::Acquire) {
                controller::apply(&window, &snapshot);
            }
            notice.update(&snapshot);
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
    let result = window
        .show()
        .and_then(|()| slint::run_event_loop_until_quit());
    pump.abort();
    if !service.is_closing() {
        runtime.block_on(service.shutdown())?;
    }
    runtime.shutdown_timeout(Duration::from_secs(10));
    result?;
    Ok(())
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod agent;
mod commands;
mod log;
mod platform;
mod service;
mod system_actions;

use service::Service;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tauri::{Emitter, Manager};

fn main() {
    if let Err(error) = run() {
        eprintln!("GitHubSP 启动失败：{error}");
        rfd::MessageDialog::new()
            .set_title("GitHubSP 启动失败")
            .set_description(format!(
                "{error}\n\n请保留原有任务数据。免安装版需要可用的 WebView2 Runtime。"
            ))
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 离线便携包可把固定版本 WebView2 放在程序旁；普通包使用系统运行时。
    let executable = std::env::current_exe()?;
    let parent = executable.parent().ok_or("程序路径无父目录")?;
    let runtime = parent.join("webview2");
    if runtime.join("msedgewebview2.exe").is_file() {
        std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime);
    }
    let startup = if std::env::args_os().len() == 1 && parent.join("portable-mode.flag").is_file() {
        platform::resolve_directory(Some(parent.join("data")), || {
            Err("随身数据模式不读取正式位置".into())
        })?
    } else {
        platform::startup()?
    };
    let locations = startup.locations.clone();
    let result = run_with_startup(startup);
    if let (Err(error), Some(locations)) = (&result, locations) {
        if let Some(pending) = locations
            .load()?
            .pending
            .filter(|p| p.phase == githubsp_lib::relocation::Phase::Switched)
        {
            let _target = if pending.target.exists() {
                Some(platform::DirectoryLock::acquire(&pending.target)?)
            } else {
                None
            };
            let _source = platform::DirectoryLock::acquire(&pending.source)?;
            if !locations.load()?.pending.as_ref().is_some_and(|current| {
                current.phase == pending.phase
                    && current.source == pending.source
                    && current.target == pending.target
            }) {
                return Err("迁移记录在取得目录锁前发生变化，未回退数据位置。".into());
            }
            locations.rollback(&format!("新界面启动失败：{error}"))?;
            return Err(format!("{error}\n已恢复原数据位置，请重新启动。目标副本仍保留。").into());
        }
    }
    result
}

fn run_with_startup(startup: platform::Startup) -> Result<(), Box<dyn std::error::Error>> {
    let locations = startup.locations.clone();
    if let Some(locations) = &locations {
        if startup
            .state
            .pending
            .as_ref()
            .is_some_and(|p| p.phase == githubsp_lib::relocation::Phase::Switched)
        {
            let _target = if startup.directory.exists() {
                Some(platform::DirectoryLock::acquire(&startup.directory)?)
            } else {
                None
            };
            locations.verify_before_start()?;
        }
    }
    let directory = if startup.state.active.is_some() {
        githubsp_lib::relocation::existing_database(&startup.directory)?
    } else {
        std::fs::create_dir_all(&startup.directory)?;
        startup.directory
    };
    let activation = Arc::new(Mutex::new(None::<tauri::AppHandle>));
    let target = activation.clone();
    let Some(_instance) =
        platform::Instance::acquire(&platform::instance_id(&directory)?, move || {
            if let Ok(app) = target.lock() {
                if let Some(app) = app.as_ref() {
                    show_main(app);
                }
            }
        })?
    else {
        return Ok(());
    };
    let _data_guard = githubsp_lib::relocation::DirectoryGuard::acquire(&directory)?;
    if let Some(locations) = &locations {
        let state = locations.load()?;
        if std::fs::canonicalize(locations.selected(&state))? != std::fs::canonicalize(&directory)?
        {
            return Err("数据位置在启动期间变化，请重新启动。".into());
        }
        if state.pending.as_ref().is_some_and(|p| {
            matches!(
                p.phase,
                githubsp_lib::relocation::Phase::Prepared
                    | githubsp_lib::relocation::Phase::Verified
            )
        }) {
            locations.rollback("上次迁移在切换前中断")?;
        }
    }
    log::init(&directory);
    let service = tauri::async_runtime::block_on(Service::start(directory, locations.clone()))?;
    let api = service.clone();
    let app = tauri::Builder::default()
        .manage(service.clone())
        .invoke_handler(tauri::generate_handler![commands::dispatch])
        .setup(move |app| {
            let handle = app.handle().clone();
            *activation.lock().map_err(|_| "窗口激活状态异常")? = Some(handle.clone());
            install_tray(app.handle())?;
            show_main(&handle);
            // 窗口和服务就绪后才把迁移标记为已投入使用。
            if let Some(locations) = &locations {
                locations.mark_started()?;
                if let Some(pending) = locations.load()?.pending {
                    let _source = if pending.source.exists() {
                        Some(platform::DirectoryLock::acquire(&pending.source)?)
                    } else {
                        None
                    };
                    if let Err(error) = locations.finish() {
                        log::write("error", &format!("数据迁移收尾失败：{error}"));
                    }
                }
            }
            let start = api.clone();
            tauri::async_runtime::spawn(async move {
                start.library.start();
            });
            stream_state(handle, api.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "notice" {
                    let _ = window.hide();
                    return;
                }
                let state = window.state::<Arc<Service>>();
                if state
                    .state
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| s.settings.close_to_tray)
                {
                    let _ = window.hide();
                } else {
                    tauri::async_runtime::spawn(commands::quit(
                        window.app_handle().clone(),
                        state.inner().clone(),
                    ));
                }
            }
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(error) => {
            let _ = tauri::async_runtime::block_on(service.shutdown());
            return Err(error.into());
        }
    };
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !service.exit_ready.load(Ordering::Acquire) {
                api.prevent_exit();
                tauri::async_runtime::spawn(commands::quit(app.clone(), service.clone()));
            }
        }
    });
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        // 系统可能拒绝抢占焦点，网页收不到 focus 事件；单独通知界面窗口已被唤起。
        let _ = window.emit("window-activated", ());
    }
}

fn install_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    };
    let show = MenuItem::with_id(app, "show", "打开 GitHubSP", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "保存并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .tooltip("GitHubSP · 下载工作台")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                let service = app.state::<Arc<Service>>().inner().clone();
                tauri::async_runtime::spawn(commands::quit(app.clone(), service));
            }
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn stream_state(app: tauri::AppHandle, service: Arc<Service>) {
    tauri::async_runtime::spawn(async move {
        let mut receiver = service.state.clone();
        let mut previous: Option<Arc<githubsp_lib::model::Snapshot>> = None;
        let mut timer = tokio::time::interval(std::time::Duration::from_millis(250));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while !service.closing.load(Ordering::Acquire) {
            if receiver.changed().await.is_err() {
                break;
            }
            timer.tick().await;
            let Some(snapshot) = receiver.borrow_and_update().clone() else {
                continue;
            };
            log::set_enabled(snapshot.settings.log_enabled);
            if let Some(previous) = &previous {
                log::transitions(previous, &snapshot);
                let background = app.get_webview_window("main").is_some_and(|w| {
                    !w.is_visible().unwrap_or(false) || !w.is_focused().unwrap_or(false)
                });
                let new_notice = snapshot.notices.iter().any(|notice| {
                    !previous.notices.iter().any(|old| old.id == notice.id)
                        && match notice.kind {
                            githubsp_lib::model::NoticeKind::DownloadCompleted => {
                                background && snapshot.settings.background_completion_notice
                            }
                            githubsp_lib::model::NoticeKind::FavoriteUpdated => {
                                snapshot.settings.watch_notifications
                            }
                            githubsp_lib::model::NoticeKind::DownloadFailed => false,
                        }
                });
                if new_notice {
                    show_notice(&app);
                }
            }
            let unread = snapshot
                .favorites
                .iter()
                .filter(|f| snapshot.watch_state.is_unread(f))
                .count();
            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_tooltip(Some(format!(
                    "GitHubSP · {} 个任务 · {unread} 项更新",
                    snapshot.total_tasks
                )));
            }
            if let Err(error) = app.emit("workspace-state", snapshot.as_ref()) {
                log::write("error", &format!("界面状态发送失败：{error}"));
            }
            previous = Some(snapshot);
        }
    });
}

fn show_notice(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("notice") {
        if let Err(error) = window.show() {
            log::write("error", &format!("提醒窗口显示失败：{error}"));
        }
        return;
    }
    let result = tauri::WebviewWindowBuilder::new(
        app,
        "notice",
        tauri::WebviewUrl::App("index.html?notice".into()),
    )
    .title("GitHubSP 更新与下载提醒")
    .inner_size(380.0, 290.0)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .build();
    match result {
        Ok(window) => {
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let scale = monitor.scale_factor();
                let position = monitor.position();
                let size = monitor.size();
                let x = position.x + size.width as i32 - (400.0 * scale) as i32;
                let y = position.y + size.height as i32 - (365.0 * scale) as i32;
                let _ = window.set_position(tauri::PhysicalPosition::new(
                    x.max(position.x),
                    y.max(position.y),
                ));
            }
        }
        Err(error) => log::write("error", &format!("提醒窗口初始化失败：{error}")),
    }
}

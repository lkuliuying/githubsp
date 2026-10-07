use crate::{
    manager::{Action, Manager},
    model::Snapshot,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager as _,
};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct DesktopState {
    pub exiting: AtomicBool,
    pub close_to_tray: AtomicBool,
}

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window
            .unminimize()
            .and_then(|()| window.show())
            .and_then(|()| window.set_focus())
        {
            eprintln!("无法激活窗口：{error}");
        }
    }
}

pub fn update(app: &AppHandle, snapshot: &Snapshot) {
    app.state::<DesktopState>()
        .close_to_tray
        .store(snapshot.settings.close_to_tray, Ordering::SeqCst);
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = if snapshot.notices.is_empty() {
            "GitHubSP · 下载工作台".into()
        } else {
            format!(
                "GitHubSP · {} 条未读提醒\n{}",
                snapshot.notices.len(),
                snapshot
                    .notices
                    .last()
                    .map(|n| n.message.as_str())
                    .unwrap_or_default()
            )
        };
        let tooltip: String = tooltip.chars().take(100).collect();
        if let Err(error) = tray.set_tooltip(Some(tooltip)) {
            eprintln!("托盘提示更新失败：{error}");
        }
    }
}

pub fn exit(app: &AppHandle) {
    if app
        .state::<DesktopState>()
        .exiting
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    let manager = app.state::<Manager>().inner().clone();
    if let Some(catalog) = app.try_state::<crate::catalog::Catalog>() {
        catalog.stop();
    }
    if let Some(library) = app.try_state::<crate::library::Library>() {
        library.stop();
    }
    tauri::async_runtime::spawn(async move {
        match manager.shutdown().await {
            Ok(_) => app.exit(0),
            Err(error) => {
                app.state::<DesktopState>()
                    .exiting
                    .store(false, Ordering::SeqCst);
                show(&app);
                app.dialog()
                    .message(format!("保存任务状态失败，尚未退出：{error}"))
                    .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                    .title("GitHubSP")
                    .show(|_| {});
            }
        }
    });
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口 / 查看提醒", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停当前下载", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "exit", "保存进度并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &pause, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("GitHubSP · 下载工作台")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    button_state: tauri::tray::MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show(app),
            "exit" => exit(app),
            "pause" => {
                let app = app.clone();
                let manager = app.state::<Manager>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let result = async {
                        let snapshot = manager.snapshot().await?;
                        if let Some(task) = snapshot.tasks.iter().find(|t| t.status.running()) {
                            manager.action(task.id.clone(), Action::Pause).await?;
                        }
                        Ok::<_, crate::error::DownloadError>(())
                    }
                    .await;
                    if let Err(error) = result {
                        app.dialog()
                            .message(error.to_string())
                            .title("无法暂停下载")
                            .show(|_| {});
                    }
                });
            }
            _ => (),
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

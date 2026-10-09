use crate::{service::Service, MainWindow};
use slint::{winit_030::WinitWindowAccessor, ComponentHandle};
use std::sync::{atomic::Ordering, Arc};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

pub fn install(
    window: &MainWindow,
    service: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "显示主窗口 / 查看提醒", true, None);
    let pause = MenuItem::with_id("pause", "暂停下载与自动恢复", true, None);
    let exit = MenuItem::with_id("exit", "保存进度并退出", true, None);
    menu.append_items(&[&show, &pause, &exit])?;
    let pixels = image::load_from_memory(include_bytes!("../resources/tray.png"))?.into_rgba8();
    let icon = tray_icon::Icon::from_rgba(pixels.to_vec(), pixels.width(), pixels.height())?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("GitHubSP · 下载工作台")
        .with_menu_on_left_click(false)
        .build()?;
    let weak = window.as_weak();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = weak.upgrade_in_event_loop(|window| window.invoke_show_main());
        }
    }));
    let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.as_ref() {
        "show" => {
            let _ = weak.upgrade_in_event_loop(|window| window.invoke_show_main());
        }
        "exit" => {
            let _ = weak.upgrade_in_event_loop(|window| window.invoke_exit());
        }
        "pause" => {
            if api.is_closing() {
                return;
            }
            let (api, weak) = (api.clone(), weak.clone());
            runtime.spawn(async move {
                let result = async {
                    let state = api.manager.snapshot().await?;
                    for task in state.tasks.iter().filter(|t| {
                        t.status.running()
                            || t.status == githubsp_lib::model::TaskStatus::WaitingNetwork
                    }) {
                        api.manager
                            .action(task.id.clone(), githubsp_lib::manager::Action::Pause)
                            .await?;
                    }
                    Ok::<_, githubsp_lib::error::DownloadError>(())
                }
                .await;
                if let Err(error) = result {
                    let _ = weak.upgrade_in_event_loop(move |window| {
                        window.set_message(format!("无法暂停下载：{error}").into())
                    });
                }
            });
        }
        _ => (),
    }));
    let (weak, api) = (window.as_weak(), service.clone());
    window.on_hide_to_tray(move || {
        if let Some(window) = weak.upgrade() {
            match window.hide() {
                Ok(()) => api.visible.store(false, Ordering::Release),
                Err(error) => window.set_message(format!("收起窗口失败：{error}").into()),
            }
        }
    });
    let (weak, api) = (window.as_weak(), service.clone());
    window.on_show_main(move || {
        if let Some(window) = weak.upgrade() {
            match window.show() {
                Ok(()) => {
                    api.visible.store(true, Ordering::Release);
                    window.window().with_winit_window(|native| {
                        native.set_minimized(false);
                        native.focus_window();
                    });
                    window.invoke_refresh_state();
                }
                Err(error) => window.set_message(format!("显示窗口失败：{error}").into()),
            }
        }
    });
    let (weak, api) = (window.as_weak(), service.clone());
    window.window().on_close_requested(move || {
        if api.is_closing() {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        if let Some(window) = weak.upgrade() {
            if api
                .state
                .borrow()
                .as_ref()
                .is_some_and(|s| s.settings.close_to_tray)
            {
                window.invoke_hide_to_tray();
            } else {
                window.invoke_exit();
            }
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = window.as_weak();
    window.window().on_winit_window_event(move |_, event| {
        if matches!(
            event,
            slint::winit_030::winit::event::WindowEvent::Focused(true)
        ) {
            if let Some(window) = weak.upgrade() {
                window.invoke_refresh_state();
            }
        }
        slint::winit_030::EventResult::Propagate
    });
    Ok(tray)
}

pub fn hwnd(window: &slint::Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window
        .with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as _),
            _ => None,
        })
        .flatten()
}

pub fn is_background(window: &MainWindow) -> bool {
    !window.window().is_visible() || hwnd(window.window()).is_none_or(|handle| unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } != handle)
}

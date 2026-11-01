use crate::log;
use crate::{
    completion::CreationContext, desktop, service::Service, MainWindow, Reminder, ReminderWindow,
    Workspace,
};
use githubsp_lib::model::Snapshot;
use slint::{ComponentHandle, Weak};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// 收藏更新提醒窗：后台且有未读更新时显示一次，用户可查看、下载或忽略。
pub struct Host {
    window: RefCell<Option<ReminderWindow>>,
    shown: RefCell<String>,
    main: Weak<MainWindow>,
    api: Arc<Service>,
    runtime: tokio::runtime::Handle,
    creation: CreationContext,
}

impl Host {
    pub fn new(
        main: &MainWindow,
        creation: CreationContext,
        api: Arc<Service>,
        runtime: tokio::runtime::Handle,
    ) -> Rc<Self> {
        Rc::new(Self {
            window: RefCell::new(None),
            shown: RefCell::new(String::new()),
            main: main.as_weak(),
            api,
            runtime,
            creation,
        })
    }

    pub fn update(self: &Rc<Self>, snapshot: &Snapshot) {
        let Some(main) = self.main.upgrade() else {
            return;
        };
        let unread: Vec<_> = snapshot
            .favorites
            .iter()
            .filter(|favorite| snapshot.watch_state.is_unread(favorite))
            .collect();
        let key = unread
            .iter()
            .filter_map(|favorite| favorite.latest.as_ref().map(|latest| latest.id))
            .max()
            .map(|id| id.to_string())
            .unwrap_or_default();
        let should_show = !unread.is_empty()
            && snapshot.settings.watch_notifications
            && desktop::is_background(&main);
        if !should_show {
            if let Some(window) = self.window.borrow().as_ref() {
                let _ = window.hide();
            }
            // 仅当没有未读或提醒被关闭时清空去重键；暂时回到前台不应导致同一版本反复弹出。
            if unread.is_empty() || !snapshot.settings.watch_notifications {
                self.shown.replace(String::new());
            }
            return;
        }
        if *self.shown.borrow() == key {
            return;
        }
        // 提醒窗已经显示时只刷新内容，避免逐项忽略后窗口被反复重置。
        if self
            .window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
        {
            self.shown.replace(key);
            return;
        }
        let window = match self.ensure_window() {
            Ok(window) => window,
            Err(error) => {
                main.set_message(format!("更新提醒显示失败：{error}").into());
                return;
            }
        };
        if let Err(error) = place(&window, &main) {
            main.set_message(format!("更新提醒显示失败：{error}").into());
            return;
        }
        window.set_entered(false);
        if let Err(error) = window.show() {
            main.set_message(format!("更新提醒显示失败：{error}").into());
            return;
        }
        window.set_entered(true);
        self.shown.replace(key);
    }

    fn ensure_window(self: &Rc<Self>) -> Result<ReminderWindow, String> {
        if let Some(window) = self.window.borrow().as_ref() {
            return Ok(window.clone_strong());
        }
        let window = self
            .creation
            .during(ReminderWindow::new)
            .map_err(|error| error.to_string())?;
        let weak = Rc::downgrade(self);
        let window_weak = window.as_weak();
        window.global::<Reminder>().on_dismiss(move || {
            if let Some(window) = window_weak.upgrade() {
                let _ = window.hide();
            }
        });
        let (weak_host, window_weak) = (weak.clone(), window.as_weak());
        window.global::<Reminder>().on_open(move || {
            if let Some(window) = window_weak.upgrade() {
                let _ = window.hide();
            }
            if let Some(host) = weak_host.upgrade() {
                if let Some(main) = host.main.upgrade() {
                    main.global::<Workspace>().invoke_navigate(2);
                    main.invoke_show_main();
                }
            }
        });
        let (weak_host, window_weak) = (weak.clone(), window.as_weak());
        window.global::<Reminder>().on_download(move |repository| {
            if let Some(window) = window_weak.upgrade() {
                let _ = window.hide();
            }
            if let Some(host) = weak_host.upgrade() {
                let (library, repository) = (host.api.library.clone(), repository.to_string());
                let runtime = host.runtime.clone();
                let weak_main = host.main.clone();
                runtime.spawn(async move {
                    if let Err(error) = library.download_latest(repository).await {
                        log::write("warn", &format!("下载最新版失败：{error}"));
                        let message = format!("下载最新版失败：{error}");
                        let _ = weak_main.upgrade_in_event_loop(move |window| {
                            window.global::<Workspace>().set_message(message.into());
                            window.invoke_show_main();
                        });
                    }
                });
            }
        });
        let (weak_host, window_weak) = (weak.clone(), window.as_weak());
        window
            .global::<Reminder>()
            .on_ignore(move |repository, _release| {
                if let Some(window) = window_weak.upgrade() {
                    let _ = window.hide();
                }
                if let Some(host) = weak_host.upgrade() {
                    let (library, repository) = (host.api.library.clone(), repository.to_string());
                    let runtime = host.runtime.clone();
                    runtime.spawn(async move {
                        if let Err(error) = library.dismiss(Some(repository)).await {
                            log::write("warn", &format!("忽略更新失败：{error}"));
                        }
                    });
                }
            });
        let (window_weak, host) = (window.as_weak(), Rc::downgrade(self));
        window.global::<Reminder>().on_ignore_all(move || {
            if let Some(window) = window_weak.upgrade() {
                let _ = window.hide();
            }
            if let Some(host) = host.upgrade() {
                let (library, runtime) = (host.api.library.clone(), host.runtime.clone());
                runtime.spawn(async move {
                    if let Err(error) = library.dismiss(None).await {
                        log::write("warn", &format!("忽略全部更新失败：{error}"));
                    }
                });
            }
        });
        window
            .window()
            .on_close_requested(|| slint::CloseRequestResponse::KeepWindowShown);
        *self.window.borrow_mut() = Some(window.clone_strong());
        Ok(window)
    }
}

fn place(window: &ReminderWindow, main: &MainWindow) -> Result<(), String> {
    use windows_sys::Win32::{
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::*,
    };
    let popup = desktop::hwnd(window.window()).ok_or("无法取得提醒窗口句柄")?;
    let scale = main.window().scale_factor();
    let main = desktop::hwnd(main.window()).ok_or("无法取得主窗口句柄")?;
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(MonitorFromWindow(main, MONITOR_DEFAULTTONEAREST), &mut info) } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let area = info.rcWork;
    let width = ((430.0 * scale) as i32).min(area.right - area.left);
    let height = ((360.0 * scale) as i32).min(area.bottom - area.top);
    let margin = (16.0 * scale) as i32;
    // 完成提醒在底部，更新提醒叠在其上方，避免互相遮挡。
    let bottom = area.bottom - margin - ((170.0 * scale) as i32) - margin;
    unsafe {
        if SetWindowPos(
            popup,
            HWND_TOPMOST,
            (area.right - width - margin).max(area.left),
            (bottom - height).max(area.top),
            width,
            height,
            SWP_NOACTIVATE,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

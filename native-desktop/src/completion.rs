use crate::{desktop, CompletionWindow, MainWindow};
use githubsp_lib::{model::Snapshot, notice::NoticeState};
use slint::{
    winit_030::{winit, EventResult, WinitWindowAccessor},
    ComponentHandle, Timer, TimerMode,
};
use std::{cell::RefCell, rc::Rc, time::Instant};

pub struct Host {
    state: RefCell<NoticeState>,
    popup: RefCell<Option<CompletionWindow>>,
    timer: Timer,
    main: slint::Weak<MainWindow>,
}

impl Host {
    pub fn new(main: &MainWindow) -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(NoticeState::default()),
            popup: RefCell::new(None),
            timer: Timer::default(),
            main: main.as_weak(),
        })
    }

    pub fn update(self: &Rc<Self>, snapshot: &Snapshot) {
        let Some(main) = self.main.upgrade() else {
            return;
        };
        let background = desktop::is_background(&main);
        let mut changed = self.state.borrow_mut().ingest(snapshot, background);
        if !background {
            changed |= self.state.borrow_mut().dismiss(None);
        }
        if !changed {
            return;
        }
        if let Err(error) = self.render() {
            self.state.borrow_mut().dismiss(None);
            self.timer.stop();
            self.popup.borrow_mut().take();
            main.set_message(format!("完成提醒显示失败：{error}").into());
        }
    }

    fn render(self: &Rc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        self.state.borrow_mut().expire(Instant::now());
        let view = self.state.borrow().view().clone();
        let Some(notice) = view.notice else {
            self.timer.stop();
            if let Some(popup) = self.popup.borrow_mut().take() {
                popup.hide()?;
            }
            return Ok(());
        };
        if self.popup.borrow().is_none() {
            let popup = CompletionWindow::new()?;
            let weak = Rc::downgrade(self);
            popup.window().set_rendering_notifier(move |stage, _| {
                if matches!(stage, slint::RenderingState::AfterRendering) {
                    if let Some(host) = weak.upgrade() {
                        let revision = host.state.borrow().view().revision;
                        host.state.borrow_mut().presented(revision, Instant::now());
                        host.arm();
                    }
                }
            })?;
            let weak = Rc::downgrade(self);
            popup.on_dismiss(move || {
                if let Some(host) = weak.upgrade() {
                    host.dismiss();
                }
            });
            let weak = Rc::downgrade(self);
            popup.on_open(move || {
                if let Some(host) = weak.upgrade() {
                    host.dismiss();
                    if let Some(main) = host.main.upgrade() {
                        main.global::<crate::Workspace>().invoke_navigate(0);
                        main.invoke_show_main();
                    }
                }
            });
            let weak = Rc::downgrade(self);
            popup.on_hover(move |hovering| {
                if let Some(host) = weak.upgrade() {
                    host.state
                        .borrow_mut()
                        .interaction(Some(hovering), None, Instant::now());
                    host.arm();
                }
            });
            let weak = Rc::downgrade(self);
            popup.window().on_winit_window_event(move |_, event| {
                if let (Some(host), winit::event::WindowEvent::Focused(focused)) =
                    (weak.upgrade(), event)
                {
                    host.state
                        .borrow_mut()
                        .interaction(None, Some(*focused), Instant::now());
                    host.arm();
                }
                EventResult::Propagate
            });
            self.popup.replace(Some(popup));
        }
        {
            let popup = self.popup.borrow();
            let popup = popup.as_ref().ok_or("提醒窗口不可用")?;
            popup.set_message(format!("{} 个下载已完成", notice.count).into());
            let elapsed = notice
                .elapsed_ms
                .map(|ms| {
                    format!(
                        " · 用时 {:.1} 秒{}",
                        ms as f64 / 1000.0,
                        if notice.elapsed_is_partial {
                            "（记录不完整）"
                        } else {
                            ""
                        }
                    )
                })
                .unwrap_or_default();
            popup.set_detail(format!("{}{elapsed}", notice.filename).into());
            popup.show()?;
            if let Some(main) = self.main.upgrade() {
                place(popup, &main)?;
            }
        }
        self.arm();
        Ok(())
    }

    fn dismiss(&self) {
        self.state.borrow_mut().dismiss(None);
        self.timer.stop();
        if let Some(popup) = self.popup.borrow_mut().take() {
            if let Err(error) = popup.hide() {
                eprintln!("关闭完成提醒失败：{error}");
            }
        }
    }

    fn arm(self: &Rc<Self>) {
        self.timer.stop();
        if let Some(deadline) = self.state.borrow().deadline() {
            let weak = Rc::downgrade(self);
            self.timer.start(
                TimerMode::SingleShot,
                deadline.saturating_duration_since(Instant::now()),
                move || {
                    if let Some(host) = weak.upgrade() {
                        if let Err(error) = host.render() {
                            eprintln!("完成提醒计时失败：{error}");
                            host.dismiss();
                        }
                    }
                },
            );
        }
    }
}

fn place(popup: &CompletionWindow, main: &MainWindow) -> Result<(), String> {
    use windows_sys::Win32::{
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::*,
    };
    let window = desktop::hwnd(popup.window()).ok_or("无法取得提醒窗口句柄")?;
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
    let width = ((380.0 * scale) as i32).min(area.right - area.left);
    let height = ((170.0 * scale) as i32).min(area.bottom - area.top);
    let margin = (16.0 * scale) as i32;
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        SetWindowLongPtrW(window, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW as isize);
        if SetWindowPos(
            window,
            HWND_TOPMOST,
            (area.right - width - margin).max(area.left),
            (area.bottom - height - margin).max(area.top),
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

use crate::{desktop, CompletionWindow, MainWindow};
use githubsp_lib::{model::Snapshot, notice::NoticeState};
use slint::{
    winit_030::{winit, EventResult, WinitWindowAccessor},
    ComponentHandle, Timer, TimerMode,
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

const INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct CreationContext(Rc<Cell<bool>>);

impl CreationContext {
    pub fn attributes(
        &self,
        attributes: winit::window::WindowAttributes,
    ) -> winit::window::WindowAttributes {
        use winit::platform::windows::WindowAttributesExtWindows;
        if self.0.get() {
            attributes
                .with_active(false)
                .with_skip_taskbar(true)
                .with_visible(false)
        } else {
            attributes
        }
    }

    fn during<T>(&self, create: impl FnOnce() -> T) -> T {
        struct Restore<'a>(&'a Cell<bool>, bool);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(&self.0, self.0.replace(true));
        create()
    }
}

type Ready = Pin<Box<dyn Future<Output = Result<(), String>>>>;

trait PopupPlatform {
    fn create(&self) -> Result<CompletionWindow, String> {
        CompletionWindow::new().map_err(|error| error.to_string())
    }
    fn ready(&self, popup: slint::Weak<CompletionWindow>) -> Ready;
    fn observe_render(
        &self,
        popup: &CompletionWindow,
        rendered: Box<dyn FnMut()>,
    ) -> Result<(), String>;
    fn place(&self, popup: &CompletionWindow, main: &MainWindow) -> Result<(), String>;
    fn background(&self, main: &MainWindow) -> bool;
    fn hide(&self, popup: &CompletionWindow) -> Result<(), String> {
        popup.hide().map_err(|error| error.to_string())
    }
}

struct NativePlatform;
impl PopupPlatform for NativePlatform {
    fn ready(&self, popup: slint::Weak<CompletionWindow>) -> Ready {
        Box::pin(async move {
            let popup = popup.upgrade().ok_or("提醒窗口已释放")?;
            let native = popup.window().winit_window().await;
            drop(native);
            Ok(())
        })
    }
    fn observe_render(
        &self,
        popup: &CompletionWindow,
        mut rendered: Box<dyn FnMut()>,
    ) -> Result<(), String> {
        let weak = popup.as_weak();
        popup
            .window()
            .set_rendering_notifier(move |stage, _| {
                if !matches!(stage, slint::RenderingState::AfterRendering) {
                    return;
                }
                // Slint 会在映射原生窗口前预绘制一次，该帧不能提前消耗可见时间。
                let visible = weak.upgrade().is_some_and(|popup| {
                    popup
                        .window()
                        .with_winit_window(|native| native.is_visible() == Some(true))
                        .unwrap_or(false)
                });
                if visible {
                    rendered();
                }
            })
            .map_err(|error| error.to_string())
    }
    fn place(&self, popup: &CompletionWindow, main: &MainWindow) -> Result<(), String> {
        place(popup, main)
    }
    fn background(&self, main: &MainWindow) -> bool {
        desktop::is_background(main)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing,
    Visible,
    Closing,
}

struct Session {
    id: u64,
    revision: u64,
    phase: Phase,
    window: CompletionWindow,
}

struct Preparation {
    task: slint::JoinHandle<()>,
    cancel: oneshot::Sender<()>,
}

pub struct Host {
    state: RefCell<NoticeState>,
    popup: RefCell<Option<Session>>,
    next_id: Cell<u64>,
    preparation: RefCell<Option<Preparation>>,
    initialization_timer: Timer,
    timer: Timer,
    hiding: Cell<bool>,
    main: slint::Weak<MainWindow>,
    creation: CreationContext,
    platform: Rc<dyn PopupPlatform>,
}

impl Host {
    pub fn new(main: &MainWindow, creation: CreationContext) -> Rc<Self> {
        Self::with_platform(main, creation, Rc::new(NativePlatform))
    }

    fn with_platform(
        main: &MainWindow,
        creation: CreationContext,
        platform: Rc<dyn PopupPlatform>,
    ) -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(NoticeState::default()),
            popup: RefCell::new(None),
            next_id: Cell::new(0),
            preparation: RefCell::new(None),
            initialization_timer: Timer::default(),
            timer: Timer::default(),
            hiding: Cell::new(false),
            main: main.as_weak(),
            creation,
            platform,
        })
    }

    pub fn update(self: &Rc<Self>, snapshot: &Snapshot) {
        let Some(main) = self.main.upgrade() else {
            self.shutdown();
            return;
        };
        let background = self.platform.background(&main);
        let changed = self.state.borrow_mut().ingest(snapshot, background);
        if !background || !snapshot.settings.background_completion_notice {
            self.shutdown();
        } else if changed {
            if let Err(error) = self.render() {
                self.fail(error);
            }
        }
    }

    fn render(self: &Rc<Self>) -> Result<(), String> {
        let view = self.state.borrow().view().clone();
        let Some(notice) = view.notice else {
            return self.close_popup();
        };
        let closing = self
            .popup
            .borrow()
            .as_ref()
            .is_some_and(|session| session.phase == Phase::Closing);
        if closing {
            self.close_popup()?;
        }
        let created = self.popup.borrow().is_none();
        if created {
            let id = self
                .next_id
                .get()
                .checked_add(1)
                .ok_or("提醒窗口会话编号耗尽")?;
            self.next_id.set(id);
            let popup = self.creation.during(|| self.platform.create())?;
            self.bind(&popup, id);
            self.popup.replace(Some(Session {
                id,
                revision: view.revision,
                phase: Phase::Preparing,
                window: popup,
            }));
        }
        let (id, popup) = {
            let mut slot = self.popup.borrow_mut();
            let session = slot.as_mut().ok_or("提醒窗口不可用")?;
            if session.phase == Phase::Closing {
                return Ok(());
            }
            session.revision = view.revision;
            (session.id, session.window.clone_strong())
        };
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
        if created {
            self.prepare(id, &popup)?;
        }
        self.arm();
        Ok(())
    }

    fn bind(self: &Rc<Self>, popup: &CompletionWindow, id: u64) {
        let weak = Rc::downgrade(self);
        popup.on_dismiss(move || {
            if let Some(host) = weak.upgrade() {
                host.dismiss_session(id);
            }
        });
        let weak = Rc::downgrade(self);
        popup.on_open(move || {
            if let Some(host) = weak.upgrade() {
                if host.dismiss_session(id) {
                    if let Some(main) = host.main.upgrade() {
                        main.global::<crate::Workspace>().invoke_navigate(0);
                        main.invoke_show_main();
                    }
                }
            }
        });
        let weak = Rc::downgrade(self);
        popup.on_hover(move |hovering| {
            if let Some(host) = weak.upgrade() {
                host.interaction(id, Some(hovering), None, Instant::now());
            }
        });
        let weak = Rc::downgrade(self);
        popup.window().on_winit_window_event(move |_, event| {
            if let (Some(host), winit::event::WindowEvent::Focused(focused)) =
                (weak.upgrade(), event)
            {
                host.interaction(id, None, Some(*focused), Instant::now());
            }
            EventResult::Propagate
        });
        let weak = Rc::downgrade(self);
        popup.window().on_close_requested(move || {
            if let Some(host) = weak.upgrade() {
                host.dismiss_session(id);
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    fn prepare(self: &Rc<Self>, id: u64, popup: &CompletionWindow) -> Result<(), String> {
        let weak = Rc::downgrade(self);
        self.platform.observe_render(
            popup,
            Box::new(move || {
                if let Some(host) = weak.upgrade() {
                    if let Some(revision) = host.visible_revision(id) {
                        host.presented(id, revision, Instant::now());
                    }
                }
            }),
        )?;
        let ready = self.platform.ready(popup.as_weak());
        let (cancel, stopped) = oneshot::channel();
        let weak = Rc::downgrade(self);
        let task = slint::spawn_local(async move {
            let result = tokio::select! { biased; _ = stopped => return, result = ready => result };
            if let Some(host) = weak.upgrade() {
                host.prepared(id, result);
            }
        })
        .map_err(|error| error.to_string())?;
        self.preparation.replace(Some(Preparation { task, cancel }));
        let weak = Rc::downgrade(self);
        self.initialization_timer
            .start(TimerMode::SingleShot, INITIALIZATION_TIMEOUT, move || {
                if let Some(host) = weak.upgrade() {
                    host.initialization_expired(id);
                }
            });
        Ok(())
    }

    fn prepared(self: &Rc<Self>, id: u64, result: Result<(), String>) {
        if !self.has_phase(id, Phase::Preparing) {
            return;
        }
        self.stop_preparation();
        if let Err(error) = result {
            self.fail(error);
            return;
        }
        let Some(main) = self.main.upgrade() else {
            self.shutdown();
            return;
        };
        if !self.platform.background(&main) {
            self.shutdown();
            return;
        }
        let Some(popup) = self.window(id) else {
            return;
        };
        let result = self.platform.place(&popup, &main);
        if !self.has_phase(id, Phase::Preparing) {
            return;
        }
        if let Err(error) = result {
            self.fail(error);
            return;
        }
        if let Some(session) = self.popup.borrow_mut().as_mut() {
            session.phase = Phase::Visible;
        }
        if let Err(error) = popup.show() {
            if self.has_phase(id, Phase::Visible) {
                self.fail(error.to_string());
            }
        }
    }

    fn initialization_expired(&self, id: u64) {
        if self.has_phase(id, Phase::Preparing) {
            self.fail("等待提醒窗口创建超时（5 秒）".into());
        }
    }

    fn presented(self: &Rc<Self>, id: u64, revision: u64, now: Instant) {
        if self.visible_revision(id) == Some(revision) {
            self.state.borrow_mut().presented(revision, now);
            self.arm();
        }
    }

    fn interaction(
        self: &Rc<Self>,
        id: u64,
        hovering: Option<bool>,
        focused: Option<bool>,
        now: Instant,
    ) {
        if self.visible_revision(id).is_some() {
            self.state.borrow_mut().interaction(hovering, focused, now);
            self.arm();
        }
    }

    fn window(&self, id: u64) -> Option<CompletionWindow> {
        self.popup
            .borrow()
            .as_ref()
            .filter(|session| session.id == id)
            .map(|session| session.window.clone_strong())
    }
    fn has_phase(&self, id: u64, phase: Phase) -> bool {
        self.popup
            .borrow()
            .as_ref()
            .is_some_and(|session| session.id == id && session.phase == phase)
    }
    fn visible_revision(&self, id: u64) -> Option<u64> {
        self.popup
            .borrow()
            .as_ref()
            .filter(|session| session.id == id && session.phase == Phase::Visible)
            .map(|session| session.revision)
    }

    fn stop_preparation(&self) {
        self.initialization_timer.stop();
        let preparation = self.preparation.borrow_mut().take();
        if let Some(preparation) = preparation {
            // abort 只标记取消；同时唤醒等待任务，及时释放其持有的窗口引用。
            preparation.task.abort();
            let _ = preparation.cancel.send(());
        }
    }

    fn close_popup(&self) -> Result<(), String> {
        self.stop_preparation();
        self.timer.stop();
        if self.hiding.get() {
            return Ok(());
        }
        let window = {
            let mut slot = self.popup.borrow_mut();
            slot.as_mut().map(|session| {
                session.phase = Phase::Closing;
                (session.id, session.window.clone_strong())
            })
        };
        if let Some((id, popup)) = window {
            // 平台调用可能同步产生焦点或关闭回调，调用期间不持有内部借用。
            self.hiding.set(true);
            let result = self.platform.hide(&popup);
            self.hiding.set(false);
            result?;
            let removed = {
                let mut slot = self.popup.borrow_mut();
                if slot.as_ref().is_some_and(|session| session.id == id) {
                    slot.take()
                } else {
                    None
                }
            };
            drop(removed);
        }
        Ok(())
    }

    fn dismiss_session(&self, id: u64) -> bool {
        if self.window(id).is_none() {
            return false;
        }
        self.shutdown();
        self.window(id).is_none()
    }

    pub fn shutdown(&self) {
        self.state.borrow_mut().dismiss(None);
        if let Err(error) = self.close_popup() {
            self.report(format!("关闭完成提醒失败：{error}"));
        }
    }

    fn fail(&self, error: String) {
        self.state.borrow_mut().dismiss(None);
        let message = match self.close_popup() {
            Ok(()) => format!("完成提醒显示失败：{error}"),
            Err(close) => format!("完成提醒显示失败：{error}；收起失败：{close}"),
        };
        self.report(message);
    }

    fn report(&self, message: String) {
        eprintln!("{message}");
        if let Some(main) = self.main.upgrade() {
            main.set_message(message.into());
        }
    }

    fn expire(self: &Rc<Self>, id: u64, revision: u64, now: Instant) {
        if self.visible_revision(id) != Some(revision) {
            return;
        }
        let expired = self.state.borrow_mut().expire(now);
        if expired {
            self.shutdown();
        } else {
            self.arm();
        }
    }

    fn arm(self: &Rc<Self>) {
        self.timer.stop();
        let deadline = self.state.borrow().deadline();
        let session = self
            .popup
            .borrow()
            .as_ref()
            .filter(|session| session.phase == Phase::Visible)
            .map(|session| (session.id, session.revision));
        if let (Some(deadline), Some((id, revision))) = (deadline, session) {
            let weak = Rc::downgrade(self);
            self.timer.start(
                TimerMode::SingleShot,
                deadline.saturating_duration_since(Instant::now()),
                move || {
                    if let Some(host) = weak.upgrade() {
                        host.expire(id, revision, Instant::now());
                    }
                },
            );
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.shutdown();
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

#[cfg(test)]
pub(crate) mod tests;

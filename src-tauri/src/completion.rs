use crate::model::{NoticeKind, Snapshot};
use serde::Serialize;
use std::{
    collections::HashSet,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub const WINDOW: &str = "download-notice";
const EVENT: &str = "download-notice-changed";
const DISPLAY_TIME: Duration = Duration::from_secs(8);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionNotice {
    pub count: u32,
    pub filename: String,
    pub elapsed_ms: Option<u64>,
    pub elapsed_is_partial: bool,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeView {
    pub revision: u64,
    pub notice: Option<CompletionNotice>,
}

#[derive(Default)]
struct NoticeState {
    view: NoticeView,
    snapshot_revision: Option<u64>,
    seen: HashSet<String>,
    presented: Option<u64>,
    deadline: Option<Instant>,
    remaining: Duration,
    hovering: bool,
    focused: bool,
}

impl NoticeState {
    fn ingest(&mut self, snapshot: &Snapshot, background: bool) -> bool {
        if self
            .snapshot_revision
            .is_some_and(|r| snapshot.revision <= r)
        {
            return false;
        }
        self.snapshot_revision = Some(snapshot.revision);
        let mut changed = false;
        for notice in &snapshot.notices {
            if !self.seen.contains(&notice.id)
                && notice.kind == NoticeKind::DownloadCompleted
                && snapshot.settings.background_completion_notice
                && background
            {
                if let Some(task) = snapshot
                    .tasks
                    .iter()
                    .find(|t| Some(&t.id) == notice.task_id.as_ref())
                {
                    let count = self
                        .view
                        .notice
                        .as_ref()
                        .map_or(1, |n| n.count.saturating_add(1));
                    self.view.notice = Some(CompletionNotice {
                        count,
                        filename: task.filename.clone(),
                        elapsed_ms: task.details.elapsed_ms,
                        elapsed_is_partial: task.details.elapsed_is_partial,
                    });
                    changed = true;
                }
            }
        }
        // 已读清理和旧快照不能重放提醒；只保留当前最多 100 个通知标识。
        self.seen = snapshot.notices.iter().map(|n| n.id.clone()).collect();
        if !snapshot.settings.background_completion_notice || !background {
            return self.dismiss(None);
        }
        if changed {
            self.view.revision += 1;
            self.presented = None;
            self.deadline = None;
            self.remaining = DISPLAY_TIME;
        }
        changed
    }

    fn dismiss(&mut self, revision: Option<u64>) -> bool {
        if revision.is_some_and(|r| r != self.view.revision) || self.view.notice.is_none() {
            return false;
        }
        self.view.revision += 1;
        self.view.notice = None;
        self.presented = None;
        self.deadline = None;
        self.hovering = false;
        self.focused = false;
        true
    }

    fn presented(&mut self, revision: u64, now: Instant) {
        if revision != self.view.revision
            || self.view.notice.is_none()
            || self.presented == Some(revision)
        {
            return;
        }
        self.presented = Some(revision);
        if !self.hovering && !self.focused {
            self.deadline = Some(now + self.remaining);
        }
    }

    fn interaction(&mut self, hovering: Option<bool>, focused: Option<bool>, now: Instant) {
        let was_paused = self.hovering || self.focused;
        if let Some(value) = hovering {
            self.hovering = value;
        }
        if let Some(value) = focused {
            self.focused = value;
        }
        let paused = self.hovering || self.focused;
        if paused && !was_paused {
            if let Some(deadline) = self.deadline.take() {
                self.remaining = deadline.saturating_duration_since(now);
            }
        } else if !paused
            && was_paused
            && self.presented == Some(self.view.revision)
            && self.view.notice.is_some()
        {
            self.deadline = Some(now + self.remaining);
        }
    }

    fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.dismiss(None)
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct CompletionHost {
    state: Mutex<NoticeState>,
    wake: Notify,
    stop: CancellationToken,
}

impl CompletionHost {
    fn lock(&self) -> Result<MutexGuard<'_, NoticeState>, String> {
        self.state.lock().map_err(|_| "桌面提醒状态不可用".into())
    }

    pub fn dismiss(&self, revision: Option<u64>) -> Result<(), String> {
        if self.lock()?.dismiss(revision) {
            self.wake.notify_one();
        }
        Ok(())
    }

    pub fn focus(&self, focused: bool) -> Result<(), String> {
        self.lock()?
            .interaction(None, Some(focused), Instant::now());
        self.wake.notify_one();
        Ok(())
    }

    pub fn stop(&self) {
        self.stop.cancel();
    }
}

pub fn allowed_command(window: &str, command: &str) -> bool {
    let notification_command = matches!(
        command,
        "read_download_notice"
            | "present_download_notice"
            | "dismiss_download_notice"
            | "hover_download_notice"
            | "open_download_notice"
    );
    (window == "main" && !notification_command) || (window == WINDOW && notification_command)
}

pub fn is_background(app: &AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("主窗口不可用")?;
    Ok(!window.is_visible().map_err(|e| e.to_string())?
        || window.is_minimized().map_err(|e| e.to_string())?
        || !window.is_focused().map_err(|e| e.to_string())?)
}

pub fn update(app: &AppHandle, snapshot: &Snapshot) {
    let result = (|| {
        let host = app.state::<CompletionHost>();
        let background = is_background(app)? && !host.stop.is_cancelled();
        if host.lock()?.ingest(snapshot, background) {
            host.wake.notify_one();
        }
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        eprintln!("桌面提醒更新失败：{error}");
    }
}

#[tauri::command]
pub fn read_download_notice(host: tauri::State<'_, CompletionHost>) -> Result<NoticeView, String> {
    Ok(host.lock()?.view.clone())
}

#[tauri::command]
pub fn present_download_notice(
    revision: u64,
    host: tauri::State<'_, CompletionHost>,
) -> Result<(), String> {
    host.lock()?.presented(revision, Instant::now());
    host.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn dismiss_download_notice(
    revision: u64,
    host: tauri::State<'_, CompletionHost>,
) -> Result<(), String> {
    host.dismiss(Some(revision))
}

#[tauri::command]
pub fn hover_download_notice(
    revision: u64,
    hovering: bool,
    host: tauri::State<'_, CompletionHost>,
) -> Result<(), String> {
    let mut state = host.lock()?;
    if revision == state.view.revision && state.view.notice.is_some() {
        state.interaction(Some(hovering), None, Instant::now());
    }
    host.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn open_download_notice(revision: u64, app: AppHandle) -> Result<(), String> {
    let host = app.state::<CompletionHost>();
    {
        let state = host.lock()?;
        if revision != state.view.revision || state.view.notice.is_none() {
            return Ok(());
        }
    }
    app.emit_to("main", "show-downloads", ())
        .map_err(|e| e.to_string())?;
    let window = app.get_webview_window("main").ok_or("主窗口不可用")?;
    window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus())
        .map_err(|e| e.to_string())?;
    host.dismiss(Some(revision))
}

pub fn start(app: &AppHandle) {
    app.manage(CompletionHost::default());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let host = app.state::<CompletionHost>();
        let mut emitted = None;
        loop {
            let wait = match host.lock() {
                Ok(state) => state
                    .deadline
                    .map(|d| d.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(86400)),
                Err(error) => {
                    eprintln!("桌面提醒停止：{error}");
                    break;
                }
            };
            tokio::select! {
                _ = host.stop.cancelled() => break,
                _ = host.wake.notified() => (),
                _ = tokio::time::sleep(wait) => (),
            }
            let rendering_revision = match host.lock() {
                Ok(state) => state.view.revision,
                Err(error) => {
                    eprintln!("桌面提醒停止：{error}");
                    break;
                }
            };
            if let Err(error) = render(&app, &mut emitted).await {
                eprintln!("桌面提醒显示失败，应用内提醒仍保留：{error}");
                if let Err(error) = host.dismiss(Some(rendering_revision)) {
                    eprintln!("桌面提醒收起失败：{error}");
                }
            }
        }
        if let Some(window) = app.get_webview_window(WINDOW) {
            if let Err(error) = set_notice_visible(&window, false).await {
                eprintln!("桌面提醒收起失败：{error}");
            }
        }
    });
}

async fn render(app: &AppHandle, emitted: &mut Option<u64>) -> Result<(), String> {
    let host = app.state::<CompletionHost>();
    host.lock()?.expire(Instant::now());
    let needs_window = host.lock()?.view.notice.is_some();
    if needs_window && app.get_webview_window(WINDOW).is_none() {
        let handle = app.clone();
        // WebView2 在同步命令或窗口事件内建窗可能死锁，交由独立工作线程创建。
        tauri::async_runtime::spawn_blocking(move || {
            WebviewWindowBuilder::new(&handle, WINDOW, tauri::WebviewUrl::App("index.html".into()))
                .title("GitHubSP · 下载完成")
                .inner_size(380.0, 170.0)
                .decorations(false)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        *emitted = None;
    }
    let Some(window) = app.get_webview_window(WINDOW) else {
        return Ok(());
    };
    if !is_background(app)? {
        host.lock()?.dismiss(None);
    }
    let (view, presented) = {
        let state = host.lock()?;
        (state.view.clone(), state.presented)
    };
    if *emitted != Some(view.revision) {
        app.emit_to(WINDOW, EVENT, &view)
            .map_err(|e| e.to_string())?;
        *emitted = Some(view.revision);
    }
    if view.notice.is_none() {
        set_notice_visible(&window, false).await?;
    } else if presented == Some(view.revision) {
        let main = app.get_webview_window("main").ok_or("主窗口不可用")?;
        let current = match main.current_monitor() {
            Ok(monitor) => monitor,
            Err(error) => {
                eprintln!("读取主窗口显示器失败，尝试主屏：{error}");
                None
            }
        };
        let monitor = match current {
            Some(monitor) => monitor,
            None => main
                .primary_monitor()
                .map_err(|e| e.to_string())?
                .ok_or("无法确定提醒显示位置")?,
        };
        let area = monitor.work_area();
        let (position, size) = placement(area.position, area.size, monitor.scale_factor());
        window
            .set_position(position)
            .and_then(|()| window.set_size(size))
            .map_err(|e| e.to_string())?;
        set_notice_visible(&window, true).await?;
    }
    Ok(())
}

async fn set_notice_visible(window: &tauri::WebviewWindow, visible: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let (send, receive) = tokio::sync::oneshot::channel();
        let notice = window.clone();
        window
            .run_on_main_thread(move || {
                let result = (|| {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        SetWindowPos, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                        SWP_NOZORDER, SWP_SHOWWINDOW,
                    };
                    let hwnd = notice.hwnd().map_err(|e| e.to_string())?.0;
                    // Tauri 的 show 会激活复用窗口。提醒的显示和隐藏统一走原生入口，避免混用可见性缓存。
                    // 不设置永久 NOACTIVATE 样式，保留用户点击后的键盘操作能力。
                    let flags = SWP_NOACTIVATE
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOZORDER
                        | if visible {
                            SWP_SHOWWINDOW
                        } else {
                            SWP_HIDEWINDOW
                        };
                    if unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, flags) } == 0 {
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                    Ok(())
                })();
                let _ = send.send(result);
            })
            .map_err(|e| e.to_string())?;
        receive
            .await
            .map_err(|_| "提醒窗口显示操作已中止".to_string())?
    }
    #[cfg(not(windows))]
    {
        if visible {
            window.show()
        } else {
            window.hide()
        }
        .map_err(|e| e.to_string())
    }
}

fn placement(
    origin: PhysicalPosition<i32>,
    area: PhysicalSize<u32>,
    scale: f64,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let margin = (16.0 * scale).round() as u32;
    let width = ((380.0 * scale).round() as u32).min(area.width.saturating_sub(margin * 2).max(1));
    let height =
        ((170.0 * scale).round() as u32).min(area.height.saturating_sub(margin * 2).max(1));
    let x = origin.x as i64 + area.width.saturating_sub(width + margin) as i64;
    let y = origin.y as i64 + area.height.saturating_sub(height + margin) as i64;
    (
        PhysicalPosition::new(
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        ),
        PhysicalSize::new(width, height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notice, Settings, Task, TaskDetails, TaskStatus, Verification};

    fn snapshot(revision: u64, ids: &[&str]) -> Snapshot {
        Snapshot {
            revision,
            last_directory: None,
            error: None,
            settings: Settings::default(),
            queue_revision: 0,
            diagnostics: vec![],
            diagnostic_context: None,
            diagnosing: false,
            favorites: vec![],
            tasks: ids
                .iter()
                .map(|id| Task {
                    id: id.to_string(),
                    url: String::new(),
                    filename: format!("{id}.zip"),
                    directory: Default::default(),
                    status: TaskStatus::Completed,
                    downloaded: 1,
                    total: Some(1),
                    speed: 0.0,
                    eta: None,
                    route: None,
                    verification: Verification::Unverified,
                    error: None,
                    final_path: None,
                    created_at: 0,
                    revision: 1,
                    details: TaskDetails {
                        elapsed_ms: Some(1234),
                        ..Default::default()
                    },
                })
                .collect(),
            notices: ids
                .iter()
                .map(|id| Notice {
                    id: id.to_string(),
                    kind: NoticeKind::DownloadCompleted,
                    task_id: Some(id.to_string()),
                    message: String::new(),
                    created_at: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn only_background_completions_show_and_foreground_events_never_replay() {
        let mut state = NoticeState::default();
        assert!(!state.ingest(&snapshot(1, &["front"]), false));
        assert!(!state.ingest(&snapshot(2, &["front"]), true));
        assert!(state.ingest(&snapshot(3, &["front", "back"]), true));
        assert_eq!(state.view.notice.as_ref().unwrap().count, 1);
        assert!(!state.ingest(&snapshot(3, &["front", "back"]), true));
        assert!(!state.ingest(&snapshot(2, &["old"]), true));
        let mut failed = snapshot(4, &["front", "back", "failure"]);
        failed.notices[2].kind = NoticeKind::DownloadFailed;
        assert!(!state.ingest(&failed, true));
        assert!(state.ingest(&snapshot(5, &["front", "back"]), false));
        assert!(state.view.notice.is_none());
        assert!(!state.ingest(&snapshot(6, &["front", "back"]), true));
    }

    #[test]
    fn switch_off_dismisses_and_does_not_replay_after_enabling() {
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        let mut disabled = snapshot(2, &["one", "two"]);
        disabled.settings.background_completion_notice = false;
        assert!(state.ingest(&disabled, true));
        assert!(state.view.notice.is_none());
        assert!(!state.ingest(&snapshot(3, &["one", "two"]), true));
    }

    #[test]
    fn merge_restarts_timeout_and_stale_callbacks_cannot_close_new_content() {
        let now = Instant::now();
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        let old = state.view.revision;
        state.presented(old, now);
        assert!(!state.expire(now + Duration::from_secs(7)));
        state.ingest(&snapshot(2, &["one", "two"]), true);
        assert_eq!(state.view.notice.as_ref().unwrap().count, 2);
        assert!(!state.dismiss(Some(old)));
        state.presented(old, now + Duration::from_secs(7));
        assert!(state.deadline.is_none());
        state.presented(state.view.revision, now + Duration::from_secs(7));
        assert!(!state.expire(now + Duration::from_secs(8)));
        assert!(state.expire(now + Duration::from_secs(15)));
        assert!(!state.ingest(&snapshot(3, &["one", "two"]), true));
    }

    #[test]
    fn hover_and_keyboard_focus_independently_pause_remaining_time() {
        let now = Instant::now();
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        state.presented(state.view.revision, now);
        state.interaction(Some(true), None, now + Duration::from_secs(2));
        state.interaction(None, Some(true), now + Duration::from_secs(3));
        state.interaction(Some(false), None, now + Duration::from_secs(5));
        assert!(!state.expire(now + Duration::from_secs(100)));
        state.interaction(None, Some(false), now + Duration::from_secs(100));
        assert!(!state.expire(now + Duration::from_secs(105)));
        assert!(state.expire(now + Duration::from_secs(106)));
    }

    #[test]
    fn failed_presentation_is_discarded_once_and_later_completions_can_show() {
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        assert!(state.dismiss(None));
        assert!(!state.ingest(&snapshot(2, &["one"]), true));
        assert!(state.ingest(&snapshot(3, &["one", "two"]), true));
        assert_eq!(state.view.notice.as_ref().unwrap().count, 1);
        assert_eq!(state.view.notice.as_ref().unwrap().filename, "two.zip");
    }

    #[test]
    fn placement_stays_in_work_area_for_scaling_and_negative_monitor_origins() {
        for scale in [1.0, 1.5, 2.0] {
            let (position, size) = placement(
                PhysicalPosition::new(-1920, -1080),
                PhysicalSize::new(1920, 1040),
                scale,
            );
            assert_eq!(size.width, (380.0 * scale) as u32);
            assert_eq!(position.x + size.width as i32, -(16.0 * scale) as i32);
            assert_eq!(position.y + size.height as i32, -40 - (16.0 * scale) as i32);
        }
        let (position, size) = placement(
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(200, 100),
            2.0,
        );
        assert!(position.x >= 0 && position.y >= 0);
        assert!(position.x as u32 + size.width <= 200);
        assert!(position.y as u32 + size.height <= 100);
    }

    #[test]
    fn notice_window_cannot_call_task_directory_or_settings_commands() {
        for command in [
            "create_task",
            "task_action",
            "create_directory",
            "save_settings",
            "list_tasks",
        ] {
            assert!(!allowed_command(WINDOW, command));
            assert!(allowed_command("main", command));
        }
        assert!(allowed_command(WINDOW, "read_download_notice"));
        assert!(!allowed_command("main", "read_download_notice"));
        assert!(!allowed_command("unknown", "open_download_notice"));
    }
}

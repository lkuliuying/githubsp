use githubsp_lib::model::Snapshot;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};

static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

pub fn init(data_directory: &Path) -> PathBuf {
    let directory = data_directory.join("logs");
    let path = directory.join("githubsp.log");
    let file = std::fs::create_dir_all(&directory).ok().and_then(|()| {
        if path.metadata().map(|meta| meta.len()).unwrap_or(0) > MAX_LOG_BYTES {
            let rotated = directory.join("githubsp.previous.log");
            let _ = std::fs::rename(&path, rotated);
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
    });
    let _ = DIRECTORY.set(directory);
    let _ = LOG.set(Mutex::new(file));
    write("info", "应用启动");
    path
}

pub fn path() -> Option<PathBuf> {
    DIRECTORY
        .get()
        .map(|directory| directory.join("githubsp.log"))
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
}

pub fn write(level: &str, message: &str) {
    if !ENABLED.load(Ordering::Acquire) {
        return;
    }
    let Some(log) = LOG.get() else {
        return;
    };
    let Ok(mut log) = log.lock() else {
        return;
    };
    let Some(file) = log.as_mut() else {
        return;
    };
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) >= MAX_LOG_BYTES {
        log.take();
        if let Some(directory) = DIRECTORY.get() {
            let path = directory.join("githubsp.log");
            let _ = std::fs::rename(&path, directory.join("githubsp.previous.log"));
            *log = OpenOptions::new().create(true).append(true).open(path).ok();
        }
    }
    let Some(file) = log.as_mut() else {
        return;
    };
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let _ = writeln!(file, "[{seconds}] {level} {message}");
    let _ = file.flush();
}

pub fn transitions(previous: &Snapshot, next: &Snapshot) {
    use githubsp_lib::model::TaskStatus;

    if !previous.checking.active && next.checking.active {
        write(
            "info",
            &format!(
                "开始检查收藏：{} 项，间隔 {}ms",
                next.checking.total, next.settings.watch_interval_ms
            ),
        );
    } else if previous.checking.active && !next.checking.active {
        write(
            "info",
            &format!(
                "收藏检查结束：{}/{}，取消={}",
                next.checking.done, next.checking.total, next.checking.cancelled
            ),
        );
    }
    for task in &next.tasks {
        let before = previous.tasks.iter().find(|old| old.id == task.id);
        if before.is_some_and(|old| old.status != task.status)
            && matches!(
                task.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        {
            write(
                if task.status == TaskStatus::Failed {
                    "warn"
                } else {
                    "info"
                },
                &format!(
                    "任务 {} → {:?}{}",
                    task.filename,
                    task.status,
                    task.error
                        .as_ref()
                        .map(|error| format!("：{error}"))
                        .unwrap_or_default()
                ),
            );
        }
    }
    if previous.api_status.error != next.api_status.error {
        write(
            "warn",
            &format!(
                "GitHub API 状态变化：来源={:?} 剩余={:?} 错误={:?}",
                next.api_status.source, next.api_status.remaining, next.api_status.error
            ),
        );
    }
    let unread = |snapshot: &Snapshot| {
        snapshot
            .favorites
            .iter()
            .filter(|favorite| snapshot.watch_state.is_unread(favorite))
            .count()
    };
    let (before, after) = (unread(previous), unread(next));
    if before != after {
        write("info", &format!("收藏未读更新：{before} → {after}"));
    }
}

pub fn diagnostics(snapshot: &Snapshot, data_directory: &Path) -> String {
    let mut text = String::new();
    text.push_str(&format!("GitHubSP v{}\n", env!("CARGO_PKG_VERSION")));
    text.push_str(&format!(
        "系统：{} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    text.push_str(&format!("数据目录：{}\n", data_directory.display()));
    if let Some(path) = path() {
        text.push_str(&format!("日志文件：{}\n", path.display()));
    }
    text.push_str(&format!(
        "收藏检查：{} 项，自动下载={}，提醒={}\n",
        snapshot.favorites.len(),
        snapshot.settings.watch_auto_download,
        snapshot.settings.watch_notifications
    ));
    text.push_str(&format!(
        "检查间隔：{} 毫秒，上次检查：{}\n",
        snapshot.settings.watch_interval_ms,
        snapshot
            .checking
            .finished_at
            .map(|at| at.to_string())
            .unwrap_or_else(|| "尚未检查".into())
    ));
    text.push_str(&format!(
        "GitHub API：剩余 {:?}/{:?}，来源={:?}，错误={:?}\n",
        snapshot.api_status.remaining,
        snapshot.api_status.limit,
        snapshot.api_status.source,
        snapshot.api_status.error
    ));
    if let Some(error) = &snapshot.error {
        text.push_str(&format!("当前错误：{error}\n"));
    }
    let failed: Vec<_> = snapshot
        .favorites
        .iter()
        .filter_map(|favorite| {
            favorite
                .error
                .as_ref()
                .map(|error| format!("  {}：{error}", favorite.repository))
        })
        .collect();
    if !failed.is_empty() {
        text.push_str("检查失败项：\n");
        text.push_str(&failed.join("\n"));
        text.push('\n');
    }
    text
}

pub fn copy(text: &str) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::HWND,
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        },
    };
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    unsafe {
        if OpenClipboard(std::ptr::null_mut() as HWND) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if EmptyClipboard() == 0 {
            CloseClipboard();
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if handle.is_null() {
            CloseClipboard();
            return Err("无法分配剪贴板内存".into());
        }
        let target = GlobalLock(handle);
        if target.is_null() {
            CloseClipboard();
            return Err("无法锁定剪贴板内存".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), target.cast::<u16>(), wide.len());
        GlobalUnlock(handle);
        if SetClipboardData(13u32, handle).is_null() {
            CloseClipboard();
            return Err(std::io::Error::last_os_error().to_string());
        }
        CloseClipboard();
    }
    Ok(())
}

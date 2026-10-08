use std::{path::Path, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::{FILETIME, SYSTEMTIME},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime},
    },
};

pub fn date(value: Option<u64>) -> String {
    let Some(value) = value.filter(|v| *v > 0) else {
        return "未知".into();
    };
    let Some(ticks) = value
        .checked_mul(10_000)
        .and_then(|v| v.checked_add(116_444_736_000_000_000))
    else {
        return "未知".into();
    };
    let file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut time = SYSTEMTIME::default();
    // 使用 Windows 当前时区，与原界面的本地日期显示保持一致。
    if unsafe { FileTimeToSystemTime(&file, &mut utc) } == 0
        || unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut time) } == 0
    {
        return "未知".into();
    }
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

pub fn copy(text: &str, owner: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    if text.contains('\0') {
        return Err("链接包含无效字符".into());
    }
    let content: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    unsafe {
        if OpenClipboard(owner) == 0 {
            return Err("无法访问剪贴板，请稍后重试".into());
        }
        let _clipboard = Clipboard;
        let memory = GlobalAlloc(GMEM_MOVEABLE, content.len() * 2);
        if memory.is_null() {
            return Err("剪贴板分配内存失败".into());
        }
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            windows_sys::Win32::Foundation::GlobalFree(memory);
            return Err("无法写入剪贴板".into());
        }
        std::ptr::copy_nonoverlapping(content.as_ptr(), pointer.cast::<u16>(), content.len());
        GlobalUnlock(memory);
        if EmptyClipboard() == 0 || SetClipboardData(13, memory).is_null() {
            windows_sys::Win32::Foundation::GlobalFree(memory);
            return Err("复制链接失败，请重试".into());
        }
        // 成功后内存归系统剪贴板管理，不再释放。
    }
    Ok(())
}

pub fn open_directory(path: &Path) -> Result<(), String> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("无法访问保存目录：{e}"))?;
    if !path.is_dir() {
        return Err("保存位置不是目录".into());
    }
    shell_open(path.as_os_str())
}

pub fn open_link(link: &str, release_only: bool) -> Result<(), String> {
    let url = githubsp_lib::updates::validate_update_link(link)?;
    if release_only
        && !matches!(
            githubsp_lib::source::parse_resource(url.as_str()),
            Ok(githubsp_lib::source::Resource::Release(_, _))
        )
    {
        return Err("只允许打开 GitHub 官方版本页面".into());
    }
    shell_open(std::ffi::OsStr::new(url.as_str()))
}

fn shell_open(target: &std::ffi::OsStr) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let target: Vec<u16> = target.encode_wide().chain(Some(0)).collect();
    let open: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            null_mut(),
            open.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    } as isize;
    if result <= 32 {
        Err(format!("无法打开目标，Windows 错误代码 {result}"))
    } else {
        Ok(())
    }
}

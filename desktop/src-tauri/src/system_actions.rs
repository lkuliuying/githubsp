use std::{
    path::Path,
    ptr::null_mut,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::Win32::System::{
    DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
    },
    Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
};

const CF_UNICODETEXT: u32 = 13;

/// 本应用最近一次写入剪贴板后的序号；识别链接时跳过自己复制的内容。
static OWN_SEQUENCE: AtomicU32 = AtomicU32::new(0);

struct OpenedClipboard;
impl Drop for OpenedClipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

/// 剪贴板内容每次变化都会递增；不打开剪贴板，开销很小。
pub fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// 当前剪贴板内容是否由本应用写入。
pub fn clipboard_is_own(sequence: u32) -> bool {
    sequence != 0 && OWN_SEQUENCE.load(Ordering::Relaxed) == sequence
}

/// 读取剪贴板文字；超过 `limit` 个 UTF-16 单元、没有文字或被占用时返回 `None`。
pub fn read_text(limit: usize) -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 || OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let _clipboard = OpenedClipboard;
        let memory = GetClipboardData(CF_UNICODETEXT);
        if memory.is_null() {
            return None;
        }
        let units = GlobalSize(memory) / 2;
        let pointer = GlobalLock(memory).cast::<u16>();
        if pointer.is_null() {
            return None;
        }
        let data = std::slice::from_raw_parts(pointer, units);
        let length = data.iter().position(|&c| c == 0).unwrap_or(units);
        let text = (length <= limit).then(|| String::from_utf16_lossy(&data[..length]));
        GlobalUnlock(memory);
        text
    }
}

pub fn copy(text: &str, owner: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    write_text(text, owner)?;
    // 关闭剪贴板后再记录序号，确保拿到的是这次写入后的值。
    OWN_SEQUENCE.store(clipboard_sequence(), Ordering::Relaxed);
    Ok(())
}

fn write_text(text: &str, owner: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    if text.contains('\0') {
        return Err("链接包含无效字符".into());
    }
    let content: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        if OpenClipboard(owner) == 0 {
            return Err("无法访问剪贴板，请稍后重试".into());
        }
        let _clipboard = OpenedClipboard;
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
        if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT, memory).is_null() {
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

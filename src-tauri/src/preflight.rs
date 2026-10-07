use crate::error::{DownloadError, ErrorKind, Result};
use serde::Serialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    pub directory: PathBuf,
    pub available: Option<u64>,
    pub required: Option<u64>,
    pub warning: Option<String>,
}

pub async fn check_async(
    directory: PathBuf,
    size: Option<u64>,
    downloaded: u64,
) -> Result<Preflight> {
    tokio::task::spawn_blocking(move || check(&directory, size, downloaded))
        .await
        .map_err(|error| DownloadError::new(ErrorKind::Io, format!("目录预检查异常：{error}")))?
}

pub fn check(directory: &Path, size: Option<u64>, downloaded: u64) -> Result<Preflight> {
    let directory = dunce::canonicalize(directory).map_err(|_| {
        DownloadError::new(
            ErrorKind::Permission,
            "保存目录不存在或无法访问，请另选目录",
        )
    })?;
    if !directory.is_dir() {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "保存位置不是目录",
        ));
    }
    let mut probe = tempfile::Builder::new()
        .prefix(".githubsp-write-check-")
        .tempfile_in(&directory)
        .map_err(|_| DownloadError::new(ErrorKind::Permission, "保存目录不可写，请另选目录"))?;
    probe.write_all(b"GitHubSP")?;
    probe.as_file().sync_all()?;
    let available = free_space(&directory)?;
    let required = size.map(|size| size.saturating_sub(downloaded).saturating_add(size));
    ensure_space(available, required)?;
    Ok(Preflight {
        directory,
        available,
        required,
        warning: if size.is_none() {
            Some("附件大小未知，无法完整估算分片与合并所需空间".into())
        } else {
            None
        },
    })
}

pub fn ensure_space(available: Option<u64>, required: Option<u64>) -> Result<()> {
    if let (Some(available), Some(required)) = (available, required) {
        if available < required {
            return Err(DownloadError::new(
                ErrorKind::DiskSpace,
                format!("磁盘空间不足：分片和合并需要 {required} 字节，当前可用 {available} 字节"),
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn free_space(directory: &Path) -> Result<Option<u64>> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0;
    // 路径以空字符结尾，输出指针在调用期间有效；采用当前用户实际可用额度。
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        return Err(DownloadError::new(
            ErrorKind::Permission,
            "无法检查目标目录的剩余空间",
        ));
    }
    Ok(Some(available))
}

#[cfg(not(windows))]
fn free_space(_directory: &Path) -> Result<Option<u64>> {
    Ok(None)
}

use crate::error::{DownloadError, ErrorKind, Result};
use serde::Serialize;
use std::{
    io::{Error as IoError, ErrorKind as IoErrorKind, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DirectoryState {
    Existing,
    Missing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryInspection {
    pub directory: PathBuf,
    pub state: DirectoryState,
}

fn directory_error(error: IoError) -> DownloadError {
    match error.kind() {
        IoErrorKind::PermissionDenied => {
            DownloadError::new(ErrorKind::Permission, "无法访问保存目录，请检查目录权限")
        }
        IoErrorKind::NotADirectory => {
            DownloadError::new(ErrorKind::InvalidInput, "保存位置或其父路径不是目录")
        }
        IoErrorKind::InvalidInput | IoErrorKind::InvalidFilename => {
            DownloadError::new(ErrorKind::InvalidInput, "保存目录路径无效，请检查输入")
        }
        _ => DownloadError::new(ErrorKind::Io, format!("无法访问保存目录：{error}")),
    }
}

pub fn inspect_directory(directory: &Path) -> Result<DirectoryInspection> {
    if directory.as_os_str().is_empty()
        || directory
            .to_str()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "请输入保存目录",
        ));
    }
    let absolute = std::path::absolute(directory).map_err(directory_error)?;
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    let mut rechecked = false;
    loop {
        match std::fs::metadata(ancestor) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(DownloadError::new(
                        ErrorKind::InvalidInput,
                        "保存位置或其父路径不是目录",
                    ));
                }
                let mut directory = dunce::canonicalize(ancestor).map_err(directory_error)?;
                let state = if missing.is_empty() {
                    DirectoryState::Existing
                } else {
                    DirectoryState::Missing
                };
                for component in missing.iter().rev() {
                    directory.push(component);
                }
                return Ok(DirectoryInspection { directory, state });
            }
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                // 失效链接与不可访问的磁盘不能当成可递归创建的普通目录。
                match std::fs::symlink_metadata(ancestor) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(DownloadError::new(
                            ErrorKind::InvalidInput,
                            "保存路径包含失效链接，请选择有效目录",
                        ));
                    }
                    // 检查之间其他进程可能刚创建目录或文件，重新读取其实际类型。
                    Ok(_) if !rechecked => {
                        rechecked = true;
                        continue;
                    }
                    Ok(_) => {
                        return Err(DownloadError::new(
                            ErrorKind::Io,
                            "保存目录状态发生变化，请重试",
                        ));
                    }
                    Err(error) if error.kind() == IoErrorKind::NotFound => {}
                    Err(error) => return Err(directory_error(error)),
                }
                let component = ancestor.file_name().ok_or_else(|| {
                    DownloadError::new(
                        ErrorKind::Permission,
                        "目标磁盘或共享目录不存在或无法访问，请另选目录",
                    )
                })?;
                missing.push(component.to_os_string());
                ancestor = ancestor.parent().ok_or_else(|| {
                    DownloadError::new(ErrorKind::InvalidInput, "保存目录路径无效")
                })?;
            }
            Err(error) => return Err(directory_error(error)),
        }
    }
}

pub async fn inspect_directory_async(directory: PathBuf) -> Result<DirectoryInspection> {
    tokio::task::spawn_blocking(move || inspect_directory(&directory))
        .await
        .map_err(|error| DownloadError::new(ErrorKind::Io, format!("目录检查异常：{error}")))?
}

pub fn create_directory(directory: &Path) -> Result<PathBuf> {
    let inspected = inspect_directory(directory)?;
    if inspected.state == DirectoryState::Missing {
        std::fs::create_dir_all(&inspected.directory).map_err(|error| {
            let mut failure = DownloadError::from(error);
            failure.message = format!("创建目录失败：{}", failure.message);
            failure
        })?;
    }
    // 创建失败或后续检查失败时保留已创建的目录，避免误删其他进程写入的内容。
    Ok(check(&inspected.directory, None, 0)?.directory)
}

pub async fn create_directory_async(directory: PathBuf) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || create_directory(&directory))
        .await
        .map_err(|error| DownloadError::new(ErrorKind::Io, format!("创建目录异常：{error}")))?
}

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

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn inspect_directory_is_read_only_and_resolves_nested_unicode_paths() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("中文 空格").join("版本");
        let inspected = inspect_directory(&nested).unwrap();
        assert_eq!(inspected.state, DirectoryState::Missing);
        assert_eq!(
            inspected.directory,
            dunce::canonicalize(root.path())
                .unwrap()
                .join("中文 空格")
                .join("版本")
        );
        assert!(!root.path().join("中文 空格").exists());
        assert_eq!(serde_json::to_value(inspected).unwrap()["state"], "missing");
        assert_eq!(
            inspect_directory(root.path()).unwrap().state,
            DirectoryState::Existing
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn create_directory_is_recursive_writable_and_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("中文 空格").join("版本");
        let created = create_directory(&nested).unwrap();
        assert!(created.is_absolute() && created.is_dir());
        assert_eq!(create_directory(&nested).unwrap(), created);
        assert_eq!(
            inspect_directory(&nested).unwrap().state,
            DirectoryState::Existing
        );
        assert_eq!(std::fs::read_dir(&created).unwrap().count(), 0);
        let file = created.join("保留.txt");
        std::fs::write(&file, b"keep").unwrap();
        create_directory(&nested).unwrap();
        assert_eq!(std::fs::read(file).unwrap(), b"keep");
    }

    #[test]
    fn inspect_directory_rejects_empty_paths_and_file_ancestors() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        std::fs::write(&file, b"keep").unwrap();
        for path in [
            PathBuf::new(),
            PathBuf::from("   "),
            PathBuf::from("invalid\0path"),
            file.clone(),
            file.join("child"),
        ] {
            assert_eq!(
                inspect_directory(&path).unwrap_err().kind,
                ErrorKind::InvalidInput
            );
            assert!(create_directory(&path).is_err());
        }
        assert_eq!(std::fs::read(file).unwrap(), b"keep");
    }

    #[test]
    fn directory_errors_preserve_permission_and_invalid_input_categories() {
        for (io_kind, expected) in [
            (IoErrorKind::PermissionDenied, ErrorKind::Permission),
            (IoErrorKind::NotADirectory, ErrorKind::InvalidInput),
            (IoErrorKind::InvalidInput, ErrorKind::InvalidInput),
            (IoErrorKind::InvalidFilename, ErrorKind::InvalidInput),
            (IoErrorKind::Other, ErrorKind::Io),
        ] {
            assert_eq!(directory_error(IoError::from(io_kind)).kind, expected);
        }
    }

    #[test]
    fn create_directory_rechecks_a_path_replaced_by_a_file() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("missing");
        assert_eq!(
            inspect_directory(&nested).unwrap().state,
            DirectoryState::Missing
        );
        std::fs::write(&nested, b"keep").unwrap();
        assert_eq!(
            create_directory(&nested).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
        assert_eq!(std::fs::read(nested).unwrap(), b"keep");
    }

    #[test]
    fn concurrent_directory_creation_is_safe() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("a").join("b");
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        create_directory(&nested).unwrap()
                    })
                })
                .collect();
            for handle in handles {
                assert_eq!(
                    handle.join().unwrap(),
                    dunce::canonicalize(&nested).unwrap()
                );
            }
        });
        assert_eq!(std::fs::read_dir(nested).unwrap().count(), 0);
    }

    #[test]
    fn ordinary_preflight_does_not_create_missing_directories() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("missing");
        assert!(check(&nested, None, 0).is_err());
        assert!(!nested.exists());
    }

    #[cfg(windows)]
    #[test]
    fn inspect_directory_rejects_an_unavailable_drive() {
        let drive = (b'A'..=b'Z')
            .map(|letter| PathBuf::from(format!("{}:\\", char::from(letter))))
            .find(|path| {
                std::fs::metadata(path).is_err_and(|error| error.kind() == IoErrorKind::NotFound)
            })
            .expect("测试需要一个未挂载的盘符");
        assert!(inspect_directory(&drive.join("missing")).is_err());
    }

    #[tokio::test]
    async fn directory_commands_use_the_same_filesystem_checks() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("异步 创建");
        assert_eq!(
            inspect_directory_async(nested.clone()).await.unwrap().state,
            DirectoryState::Missing
        );
        let created = create_directory_async(nested.clone()).await.unwrap();
        assert_eq!(created, dunce::canonicalize(nested).unwrap());
    }
}

use crate::error::{DownloadError, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub name: PathBuf,
    pub size: u64,
    pub sha256: String,
}

pub fn error(message: impl Into<String>) -> DownloadError {
    DownloadError::new(ErrorKind::Storage, message)
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

pub fn directory(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(error("数据目录必须为绝对路径"));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err(error("数据位置不是普通目录，不能使用链接或重解析点"));
    }
    Ok(dunce::canonicalize(path)?)
}

pub fn safe_path(root: &Path, name: &Path) -> Result<PathBuf> {
    if name.as_os_str().is_empty()
        || name
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err(error("迁移文件清单包含无效路径"));
    }
    let mut path = root.to_owned();
    for part in name.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if linked(&metadata) => return Err(error("迁移路径包含链接或重解析点")),
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}

pub fn fingerprint(file: &mut File) -> Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
        size += count as u64;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

fn read_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || linked(&metadata) {
        return Err(error("迁移数据不是普通文件"));
    }
    Ok(file)
}

pub fn entry(root: &Path, name: &Path) -> Result<Entry> {
    let mut file = read_file(&safe_path(root, name)?)?;
    let (size, sha256) = fingerprint(&mut file)?;
    Ok(Entry {
        name: name.into(),
        size,
        sha256,
    })
}

pub fn matches(root: &Path, expected: &Entry) -> Result<bool> {
    let actual = entry(root, &expected.name)?;
    Ok(actual.size == expected.size && actual.sha256 == expected.sha256)
}

pub fn inventory(root: &Path) -> Result<Vec<Entry>> {
    managed_names(root)?
        .iter()
        .map(|name| entry(root, name))
        .collect()
}

pub fn estimated_bytes(root: &Path) -> Result<u64> {
    managed_names(root)?.iter().try_fold(0_u64, |total, name| {
        let metadata = fs::symlink_metadata(safe_path(root, name)?)?;
        if !metadata.is_file() || linked(&metadata) {
            return Err(error("应用数据不是普通文件"));
        }
        total
            .checked_add(metadata.len())
            .ok_or_else(|| error("迁移数据大小超出支持范围"))
    })
}

fn managed_names(root: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    for name in ["tasks.sqlite3", "tasks.sqlite3-wal", "tasks.sqlite3-shm"] {
        match fs::symlink_metadata(root.join(name)) {
            Ok(_) => entries.push(PathBuf::from(name)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    let backups = safe_path(root, Path::new("backups"))?;
    match fs::read_dir(&backups) {
        Ok(children) => {
            for child in children {
                let child = child?;
                // 只管理既有 SQLite 备份，未知文件和子目录不参与删除。
                if child
                    .path()
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("sqlite3"))
                {
                    entries.push(Path::new("backups").join(child.file_name()));
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    entries.sort();
    Ok(entries)
}

pub fn copy(source: &Path, target: &Path, expected: &Entry) -> Result<()> {
    let source = safe_path(source, &expected.name)?;
    let destination = safe_path(target, &expected.name)?;
    let parent = destination.parent().ok_or_else(|| error("迁移目标无效"))?;
    fs::create_dir_all(parent)?;
    let _parent = DirectoryGuard::acquire(parent)?;
    let mut input = read_file(&source)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut input, &mut temporary)?;
    temporary.as_file().sync_all()?;
    let mut check = File::open(temporary.path())?;
    let (size, sha256) = fingerprint(&mut check)?;
    if size != expected.size || sha256 != expected.sha256 {
        return Err(error("迁移期间备份内容发生变化，原数据已保留"));
    }
    drop(check);
    publish(temporary, &destination, false)?;
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.is_file() || linked(&metadata) {
            return Err(error("启动配置位置不是普通文件"));
        }
    }
    let parent = path.parent().ok_or_else(|| error("启动配置路径无效"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    publish(file, path, true)
}

fn publish(file: tempfile::NamedTempFile, path: &Path, replace: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, SetFileAttributesW, FILE_ATTRIBUTE_NORMAL, MOVEFILE_REPLACE_EXISTING,
            MOVEFILE_WRITE_THROUGH,
        };
        let temporary = file.into_temp_path();
        let wide = |path: &Path| {
            path.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let source = wide(&temporary);
        let destination = wide(path);
        // 先同步文件内容，再等待替换落盘；清理来源前必须提交新位置。
        let flags = MOVEFILE_WRITE_THROUGH
            | if replace {
                MOVEFILE_REPLACE_EXISTING
            } else {
                0
            };
        if unsafe { SetFileAttributesW(source.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0
            || unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(windows))]
    {
        if replace {
            file.persist(path)
        } else {
            file.persist_noclobber(path)
        }
        .map_err(|e| DownloadError::from(e.error))?;
    }
    Ok(())
}

pub fn read_config(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(_) => (),
    }
    let file = read_file(path)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(error("数据位置配置过大，请保留配置文件并检查"));
    }
    Ok(Some(bytes))
}

pub fn delete_matching(root: &Path, expected: &Entry) -> Result<()> {
    let path = safe_path(root, &expected.name)?;
    let parent = path.parent().ok_or_else(|| error("来源文件目录无效"))?;
    if !parent.try_exists()? {
        return Ok(());
    }
    let _parent = DirectoryGuard::acquire(parent)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // 禁止并发写入和替换；校验与删除针对同一个文件句柄。
        options
            .access_mode(0x80000000 | 0x00010000)
            .share_mode(1)
            .custom_flags(0x00200000);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || linked(&metadata) {
        return Err(error("来源文件类型变化，已保留该文件"));
    }
    let (size, sha256) = fingerprint(&mut file)?;
    if size != expected.size || sha256 != expected.sha256 {
        return Err(error(format!("来源文件已变化，未删除：{}", path.display())));
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
        };
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        if unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle(),
                FileDispositionInfo,
                (&info as *const FILE_DISPOSITION_INFO).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(windows))]
    fs::remove_file(path)?;
    Ok(())
}

/// 防止正在校验的目录被其他进程重命名或替换。
pub struct DirectoryGuard {
    _file: File,
}

impl DirectoryGuard {
    pub fn acquire(path: &Path) -> Result<Self> {
        let expected = directory(path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(3).custom_flags(0x02000000 | 0x00200000);
        }
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || linked(&metadata) || directory(path)? != expected {
            return Err(error("目录在检查期间发生变化，已停止迁移"));
        }
        Ok(Self { _file: file })
    }
}

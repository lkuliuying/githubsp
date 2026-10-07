use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::{Checkpoint, DownloadedFile, Publication},
    source::validate_filename,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub path: PathBuf,
    pub directory: PathBuf,
    id: String,
}

impl Workspace {
    pub fn new(directory: &Path, id: &str) -> Result<Self> {
        uuid::Uuid::parse_str(id)
            .map_err(|_| DownloadError::new(ErrorKind::InvalidInput, "无效任务标识"))?;
        let directory = dunce::canonicalize(directory)?;
        if !directory.is_dir() {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "请选择已存在的文件夹",
            ));
        }
        let path = directory.join(format!(".githubsp-{id}"));
        let workspace = Self {
            path,
            directory,
            id: id.to_owned(),
        };
        match fs::create_dir(&workspace.path) {
            Ok(()) => {
                let mut owner = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(workspace.path.join("owner"))?;
                owner.write_all(id.as_bytes())?;
                owner.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                workspace.validate()?
            }
            Err(error) => return Err(error.into()),
        }
        Ok(workspace)
    }

    pub fn validate(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || dunce::canonicalize(&self.path)?.parent() != Some(self.directory.as_path())
        {
            return Err(DownloadError::new(
                ErrorKind::Io,
                "临时目录位置发生变化，已停止文件操作",
            ));
        }
        self.safe_file("owner")?;
        if fs::read(self.path.join("owner"))? != self.id.as_bytes() {
            return Err(DownloadError::new(
                ErrorKind::Io,
                "临时目录不属于此任务，已停止文件操作",
            ));
        }
        Ok(())
    }

    pub fn safe_file(&self, name: &str) -> Result<PathBuf> {
        let path = self.path.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
                DownloadError::new(ErrorKind::Io, "临时文件类型异常，已停止文件操作"),
            ),
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(error) => Err(error.into()),
        }
    }

    pub fn part(&self, index: usize) -> Result<PathBuf> {
        self.validate()?;
        self.safe_file(&format!("part-{index}.bin"))
    }

    pub fn clear_parts(&self) -> Result<()> {
        self.validate()?;
        for index in 0..2 {
            let path = self.part(index)?;
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        self.validate()?;
        self.clear_parts()?;
        for entry in fs::read_dir(&self.path)? {
            let name = entry?.file_name();
            if name
                .to_str()
                .is_some_and(|name| name.starts_with("assembly-") && name.ends_with(".tmp"))
            {
                fs::remove_file(self.safe_file(name.to_str().unwrap_or_default())?)?;
                continue;
            }
            if name != "owner" {
                return Err(DownloadError::new(
                    ErrorKind::Io,
                    "临时目录存在未知文件，已保留目录，请检查后手动清理",
                ));
            }
        }
        fs::remove_file(self.safe_file("owner")?)?;
        fs::remove_dir(&self.path)?;
        Ok(())
    }

    pub fn verify_parts(&self, checkpoint: &Checkpoint, token: &CancellationToken) -> Result<bool> {
        if checkpoint.parts.is_empty() || checkpoint.parts.len() > 2 {
            return Ok(false);
        }
        let mut next = 0;
        for (index, part) in checkpoint.parts.iter().enumerate() {
            if part.start != next
                || part.end.is_some_and(|end| end < part.start)
                || part.length().is_some_and(|length| part.downloaded > length)
            {
                return Ok(false);
            }
            next = part.end.and_then(|end| end.checked_add(1)).unwrap_or(0);
            if part.downloaded == 0 {
                continue;
            }
            let path = self.part(index)?;
            if !path.exists() || fs::metadata(&path)?.len() < part.downloaded {
                return Ok(false);
            }
            let mut file = File::open(path)?;
            let mut hasher = Sha256::new();
            let mut remaining = part.downloaded;
            let mut buffer = vec![0; 256 * 1024];
            while remaining > 0 {
                if token.is_cancelled() {
                    return Err(DownloadError::cancelled());
                }
                let size = remaining.min(buffer.len() as u64) as usize;
                file.read_exact(&mut buffer[..size])?;
                hasher.update(&buffer[..size]);
                remaining -= size as u64;
            }
            if format!("{:x}", hasher.finalize()) != part.sha256 {
                return Ok(false);
            }
        }
        Ok(checkpoint.total.is_none_or(|total| next == total))
    }

    pub fn assemble(
        &self,
        checkpoint: &Checkpoint,
        filename: &str,
        token: &CancellationToken,
        mut before_publish: impl FnMut(Publication) -> Result<()>,
    ) -> Result<DownloadedFile> {
        self.validate()?;
        validate_filename(filename)?;
        let mut output = tempfile::Builder::new()
            .prefix("assembly-")
            .suffix(".tmp")
            .tempfile_in(&self.path)?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = vec![0; 256 * 1024];
        for (index, part) in checkpoint.parts.iter().enumerate() {
            if part
                .length()
                .is_some_and(|length| length != part.downloaded)
            {
                return Err(DownloadError::new(
                    ErrorKind::Protocol,
                    "分片尚未完整，不能生成最终文件",
                ));
            }
            let mut file = File::open(self.part(index)?)?;
            if file.metadata()?.len() != part.downloaded {
                return Err(DownloadError::new(
                    ErrorKind::Integrity,
                    "临时文件长度发生变化",
                ));
            }
            let mut part_hasher = Sha256::new();
            loop {
                if token.is_cancelled() {
                    return Err(DownloadError::cancelled());
                }
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
                hasher.update(&buffer[..count]);
                part_hasher.update(&buffer[..count]);
                size += count as u64;
            }
            if format!("{:x}", part_hasher.finalize()) != part.sha256 {
                return Err(DownloadError::new(
                    ErrorKind::Integrity,
                    "临时文件内容发生变化，请重新下载",
                ));
            }
        }
        if checkpoint.total.is_some_and(|total| size != total) {
            return Err(DownloadError::new(
                ErrorKind::Integrity,
                "下载文件大小与预期不一致",
            ));
        }
        let sha256 = format!("{:x}", hasher.finalize());
        if checkpoint
            .expected_sha256
            .as_ref()
            .is_some_and(|expected| *expected != sha256)
        {
            return Err(DownloadError::new(
                ErrorKind::Integrity,
                "SHA-256 与 GitHub 官方摘要不一致，未生成最终文件",
            ));
        }
        output.as_file_mut().sync_all()?;
        for suffix in 0..10000 {
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            let name = collision_name(filename, suffix);
            let path = self.directory.join(name);
            if path.try_exists()? {
                continue;
            }
            // 先持久化发布意图，崩溃后可核对成品，避免重复生成同名副本。
            before_publish(Publication {
                path: path.clone(),
                size,
                sha256: sha256.clone(),
                verified: checkpoint.expected_sha256.is_some(),
            })?;
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            match output.persist_noclobber(&path) {
                Ok(_) => {
                    return Ok(DownloadedFile {
                        path,
                        size,
                        sha256,
                        verified: checkpoint.expected_sha256.is_some(),
                    })
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    output = error.file
                }
                Err(error) => return Err(error.error.into()),
            }
        }
        Err(DownloadError::new(
            ErrorKind::Io,
            "目录中存在过多同名文件，请选择其他目录",
        ))
    }

    pub fn recover_published(
        &self,
        publication: &Publication,
        token: &CancellationToken,
    ) -> Result<Option<DownloadedFile>> {
        if publication.path.parent() != Some(self.directory.as_path())
            || !publication.path.try_exists()?
        {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&publication.path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != publication.size
        {
            return Ok(None);
        }
        let mut file = File::open(&publication.path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 256 * 1024];
        loop {
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        if format!("{:x}", hasher.finalize()) != publication.sha256 {
            return Ok(None);
        }
        Ok(Some(DownloadedFile {
            path: publication.path.clone(),
            size: publication.size,
            sha256: publication.sha256.clone(),
            verified: publication.verified,
        }))
    }
}

pub fn collision_name(filename: &str, suffix: u32) -> String {
    if suffix == 0 {
        return filename.to_owned();
    }
    match filename.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({suffix}).{extension}"),
        _ => format!("{filename} ({suffix})"),
    }
}

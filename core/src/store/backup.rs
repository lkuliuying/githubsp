use crate::error::{DownloadError, ErrorKind, Result};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn export(
    source: &Connection,
    target: &Path,
    cancelled: &impl Fn() -> bool,
) -> Result<()> {
    if !target.is_absolute() {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "备份位置必须为绝对路径",
        ));
    }
    match std::fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "目标文件已存在，请选择新的备份文件名",
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let parent = target
        .parent()
        .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "备份目录无效"))?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut destination = Connection::open(temporary.path())?;
    // 只写入临时库，校验通过后才以不覆盖方式发布单文件备份。
    destination.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
    copy(source, &mut destination, cancelled)?;
    destination.execute_batch("PRAGMA journal_mode=DELETE;")?;
    super::Store::check_integrity(&destination)?;
    destination
        .close()
        .map_err(|(_, error)| DownloadError::from(error))?;
    temporary.as_file().sync_all()?;
    if cancelled() {
        return Err(DownloadError::cancelled());
    }
    temporary
        .persist_noclobber(target)
        .map_err(|error| DownloadError::from(error.error))?;
    Ok(())
}

fn copy(
    source: &Connection,
    destination: &mut Connection,
    cancelled: &impl Fn() -> bool,
) -> Result<()> {
    let backup = Backup::new(source, destination)?;
    let started = Instant::now();
    loop {
        if cancelled() {
            return Err(DownloadError::cancelled());
        }
        if started.elapsed() > Duration::from_secs(120) {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "备份等待超时，原数据库未修改，请稍后重试",
            ));
        }
        match backup.step(128)? {
            StepResult::Done => break,
            StepResult::More => std::thread::yield_now(),
            StepResult::Busy | StepResult::Locked => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                return Err(DownloadError::new(
                    ErrorKind::Storage,
                    "备份返回了不支持的状态",
                ))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_reports_destination_full_and_preserves_source() {
        let source = Connection::open_in_memory().unwrap();
        source
            .execute_batch(
                "CREATE TABLE payload(value BLOB); INSERT INTO payload VALUES(zeroblob(1048576));",
            )
            .unwrap();
        let mut destination = Connection::open_in_memory().unwrap();
        destination
            .pragma_update(None, "max_page_count", 2)
            .unwrap();
        let error = copy(&source, &mut destination, &|| false).unwrap_err();
        assert!(error.message.contains("full"), "{error}");
        assert_eq!(
            source
                .query_row("SELECT length(value) FROM payload", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1048576
        );
        super::super::Store::check_integrity(&source).unwrap();
    }
}

//! 数据位置配置与迁移事务；调用方负责停止服务并持有两端的进程互斥锁。

mod files;
#[cfg(test)]
mod tests;

use crate::{error::Result, preflight, store::Store};
pub use files::DirectoryGuard;
use files::{error, Entry};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Locations {
    pub default_directory: PathBuf,
    pub config_file: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    version: u32,
    pub active: Option<PathBuf>,
    pub pending: Option<Migration>,
    pub message: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            pending: None,
            message: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    Verified,
    Switched,
    Cleanup,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    id: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub keep_backup: bool,
    pub phase: Phase,
    files: Vec<Entry>,
    target_database: Option<Entry>,
}

pub fn overlap(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    let (a, b) = (
        PathBuf::from(a.to_string_lossy().to_lowercase()),
        PathBuf::from(b.to_string_lossy().to_lowercase()),
    );
    a.starts_with(&b) || b.starts_with(&a)
}

pub fn inspect_target(source: &Path, target: &Path) -> Result<preflight::DirectoryInspection> {
    if !target.is_absolute() {
        return Err(error("请输入目标数据目录的绝对路径"));
    }
    let source = files::directory(source)?;
    let inspected = preflight::inspect_directory(target)?;
    if overlap(&source, &inspected.directory) {
        return Err(error("新旧数据目录不能相同，也不能互相包含"));
    }
    if inspected.state == preflight::DirectoryState::Existing {
        files::directory(target)?;
        require_empty(&inspected.directory)?;
    }
    Ok(inspected)
}

fn require_empty(path: &Path) -> Result<()> {
    if fs::read_dir(path)?.next().transpose()?.is_some() {
        return Err(error("目标数据目录必须为空，不会覆盖或合并已有文件"));
    }
    Ok(())
}

pub fn existing_database(directory: &Path) -> Result<PathBuf> {
    let directory = files::directory(directory)?;
    let path = files::safe_path(&directory, Path::new("tasks.sqlite3"))?;
    if !fs::metadata(&path)?.is_file() {
        return Err(error("保存的数据位置中没有有效数据库，不会创建空库"));
    }
    Store::require_existing(&path)?;
    Ok(directory)
}

pub fn validate_target(source: &Path, target: &Path) -> Result<PathBuf> {
    validate_target_with(source, target, |directory, bytes| {
        preflight::check(directory, Some(bytes), 0).map(|_| ())
    })
}

fn validate_target_with(
    source: &Path,
    target: &Path,
    check: impl FnOnce(&Path, u64) -> Result<()>,
) -> Result<PathBuf> {
    let inspected = inspect_target(source, target)?;
    if inspected.state != preflight::DirectoryState::Existing {
        return Err(error("目标目录已不存在，请重新选择或确认创建"));
    }
    let bytes = files::estimated_bytes(source)?;
    // 为一致性副本和 SQLite 工作文件保留额外空间，写入时仍处理磁盘耗尽。
    check(&inspected.directory, bytes).map_err(|mut failure| {
        if failure.kind == crate::error::ErrorKind::DiskSpace {
            failure.message = format!(
                "目标数据目录空间不足，迁移至少需要 {} 字节可用空间，请释放空间或另选目录",
                bytes.saturating_mul(2)
            );
        }
        failure
    })?;
    Ok(inspected.directory)
}

impl Locations {
    pub fn load(&self) -> Result<State> {
        let Some(bytes) = files::read_config(&self.config_file)? else {
            return Ok(State::default());
        };
        let state: State = serde_json::from_slice(&bytes)?;
        self.validate_state(&state)?;
        Ok(state)
    }

    fn validate_state(&self, state: &State) -> Result<()> {
        if state.version != 1 || state.active.as_ref().is_some_and(|p| !p.is_absolute()) {
            return Err(error("数据位置配置版本或路径无效，不会创建空数据库"));
        }
        if let Some(pending) = &state.pending {
            if uuid::Uuid::parse_str(&pending.id).is_err()
                || !pending.source.is_absolute()
                || !pending.target.is_absolute()
                || overlap(&pending.source, &pending.target)
            {
                return Err(error("迁移记录路径无效，已停止恢复"));
            }
            let expected = if matches!(pending.phase, Phase::Prepared | Phase::Verified) {
                &pending.source
            } else {
                &pending.target
            };
            if self.selected(state) != *expected {
                return Err(error("迁移记录与当前数据位置不一致"));
            }
            let mut names = std::collections::HashSet::new();
            for file in &pending.files {
                let name = file.name.as_path();
                let database = matches!(
                    name.to_str(),
                    Some("tasks.sqlite3" | "tasks.sqlite3-wal" | "tasks.sqlite3-shm")
                );
                let backup = name.parent() == Some(Path::new("backups"))
                    && name
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("sqlite3"));
                if (!database && !backup)
                    || !names.insert(name)
                    || file.sha256.len() != 64
                    || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(error("迁移清单无效，已停止恢复与清理"));
                }
                if name
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    return Err(error("迁移清单包含无效路径"));
                }
            }
            if pending.phase != Phase::Prepared
                && (!pending
                    .files
                    .iter()
                    .any(|f| f.name == Path::new("tasks.sqlite3"))
                    || pending.target_database.as_ref().is_none_or(|f| {
                        f.name != Path::new("tasks.sqlite3")
                            || f.sha256.len() != 64
                            || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    }))
            {
                return Err(error("迁移记录缺少有效数据库校验清单"));
            }
        }
        Ok(())
    }

    pub fn save(&self, state: &State) -> Result<()> {
        self.validate_state(state)?;
        files::atomic_write(&self.config_file, &serde_json::to_vec_pretty(state)?)
    }

    pub fn selected(&self, state: &State) -> PathBuf {
        state
            .active
            .as_ref()
            .unwrap_or(&self.default_directory)
            .clone()
    }

    pub fn require_database(&self, state: &State) -> Result<()> {
        let directory = self.selected(state);
        files::directory(&directory)?;
        files::safe_path(&directory, Path::new("tasks.sqlite3"))?;
        Store::validate_database(&directory.join("tasks.sqlite3"))
    }

    pub fn verify_before_start(&self) -> Result<()> {
        let state = self.load()?;
        if let Some(pending) = state.pending.filter(|p| p.phase == Phase::Switched) {
            let expected = pending
                .target_database
                .as_ref()
                .ok_or_else(|| error("缺少迁移快照校验信息"))?;
            if !files::matches(&pending.target, expected)? {
                return Err(error("目标数据库在切换后发生变化，停止启用与清理"));
            }
            for entry in &pending.files {
                if entry.name.starts_with("backups") && !files::matches(&pending.target, entry)? {
                    return Err(error("目标备份校验失败，停止启用与清理"));
                }
            }
            Store::validate_database(&pending.target.join("tasks.sqlite3"))?;
        }
        Ok(())
    }

    pub fn mark_started(&self) -> Result<()> {
        let mut state = self.load()?;
        if let Some(pending) = state
            .pending
            .as_mut()
            .filter(|p| p.phase == Phase::Switched)
        {
            pending.phase = Phase::Cleanup;
            self.save(&state)?;
        }
        Ok(())
    }

    pub fn rollback(&self, reason: &str) -> Result<State> {
        let mut state = self.load()?;
        let pending = state
            .pending
            .as_ref()
            .ok_or_else(|| error("没有可回退的迁移记录"))?;
        if pending.phase == Phase::Cleanup {
            return Err(error("新目录已投入使用，不能回退至旧副本"));
        }
        let source = pending.source.clone();
        Store::validate_database(&source.join("tasks.sqlite3"))?;
        state.message = Some(format!(
            "{reason}；已恢复原数据目录。目标目录中的迁移副本保留在 {}，未覆盖或删除原数据。",
            pending.target.display()
        ));
        state.active = Some(source);
        state.pending = None;
        self.save(&state)?;
        Ok(state)
    }

    pub fn migrate(&self, source: &Path, target: &Path, keep_backup: bool) -> Result<()> {
        self.migrate_steps(source, target, keep_backup, &|_| Ok(()))
    }

    fn migrate_steps(
        &self,
        source: &Path,
        target: &Path,
        keep_backup: bool,
        boundary: &impl Fn(Phase) -> Result<()>,
    ) -> Result<()> {
        let source = files::directory(source)?;
        let target = validate_target(&source, target)?;
        let _source = DirectoryGuard::acquire(&source)?;
        let _target = DirectoryGuard::acquire(&target)?;
        let mut state = self.load()?;
        if state.pending.is_some() {
            return Err(error("上次迁移尚未完成，请先重启处理迁移结果"));
        }
        if files::directory(&self.selected(&state))? != source {
            return Err(error("当前数据位置已变化，请重启后重试"));
        }
        Store::validate_database(&source.join("tasks.sqlite3"))?;
        let mut pending = Migration {
            id: uuid::Uuid::new_v4().to_string(),
            source: source.clone(),
            target: target.clone(),
            keep_backup,
            phase: Phase::Prepared,
            files: Vec::new(),
            target_database: None,
        };
        state.active = Some(source.clone());
        state.pending = Some(pending.clone());
        state.message = None;
        self.save(&state)?;
        boundary(Phase::Prepared)?;
        require_empty(&target)?;
        let entries = files::inventory(&source)?;
        Store::copy_database(&source.join("tasks.sqlite3"), &target.join("tasks.sqlite3"))?;
        for entry in &entries {
            if entry.name.starts_with("backups") {
                files::copy(&source, &target, entry)?;
            }
        }
        Store::validate_database(&target.join("tasks.sqlite3"))?;
        for entry in &entries {
            if !files::matches(&source, entry)? {
                return Err(error("迁移期间原数据发生变化，未切换目录"));
            }
        }
        pending.files = entries;
        pending.target_database = Some(files::entry(&target, Path::new("tasks.sqlite3"))?);
        pending.phase = Phase::Verified;
        state.pending = Some(pending.clone());
        boundary(Phase::Verified)?;
        self.save(&state)?;
        pending.phase = Phase::Switched;
        state.pending = Some(pending);
        state.active = Some(target);
        boundary(Phase::Switched)?;
        self.save(&state)
    }

    /// 仅在新服务及窗口初始化成功后调用；一旦进入此阶段不再回退旧副本。
    pub fn finish(&self) -> Result<String> {
        let mut state = self.load()?;
        let Some(mut pending) = state.pending.clone() else {
            return Ok(state.message.unwrap_or_default());
        };
        if !matches!(pending.phase, Phase::Switched | Phase::Cleanup) {
            return Err(error("迁移尚未验证，禁止清理原数据"));
        }
        pending.phase = Phase::Cleanup;
        state.pending = Some(pending.clone());
        self.save(&state)?;
        let message = if pending.keep_backup {
            format!(
                "数据目录已更改。原目录作为备份保留：{}。下载任务已暂停，可手动继续。",
                pending.source.display()
            )
        } else {
            cleanup(&pending)?;
            format!("数据目录已更改，原目录中已迁移的应用数据已清理。下载文件、续传分片及未知文件保持原位置：{}。下载任务已暂停，可手动继续。", pending.source.display())
        };
        state.pending = None;
        state.message = Some(message.clone());
        self.save(&state)?;
        Ok(message)
    }
}

fn cleanup(pending: &Migration) -> Result<()> {
    if !pending.source.try_exists()? {
        return Ok(());
    }
    let source = files::directory(&pending.source)?;
    if source != pending.source {
        return Err(error("原数据目录位置已变化，停止清理"));
    }
    let guard = DirectoryGuard::acquire(&source)?;
    // 已投入使用的目标会正常产生新写入，不能再与迁移前的物理文件摘要比较。
    Store::validate_database(&pending.target.join("tasks.sqlite3"))?;
    for current in files::inventory(&source)? {
        if !pending.files.iter().any(|old| {
            old.name == current.name && old.size == current.size && old.sha256 == current.sha256
        }) {
            return Err(error(format!(
                "原目录存在新增或变化的数据，已停止清理：{}",
                source.join(current.name).display()
            )));
        }
    }
    for entry in &pending.files {
        files::delete_matching(&source, entry)?;
    }
    remove_empty(&source.join("backups"))?;
    drop(guard);
    remove_empty(&source)
}

fn remove_empty(path: &Path) -> Result<()> {
    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().transpose()?.is_none() {
                fs::remove_dir(path)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

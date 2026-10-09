use crate::{
    error::{DownloadError, ErrorKind, Result},
    history::{self, HistoryPage, PAGE_SIZE, RECENT_TASKS},
    model::{Settings, StoredTask, TaskStatus},
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;

mod backup;
mod compatibility;
#[cfg(test)]
mod compatibility_tests;
mod migration;
#[cfg(test)]
mod tests;

pub struct Store {
    connection: Connection,
}

fn integer(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| DownloadError::new(ErrorKind::Storage, "任务数值超出数据库整数范围"))
}

fn validate(id: &str, created: i64, record: &StoredTask) -> Result<()> {
    if record.task.id != id
        || uuid::Uuid::parse_str(id).is_err()
        || integer(record.task.created_at)? != created
    {
        return Err(DownloadError::new(
            ErrorKind::Storage,
            "任务标识或时间损坏，原始数据库未被覆盖",
        ));
    }
    let source = crate::source::parse_release_url(&record.task.url)?;
    if source.filename != record.task.filename {
        return Err(DownloadError::new(
            ErrorKind::Storage,
            "任务文件名与来源不一致",
        ));
    }
    crate::source::validate_filename(&record.task.filename)?;
    integer(record.task.details.queue_position)?;
    Ok(())
}

fn decode(row: &rusqlite::Row<'_>) -> Result<StoredTask> {
    let id: String = row.get(0)?;
    let created: i64 = row.get(1)?;
    let json: String = row.get(2)?;
    let mut record: StoredTask = serde_json::from_str(&json)?;
    validate(&id, created, &record)?;
    let status: String = row.get(3)?;
    let search: String = row.get(4)?;
    let position: i64 = row.get(5)?;
    // 共享 JSON 使用旧版可识别的暂停状态，当前进程的等待状态保留在投影中。
    if status == TaskStatus::WaitingNetwork.as_str() && record.task.status == TaskStatus::Paused {
        record.task.status = TaskStatus::WaitingNetwork;
    }
    if status != record.task.status.as_str()
        || search != history::search_text(&record.task)
        || position != integer(record.task.details.queue_position)?
    {
        return Err(DownloadError::new(
            ErrorKind::Storage,
            "任务索引与保存内容不一致，请保留数据库并从备份恢复",
        ));
    }
    Ok(record)
}

fn write_record(connection: &Connection, record: &StoredTask, insert: bool) -> Result<()> {
    let created = integer(record.task.created_at)?;
    validate(&record.task.id, created, record)?;
    let sql = if insert {
        "INSERT INTO tasks(id,created_at,payload,status,search_text,queue_position) VALUES (?1,?2,?3,?4,?5,?6)"
    } else {
        "INSERT INTO tasks(id,created_at,payload,status,search_text,queue_position) VALUES (?1,?2,?3,?4,?5,?6)
        ON CONFLICT(id) DO UPDATE SET created_at=excluded.created_at,payload=excluded.payload,status=excluded.status,search_text=excluded.search_text,queue_position=excluded.queue_position"
    };
    connection.execute(
        sql,
        params![
            record.task.id,
            created,
            compatibility::encode(record)?,
            record.task.status.as_str(),
            history::search_text(&record.task),
            integer(record.task.details.queue_position)?
        ],
    )?;
    connection.execute(
        "DELETE FROM githubsp_dirty_tasks WHERE id=?1",
        [&record.task.id],
    )?;
    Ok(())
}

impl Store {
    pub(crate) fn require_existing(path: &Path) -> Result<()> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 0 {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "保存的数据位置中没有已初始化的应用数据库，不会创建空库",
            ));
        }
        connection.close().map_err(|(_, error)| error.into())
    }

    /// 迁移只读已完成初始化的库，禁止在路径错误时创建空库。
    pub fn validate_database(path: &Path) -> Result<()> {
        let store = Self::open_readonly(path)?;
        Self::check_integrity(&store.connection)?;
        store.settings()?;
        store.favorites()?;
        let mut statement = store.connection.prepare(
            "SELECT id,created_at,payload,status,search_text,queue_position FROM tasks ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            decode(row)?;
        }
        drop(rows);
        drop(statement);
        store.close()
    }

    /// 复制一致性快照后逐行比较持久化内容，不依赖数据库文件的物理布局。
    pub fn copy_database(source: &Path, target: &Path) -> Result<()> {
        Self::validate_database(source)?;
        let source = Self::open_readonly(source)?;
        source.export_backup(target, &|| false)?;
        let target = Self::open_readonly(target)?;
        for sql in [
            "SELECT id,created_at,payload,status,search_text,queue_position FROM tasks ORDER BY id",
            "SELECT key,value FROM settings ORDER BY key",
            "SELECT repository,payload FROM favorites ORDER BY repository",
            "SELECT id,revision FROM githubsp_storage_meta ORDER BY id",
            "SELECT id FROM githubsp_dirty_tasks ORDER BY id",
        ] {
            let mut left = source.connection.prepare(sql)?;
            let mut right = target.connection.prepare(sql)?;
            let columns = left.column_count();
            let mut left = left.query([])?;
            let mut right = right.query([])?;
            loop {
                match (left.next()?, right.next()?) {
                    (None, None) => break,
                    (Some(a), Some(b)) => {
                        for index in 0..columns {
                            if a.get_ref(index)? != b.get_ref(index)? {
                                return Err(DownloadError::new(
                                    ErrorKind::Integrity,
                                    "迁移前后的数据库记录不一致，原数据已保留",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(DownloadError::new(
                            ErrorKind::Integrity,
                            "迁移前后的数据库记录不一致，原数据已保留",
                        ))
                    }
                }
            }
        }
        target.close()?;
        source.close()
    }

    pub fn close(self) -> Result<()> {
        self.connection.close().map_err(|(_, error)| error.into())
    }

    pub fn open(path: &Path) -> Result<Self> {
        let path = std::path::absolute(path)?;
        let mut connection = Connection::open(&path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        migration::initialize(&mut connection, &path)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
        Ok(Self { connection })
    }

    pub(crate) fn open_readonly(path: &Path) -> Result<Self> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA query_only=ON;")?;
        migration::check_ready(&connection)?;
        compatibility::require_clean(&connection)?;
        Ok(Self { connection })
    }

    fn check_integrity(connection: &Connection) -> Result<()> {
        let mut statement = connection.prepare("PRAGMA integrity_check")?;
        let mut rows = statement.query([])?;
        let mut checked = false;
        while let Some(row) = rows.next()? {
            let result: String = row.get(0)?;
            if result != "ok" {
                return Err(DownloadError::new(
                    ErrorKind::Storage,
                    "数据库完整性检查失败，原始数据未被覆盖",
                ));
            }
            checked = true;
        }
        if !checked {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "数据库完整性检查未返回结果",
            ));
        }
        Ok(())
    }

    fn select(&self, sql: &str) -> Result<Vec<StoredTask>> {
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(decode(row)?);
        }
        Ok(records)
    }

    pub fn recover(&self) -> Result<Vec<StoredTask>> {
        let mut records = self.select("SELECT id,created_at,payload,status,search_text,queue_position FROM tasks WHERE status NOT IN ('completed','cancelled') ORDER BY queue_position,created_at,id")?;
        // 全部校验后再恢复状态，避免后续坏记录导致前面的记录已被部分修改。
        let transaction = self.connection.unchecked_transaction()?;
        for record in &mut records {
            if record.task.status.running() && record.task.details.elapsed_ms.is_some() {
                record.task.details.elapsed_is_partial = true;
            }
            if record.task.status.running()
                || matches!(
                    record.task.status,
                    TaskStatus::Queued | TaskStatus::WaitingNetwork
                )
            {
                record.task.status = TaskStatus::Paused;
                record.task.revision += 1;
            }
            record.task.speed = 0.0;
            record.task.eta = None;
            record.task.details.retry_info = None;
            record.task.details.recovery_info = None;
            record.task.details.route_suggestion = None;
            write_record(&transaction, record, false)?;
        }
        let recent = self.recent_terminal()?;
        transaction.commit()?;
        records.extend(recent);
        records.sort_by(|a, b| {
            (a.task.details.queue_position, a.task.created_at, &a.task.id).cmp(&(
                b.task.details.queue_position,
                b.task.created_at,
                &b.task.id,
            ))
        });
        Ok(records)
    }

    pub fn recent_terminal(&self) -> Result<Vec<StoredTask>> {
        // 固定使用按时间排列的部分索引，避免两个结束状态触发全量临时排序。
        self.select(&format!("SELECT id,created_at,payload,status,search_text,queue_position FROM tasks INDEXED BY tasks_recent WHERE status IN ('completed','cancelled') ORDER BY created_at DESC,id DESC LIMIT {RECENT_TASKS}"))
    }

    pub fn task(&self, id: &str) -> Result<Option<StoredTask>> {
        uuid::Uuid::parse_str(id)
            .map_err(|_| DownloadError::new(ErrorKind::InvalidInput, "无效任务标识"))?;
        let mut statement = self.connection.prepare(
            "SELECT id,created_at,payload,status,search_text,queue_position FROM tasks WHERE id=?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode).transpose()
    }

    pub fn counts(&self) -> Result<(usize, usize)> {
        let (total, completed): (i64, i64) = self.connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(status='completed'),0) FROM tasks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((total as usize, completed as usize))
    }

    pub fn next_position(&self) -> Result<u64> {
        let maximum: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(queue_position),-1) FROM tasks",
            [],
            |row| row.get(0),
        )?;
        let next = maximum
            .checked_add(1)
            .ok_or_else(|| DownloadError::new(ErrorKind::Storage, "队列序号超出支持范围"))?;
        Ok(next as u64)
    }

    pub fn save(&self, record: &StoredTask) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        write_record(&transaction, record, false)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert(&mut self, record: &StoredTask) -> Result<()> {
        let transaction = self.connection.transaction()?;
        write_record(&transaction, record, true)?;
        transaction.execute("INSERT INTO settings(key,value) VALUES ('last_directory',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [record.task.directory.to_string_lossy().as_ref()])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_completed(&self) -> Result<usize> {
        Ok(self
            .connection
            .execute("DELETE FROM tasks WHERE status='completed'", [])?)
    }

    pub fn export_backup(&self, target: &Path, cancelled: &impl Fn() -> bool) -> Result<()> {
        backup::export(&self.connection, target, cancelled)
    }

    pub fn query_history(
        &mut self,
        query: &str,
        status: Option<TaskStatus>,
        page: u32,
        revision: u64,
    ) -> Result<HistoryPage> {
        let query = history::normalize_query(query, page)?;
        let transaction = self.connection.transaction()?;
        let mut predicates = Vec::new();
        let mut parameters = Vec::<rusqlite::types::Value>::new();
        if let Some(status) = status {
            predicates.push("status=?");
            parameters.push(status.as_str().to_owned().into());
        }
        if !query.is_empty() {
            predicates.push("instr(search_text,?)>0");
            parameters.push(query.into());
        }
        let predicate = if predicates.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", predicates.join(" AND "))
        };
        let total: i64 = transaction.query_row(
            &format!("SELECT COUNT(*) FROM tasks{predicate}"),
            rusqlite::params_from_iter(&parameters),
            |row| row.get(0),
        )?;
        let total = usize::try_from(total)
            .map_err(|_| DownloadError::new(ErrorKind::Storage, "历史记录数量超出支持范围"))?;
        let last = total.div_ceil(PAGE_SIZE).max(1).min(u32::MAX as usize) as u32;
        let page = page.min(last);
        let offset = i64::from(page - 1) * PAGE_SIZE as i64;
        let records = {
            let mut statement = transaction.prepare(&format!("SELECT id,created_at,payload,status,search_text,queue_position FROM tasks{predicate} ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?"))?;
            parameters.push((PAGE_SIZE as i64).into());
            parameters.push(offset.into());
            let mut rows = statement.query(rusqlite::params_from_iter(&parameters))?;
            let mut records = Vec::with_capacity(PAGE_SIZE);
            while let Some(row) = rows.next()? {
                records.push(decode(row)?.task);
            }
            records
        };
        transaction.commit()?;
        // 文件系统查询在读事务结束后执行，避免慢磁盘持续占用 WAL 读快照。
        Ok(HistoryPage {
            items: records.into_iter().map(history::entry).collect(),
            total,
            page,
            page_size: PAGE_SIZE,
            revision,
        })
    }

    pub fn last_directory(&self) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key='last_directory'",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM tasks WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings> {
        self.get_json("preferences").map(|v| v.unwrap_or_default())
    }
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        value
            .map(|v| serde_json::from_str(&v).map_err(Into::into))
            .transpose()
    }
    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.connection.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, serde_json::to_string(value)?])?;
        Ok(())
    }
    pub fn save_order(&mut self, records: &[StoredTask]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for record in records {
            write_record(&transaction, record, false)?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn favorites(&self) -> Result<Vec<crate::model::Favorite>> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM favorites ORDER BY repository COLLATE NOCASE")?;
        let values = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut favorites = Vec::new();
        for value in values {
            let favorite: crate::model::Favorite = serde_json::from_str(&value?)?;
            crate::source::parse_resource(&format!("https://github.com/{}", favorite.repository))?;
            favorites.push(favorite);
        }
        Ok(favorites)
    }
    pub fn save_favorite(&self, favorite: &crate::model::Favorite) -> Result<()> {
        self.connection.execute("INSERT INTO favorites(repository,payload) VALUES (?1,?2) ON CONFLICT(repository) DO UPDATE SET payload=excluded.payload", params![favorite.repository, serde_json::to_string(favorite)?])?;
        Ok(())
    }
    pub fn remove_favorite(&self, repository: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM favorites WHERE repository=?1 COLLATE NOCASE",
            [repository],
        )?;
        Ok(())
    }
}

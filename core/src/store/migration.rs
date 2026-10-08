use super::{integer, validate, Store};
use crate::{
    error::{DownloadError, ErrorKind, Result},
    history,
    model::StoredTask,
};
use rusqlite::{params, Connection};
use std::path::Path;

pub(super) const VERSION: u32 = 3;

pub(super) fn initialize(connection: &mut Connection, path: &Path) -> Result<()> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > VERSION {
        return Err(DownloadError::new(
            ErrorKind::Storage,
            "任务数据库来自更新版本，请使用相应版本打开",
        ));
    }
    if version == VERSION {
        // 已升级的库只检查结构；历史内容在读取时校验，不在每次启动时全表反序列化。
        connection.prepare(
            "SELECT id,created_at,payload,status,search_text,queue_position FROM tasks LIMIT 0",
        )?;
        return Ok(());
    }
    if version == 0 {
        let tables: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if tables != 0 {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "数据库缺少受支持的结构版本，原始数据未被覆盖",
            ));
        }
    } else {
        let backups = path.parent().unwrap_or(Path::new(".")).join("backups");
        std::fs::create_dir_all(&backups)?;
        let backup = backups.join(format!(
            "tasks-v{version}-before-v3-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        super::backup::export(connection, &backup, &|| false)?;
    }
    let transaction = connection.transaction()?;
    if version == 0 {
        transaction.execute_batch("CREATE TABLE tasks (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, payload TEXT NOT NULL, status TEXT NOT NULL, search_text TEXT NOT NULL, queue_position INTEGER NOT NULL);
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE favorites (repository TEXT PRIMARY KEY COLLATE NOCASE, payload TEXT NOT NULL);")?;
    } else {
        transaction.execute_batch("ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT '';
            ALTER TABLE tasks ADD COLUMN search_text TEXT NOT NULL DEFAULT '';
            ALTER TABLE tasks ADD COLUMN queue_position INTEGER NOT NULL DEFAULT 0;
            CREATE TABLE IF NOT EXISTS favorites (repository TEXT PRIMARY KEY COLLATE NOCASE, payload TEXT NOT NULL);")?;
        let mut statement = transaction
            .prepare("SELECT id,created_at,payload FROM tasks ORDER BY created_at,id")?;
        let mut rows = statement.query([])?;
        let mut position = 0_u64;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let created: i64 = row.get(1)?;
            let original: String = row.get(2)?;
            let mut record: StoredTask = serde_json::from_str(&original)?;
            validate(&id, created, &record)?;
            if version == 1 {
                let source = crate::source::parse_release_url(&record.task.url)?;
                record.task.url = source.url.to_string();
                record.task.details.repository = Some(format!("{}/{}", source.owner, source.repo));
                record.task.details.tag = Some(source.tag);
                record.task.details.queue_position = position;
            }
            let payload = if version == 1 {
                serde_json::to_string(&record)?
            } else {
                original
            };
            transaction.execute("UPDATE tasks SET payload=?1,status=?2,search_text=?3,queue_position=?4 WHERE id=?5", params![payload, record.task.status.as_str(), history::search_text(&record.task), integer(record.task.details.queue_position)?, id])?;
            position = position
                .checked_add(1)
                .ok_or_else(|| DownloadError::new(ErrorKind::Storage, "任务数量超出支持范围"))?;
        }
    }
    transaction.execute_batch("CREATE INDEX tasks_created ON tasks(created_at DESC,id DESC);
        CREATE INDEX tasks_status_created ON tasks(status,created_at DESC,id DESC);
        CREATE INDEX tasks_queue ON tasks(queue_position,id);
        CREATE INDEX tasks_recent ON tasks(created_at DESC,id DESC) WHERE status IN ('completed','cancelled');
        CREATE INDEX tasks_active_queue ON tasks(queue_position,created_at,id) WHERE status NOT IN ('completed','cancelled');
        PRAGMA user_version=3;")?;
    Store::check_integrity(&transaction)?;
    transaction.commit()?;
    Ok(())
}

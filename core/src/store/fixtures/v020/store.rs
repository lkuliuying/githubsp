// 固定兼容夹具：v0.2.0，src-tauri/src/store.rs，Git blob ef40b9de82c4288f0ae87112a6f659d1ef8bac8a。
// 适配模块引用及 rusqlite 0.39 的整数绑定；其余持久化与恢复逻辑保持发布版原样。
use super::model::{Settings, StoredTask, TaskStatus};
use crate::{
    error::{DownloadError, ErrorKind, Result},
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn close(self) -> Result<()> {
        self.connection.close().map_err(|(_, error)| error.into())
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 2 {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "任务数据库来自更新版本，请使用相应版本打开",
            ));
        }
        if version == 1 {
            let backup =
                path.with_file_name(format!("tasks-v1-backup-{}.sqlite3", uuid::Uuid::new_v4()));
            connection.backup("main", &backup, None)?;
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
        let transaction = connection.transaction()?;
        transaction.execute_batch("
            CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS favorites (repository TEXT PRIMARY KEY COLLATE NOCASE, payload TEXT NOT NULL);")?;
        if version == 1 {
            let values: Vec<(String, String)> = {
                let mut statement =
                    transaction.prepare("SELECT id,payload FROM tasks ORDER BY created_at,id")?;
                let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
                rows.collect::<std::result::Result<_, _>>()?
            };
            for (position, (id, json)) in values.into_iter().enumerate() {
                let mut record: StoredTask = serde_json::from_str(&json)?;
                if record.task.id != id || uuid::Uuid::parse_str(&id).is_err() {
                    return Err(DownloadError::new(
                        ErrorKind::Storage,
                        "旧任务标识损坏，迁移已回滚",
                    ));
                }
                let source = crate::source::parse_release_url(&record.task.url)?;
                if source.filename != record.task.filename {
                    return Err(DownloadError::new(
                        ErrorKind::Storage,
                        "旧任务文件名与来源不一致，迁移已回滚",
                    ));
                }
                record.task.url = source.url.to_string();
                record.task.details.repository = Some(format!("{}/{}", source.owner, source.repo));
                record.task.details.tag = Some(source.tag);
                record.task.details.queue_position = position as u64;
                transaction.execute(
                    "UPDATE tasks SET payload=?1 WHERE id=?2",
                    params![serde_json::to_string(&record)?, id],
                )?;
            }
        }
        transaction.execute_batch("PRAGMA user_version=2;")?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn recover(&self) -> Result<Vec<StoredTask>> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM tasks ORDER BY created_at, id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut records = Vec::new();
        for row in rows {
            let mut record: StoredTask = serde_json::from_str(&row?)?;
            uuid::Uuid::parse_str(&record.task.id).map_err(|_| {
                DownloadError::new(
                    ErrorKind::Storage,
                    "任务标识损坏，已停止加载，原始数据库未被覆盖",
                )
            })?;
            crate::source::parse_release_url(&record.task.url)?;
            crate::source::validate_filename(&record.task.filename)?;
            if record.task.status.running() || record.task.status == TaskStatus::Queued {
                record.task.status = TaskStatus::Paused;
                record.task.revision += 1;
            }
            record.task.speed = 0.0;
            record.task.eta = None;
            self.save(&record)?;
            records.push(record);
        }
        records.sort_by_key(|record| (record.task.details.queue_position, record.task.created_at));
        Ok(records)
    }

    pub fn save(&self, record: &StoredTask) -> Result<()> {
        self.connection.execute(
            "INSERT INTO tasks(id,created_at,payload) VALUES (?1,?2,?3)
            ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            params![
                record.task.id,
                i64::try_from(record.task.created_at).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                serde_json::to_string(record)?
            ],
        )?;
        Ok(())
    }

    pub fn insert(&mut self, record: &StoredTask) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO tasks(id,created_at,payload) VALUES (?1,?2,?3)",
            params![
                record.task.id,
                i64::try_from(record.task.created_at).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                serde_json::to_string(record)?
            ],
        )?;
        transaction.execute(
            "INSERT INTO settings(key,value) VALUES ('last_directory',?1)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [record.task.directory.to_string_lossy().as_ref()],
        )?;
        transaction.commit()?;
        Ok(())
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
            transaction.execute(
                "UPDATE tasks SET payload=?1 WHERE id=?2",
                params![serde_json::to_string(record)?, record.task.id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn favorites(&self) -> Result<Vec<super::model::Favorite>> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM favorites ORDER BY repository COLLATE NOCASE")?;
        let values = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut favorites = Vec::new();
        for value in values {
            let favorite: super::model::Favorite = serde_json::from_str(&value?)?;
            crate::source::parse_resource(&format!("https://github.com/{}", favorite.repository))?;
            favorites.push(favorite);
        }
        Ok(favorites)
    }
    pub fn save_favorite(&self, favorite: &super::model::Favorite) -> Result<()> {
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

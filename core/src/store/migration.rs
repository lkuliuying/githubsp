use super::{compatibility, integer, validate, Store};
use crate::{
    error::{DownloadError, ErrorKind, Result},
    history,
    model::{Favorite, RouteReport, Settings, StoredTask, TaskStatus},
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub(super) const VERSION: u32 = 2;
const REVISION: u32 = 1;

fn unsupported() -> DownloadError {
    DownloadError::new(
        ErrorKind::Storage,
        "数据库结构或内部修订不受支持，请使用相应版本打开；原始数据未被覆盖",
    )
}

fn schema_sql(connection: &Connection, name: &str) -> Result<Option<String>> {
    Ok(connection
        .query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [name], |r| {
            r.get(0)
        })
        .optional()?)
}

fn check_definition(connection: &Connection, name: &str, expected: &str) -> Result<()> {
    let normalize = |sql: &str| {
        sql.trim()
            .trim_end_matches(';')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    if schema_sql(connection, name)?.is_none_or(|sql| normalize(&sql) != normalize(expected)) {
        return Err(unsupported());
    }
    Ok(())
}

fn check_columns(connection: &Connection, table: &str, expected: &[(&str, &str)]) -> Result<()> {
    let mut statement =
        connection.prepare("SELECT name,type,pk FROM pragma_table_info(?1) ORDER BY cid")?;
    let columns = statement
        .query_map([table], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if columns.len() != expected.len()
        || columns.iter().zip(expected).enumerate().any(
            |(i, ((name, kind, key), (wanted_name, wanted_type)))| {
                name != wanted_name
                    || !kind.eq_ignore_ascii_case(wanted_type)
                    || *key != u32::from(i == 0)
            },
        )
    {
        return Err(unsupported());
    }
    Ok(())
}

fn check_auxiliary_structure(connection: &Connection, require_favorites: bool) -> Result<()> {
    check_columns(
        connection,
        "settings",
        &[("key", "TEXT"), ("value", "TEXT")],
    )?;
    if require_favorites || schema_sql(connection, "favorites")?.is_some() {
        check_columns(
            connection,
            "favorites",
            &[("repository", "TEXT"), ("payload", "TEXT")],
        )?;
    }
    Ok(())
}

pub(super) fn check_ready(connection: &Connection) -> Result<()> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != VERSION {
        return Err(unsupported());
    }
    check_definition(connection, "githubsp_storage_meta", compatibility::META)?;
    let revisions: Vec<(u32, u32)> = {
        let mut statement = connection.prepare("SELECT id,revision FROM githubsp_storage_meta")?;
        let rows = statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    if revisions != [(1, REVISION)] {
        return Err(unsupported());
    }
    check_definition(connection, "tasks", compatibility::TASKS)?;
    check_definition(connection, "githubsp_dirty_tasks", compatibility::DIRTY)?;
    check_auxiliary_structure(connection, true)?;
    for (name, sql) in compatibility::INDEXES {
        check_definition(connection, name, sql)?;
    }
    for (name, sql) in compatibility::triggers() {
        check_definition(connection, name, &sql)?;
    }
    Ok(())
}

fn check_legacy(connection: &Connection, version: u32) -> Result<()> {
    let mut statement = connection.prepare("SELECT type,name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND type IN ('table','view','trigger')")?;
    let objects = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if version == 0 {
        return if objects.is_empty() {
            Ok(())
        } else {
            Err(unsupported())
        };
    }
    if objects.iter().any(|(kind, name)| {
        kind != "table" || !["tasks", "settings", "favorites"].contains(&name.as_str())
    }) {
        return Err(unsupported());
    }
    let mut columns = vec![
        ("id", "TEXT"),
        ("created_at", "INTEGER"),
        ("payload", "TEXT"),
    ];
    if version == 3 {
        columns.extend([
            ("status", "TEXT"),
            ("search_text", "TEXT"),
            ("queue_position", "INTEGER"),
        ]);
    }
    check_columns(connection, "tasks", &columns)?;
    check_auxiliary_structure(connection, version >= 2)
}

fn validate_auxiliary(connection: &Connection) -> Result<()> {
    let mut settings = connection.prepare("SELECT key,value FROM settings")?;
    let mut rows = settings.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        match key.as_str() {
            "preferences" => {
                serde_json::from_str::<Settings>(&value)?;
            }
            "diagnostics" => {
                serde_json::from_str::<Vec<RouteReport>>(&value)?;
            }
            _ => (),
        }
    }
    let mut favorites = connection.prepare("SELECT repository,payload FROM favorites")?;
    let mut rows = favorites.query([])?;
    while let Some(row) = rows.next()? {
        let repository: String = row.get(0)?;
        let favorite: Favorite = serde_json::from_str(&row.get::<_, String>(1)?)?;
        crate::source::parse_resource(&format!("https://github.com/{}", favorite.repository))?;
        if !favorite.repository.eq_ignore_ascii_case(&repository) {
            return Err(unsupported());
        }
    }
    Ok(())
}

fn copy_tasks(connection: &Connection, version: u32) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT id,created_at,payload FROM githubsp_previous_tasks ORDER BY created_at,id",
    )?;
    let mut rows = statement.query([])?;
    let mut position = 0_u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let created: i64 = row.get(1)?;
        let original: String = row.get(2)?;
        let mut record: StoredTask = serde_json::from_str(&original)?;
        validate(&id, created, &record)?;
        let mut payload = original.clone();
        if version == 1 || record.task.status == TaskStatus::WaitingNetwork {
            // 只改需要转换的字段，保留其他 JSON 内容，避免迁移时丢弃扩展信息。
            let mut value: serde_json::Value = serde_json::from_str(&original)?;
            if version == 1 {
                let source = crate::source::parse_release_url(&record.task.url)?;
                record.task.url = source.url.to_string();
                record.task.details.repository = Some(format!("{}/{}", source.owner, source.repo));
                record.task.details.tag = Some(source.tag);
                record.task.details.queue_position = position;
                value["task"]["url"] = record.task.url.clone().into();
                value["task"]["repository"] =
                    serde_json::to_value(&record.task.details.repository)?;
                value["task"]["tag"] = serde_json::to_value(&record.task.details.tag)?;
                value["task"]["queuePosition"] = position.into();
            }
            if record.task.status == TaskStatus::WaitingNetwork {
                value["task"]["status"] = "paused".into();
            }
            payload = serde_json::to_string(&value)?;
        }
        connection.execute("INSERT INTO tasks(id,created_at,payload,status,search_text,queue_position) VALUES(?1,?2,?3,?4,?5,?6)", params![id,created,payload,record.task.status.as_str(),history::search_text(&record.task),integer(record.task.details.queue_position)?])?;
        position = position.checked_add(1).ok_or_else(unsupported)?;
    }
    Ok(())
}

pub(super) fn initialize(connection: &mut Connection, path: &Path) -> Result<()> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > 3 {
        return Err(unsupported());
    }
    if schema_sql(connection, "githubsp_storage_meta")?.is_some() {
        check_ready(connection)?;
        return compatibility::repair(connection);
    }
    check_legacy(connection, version)?;
    if version != 0 {
        let backups = path.parent().unwrap_or(Path::new(".")).join("backups");
        std::fs::create_dir_all(&backups)?;
        let backup = backups.join(format!(
            "tasks-v{version}-before-compat-{REVISION}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        super::backup::export(connection, &backup, &|| false)?;
    }
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if version != 0 {
        transaction.execute_batch("ALTER TABLE tasks RENAME TO githubsp_previous_tasks;")?;
    }
    transaction.execute_batch(compatibility::TASKS)?;
    transaction.execute_batch("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS favorites (repository TEXT PRIMARY KEY COLLATE NOCASE, payload TEXT NOT NULL);")?;
    if version != 0 {
        copy_tasks(&transaction, version)?;
        transaction.execute_batch("DROP TABLE githubsp_previous_tasks;")?;
    }
    transaction.execute_batch(compatibility::META)?;
    transaction.execute_batch(compatibility::DIRTY)?;
    transaction.execute(
        "INSERT INTO githubsp_storage_meta(id,revision) VALUES(1,?1)",
        [REVISION],
    )?;
    for (_, sql) in compatibility::INDEXES {
        transaction.execute_batch(sql)?;
    }
    for (_, sql) in compatibility::triggers() {
        transaction.execute_batch(&sql)?;
    }
    validate_auxiliary(&transaction)?;
    transaction.pragma_update(None, "user_version", VERSION)?;
    check_ready(&transaction)?;
    Store::check_integrity(&transaction)?;
    transaction.commit()?;
    Ok(())
}

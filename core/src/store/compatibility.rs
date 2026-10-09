use super::{validate, write_record};
use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::{StoredTask, TaskStatus},
};
use rusqlite::Connection;

pub(super) const TASKS: &str = "CREATE TABLE tasks (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, payload TEXT NOT NULL, status TEXT NOT NULL DEFAULT '', search_text TEXT NOT NULL DEFAULT '', queue_position INTEGER NOT NULL DEFAULT 0)";
pub(super) const META: &str = "CREATE TABLE githubsp_storage_meta (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL)";
pub(super) const DIRTY: &str = "CREATE TABLE githubsp_dirty_tasks (id TEXT PRIMARY KEY)";
pub(super) const INDEXES: &[(&str, &str)] = &[
    ("tasks_created", "CREATE INDEX tasks_created ON tasks(created_at DESC,id DESC)"),
    ("tasks_status_created", "CREATE INDEX tasks_status_created ON tasks(status,created_at DESC,id DESC)"),
    ("tasks_queue", "CREATE INDEX tasks_queue ON tasks(queue_position,id)"),
    ("tasks_recent", "CREATE INDEX tasks_recent ON tasks(created_at DESC,id DESC) WHERE status IN ('completed','cancelled')"),
    ("tasks_active_queue", "CREATE INDEX tasks_active_queue ON tasks(queue_position,created_at,id) WHERE status NOT IN ('completed','cancelled')"),
];

pub(super) fn encode(record: &StoredTask) -> Result<String> {
    if record.task.status == TaskStatus::WaitingNetwork {
        let mut persisted = record.clone();
        persisted.task.status = TaskStatus::Paused;
        Ok(serde_json::to_string(&persisted)?)
    } else {
        Ok(serde_json::to_string(record)?)
    }
}

fn normalized_payload(update: bool) -> String {
    let mut payload = "CASE WHEN json_extract(NEW.payload,'$.task.status')='waiting_network' THEN json_set(NEW.payload,'$.task.status','paused') ELSE NEW.payload END".to_owned();
    if update {
        // 缺失字段代表旧版不认识它；显式 null、false、0 必须保留其原意。
        let executed = "(json_extract(NEW.payload,'$.task.downloaded') IS NOT json_extract(OLD.payload,'$.task.downloaded')
            OR json_extract(NEW.payload,'$.checkpoint') IS NOT json_extract(OLD.payload,'$.checkpoint')
            OR json_extract(NEW.payload,'$.task.completedAt') IS NOT json_extract(OLD.payload,'$.task.completedAt')
            OR (json_extract(NEW.payload,'$.task.status') IS NOT json_extract(OLD.payload,'$.task.status')
                AND (json_extract(NEW.payload,'$.task.status') IN ('probing','downloading','retrying','verifying','pausing','cancelling','completed')
                    OR json_extract(OLD.payload,'$.task.status') IN ('probing','downloading','retrying','verifying','pausing','cancelling'))))";
        payload = format!(
            "json_insert({payload},
            '$.task.elapsedMs',json(OLD.payload -> '$.task.elapsedMs'),
            '$.task.elapsedIsPartial',json(CASE
                WHEN json_type(NEW.payload,'$.task.elapsedMs') IS NULL AND {executed} THEN 'true'
                ELSE COALESCE(OLD.payload -> '$.task.elapsedIsPartial','false') END),
            '$.task.routeFailures',json(COALESCE(OLD.payload -> '$.task.routeFailures','[]')))"
        );
    }
    // 旧版接管时不恢复过期的重试、倒计时和换线建议。
    format!("CASE WHEN json_type(NEW.payload,'$.task.elapsedMs') IS NULL THEN json_insert({payload},'$.task.elapsedMs',NULL,'$.task.elapsedIsPartial',json('true'),'$.task.retryInfo',NULL,'$.task.recoveryInfo',NULL,'$.task.routeSuggestion',NULL) ELSE {payload} END")
}

pub(super) fn triggers() -> Vec<(&'static str, String)> {
    let insert = normalized_payload(false);
    let update = normalized_payload(true);
    vec![
        ("githubsp_tasks_insert", format!("CREATE TRIGGER githubsp_tasks_insert AFTER INSERT ON tasks BEGIN
            UPDATE tasks SET payload={insert} WHERE id=NEW.id AND payload IS NOT {insert};
            INSERT INTO githubsp_dirty_tasks(id) VALUES(NEW.id) ON CONFLICT(id) DO NOTHING;
        END")),
        ("githubsp_tasks_update", format!("CREATE TRIGGER githubsp_tasks_update AFTER UPDATE OF id,created_at,payload ON tasks WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at OR NEW.payload IS NOT OLD.payload BEGIN
            UPDATE tasks SET payload={update} WHERE id=NEW.id AND payload IS NOT {update};
            DELETE FROM githubsp_dirty_tasks WHERE id=OLD.id AND OLD.id IS NOT NEW.id;
            INSERT INTO githubsp_dirty_tasks(id) VALUES(NEW.id) ON CONFLICT(id) DO NOTHING;
        END")),
        ("githubsp_tasks_delete", "CREATE TRIGGER githubsp_tasks_delete AFTER DELETE ON tasks BEGIN DELETE FROM githubsp_dirty_tasks WHERE id=OLD.id; END".into()),
        ("githubsp_preferences_update", "CREATE TRIGGER githubsp_preferences_update AFTER UPDATE OF value ON settings WHEN NEW.key='preferences' AND json_type(NEW.value,'$.backgroundCompletionNotice') IS NULL AND json_type(OLD.value,'$.backgroundCompletionNotice') IS NOT NULL BEGIN
            UPDATE settings SET value=json_insert(NEW.value,'$.backgroundCompletionNotice',json(OLD.value -> '$.backgroundCompletionNotice')) WHERE key=NEW.key;
        END".into()),
    ]
}

pub(super) fn require_clean(connection: &Connection) -> Result<()> {
    let dirty: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM githubsp_dirty_tasks)",
        [],
        |r| r.get(0),
    )?;
    if dirty {
        return Err(DownloadError::new(
            ErrorKind::Storage,
            "旧版修改的任务索引尚未就绪，请退出其他版本后重新启动",
        ));
    }
    Ok(())
}

pub(super) fn repair(connection: &mut Connection) -> Result<()> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    loop {
        // 分批读取后再修改扫描表，既限制内存，也避免一边扫描一边删行的游标歧义。
        let batch = {
            let mut statement = transaction.prepare("SELECT t.id,t.created_at,t.payload FROM githubsp_dirty_tasks d JOIN tasks t ON t.id=d.id ORDER BY d.id LIMIT 128")?;
            let rows = statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        if batch.is_empty() {
            break;
        }
        for (id, created, payload) in batch {
            let record: StoredTask = serde_json::from_str(&payload)?;
            validate(&id, created, &record)?;
            write_record(&transaction, &record, false)?;
        }
    }
    transaction.execute("DELETE FROM githubsp_dirty_tasks WHERE NOT EXISTS(SELECT 1 FROM tasks WHERE tasks.id=githubsp_dirty_tasks.id)", [])?;
    transaction.commit()?;
    Ok(())
}

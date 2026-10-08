use super::*;
use crate::{
    engine::Engine,
    manager::{Action, Manager},
    model::{Settings, Task, TaskDetails, Verification},
};
use std::{path::PathBuf, sync::Arc};

fn record(root: &Path, index: u64, status: TaskStatus) -> StoredTask {
    StoredTask {
        task: Task {
            id: format!("00000000-0000-4000-8000-{index:012}"),
            url: "https://github.com/Test/Repo/releases/download/v1/中文_File.zip".into(),
            filename: "中文_File.zip".into(),
            directory: root.to_owned(),
            status,
            downloaded: 0,
            total: None,
            speed: 0.0,
            eta: None,
            route: None,
            verification: Verification::Unverified,
            error: None,
            final_path: None,
            created_at: 100,
            revision: 0,
            details: TaskDetails {
                repository: Some("Test/Repo".into()),
                tag: Some("v1".into()),
                queue_position: index,
                ..Default::default()
            },
        },
        checkpoint: None,
    }
}

fn legacy(path: &Path, version: u32, records: &[StoredTask]) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
        CREATE TABLE tasks(id TEXT PRIMARY KEY,created_at INTEGER NOT NULL,payload TEXT NOT NULL);
        CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        CREATE TABLE favorites(repository TEXT PRIMARY KEY COLLATE NOCASE,payload TEXT NOT NULL);",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
    for item in records {
        connection
            .execute(
                "INSERT INTO tasks VALUES(?1,?2,?3)",
                params![
                    item.task.id,
                    integer(item.task.created_at).unwrap(),
                    serde_json::to_string(item).unwrap()
                ],
            )
            .unwrap();
    }
    connection
        .execute(
            "INSERT INTO settings VALUES('last_directory','保留中文目录')",
            [],
        )
        .unwrap();
    connection
}

fn backup_files(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root.join("backups"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

#[test]
fn fresh_schema_and_repeat_start_keep_full_durability() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    for _ in 0..2 {
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "synchronous", |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert!(store.recover().unwrap().is_empty());
        assert_eq!(store.counts().unwrap(), (0, 0));
        store.close().unwrap();
    }
    assert!(!root.path().join("backups").exists());
    assert_eq!(rusqlite::version(), "3.51.3");
}

#[test]
fn working_set_and_history_order_use_indexes() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    for (sql,index) in [
        ("SELECT id FROM tasks WHERE status NOT IN ('completed','cancelled') ORDER BY queue_position,created_at,id", "tasks_active_queue"),
        ("SELECT id FROM tasks INDEXED BY tasks_recent WHERE status IN ('completed','cancelled') ORDER BY created_at DESC,id DESC LIMIT 50", "tasks_recent"),
        ("SELECT id FROM tasks WHERE status='completed' ORDER BY created_at DESC,id DESC LIMIT 20", "tasks_status_created"),
        ("SELECT id FROM tasks ORDER BY created_at DESC,id DESC LIMIT 20", "tasks_created"),
    ] {
        let mut statement = store.connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let plan = statement.query_map([],|row|row.get::<_,String>(3)).unwrap().collect::<std::result::Result<Vec<_>,_>>().unwrap().join("\n");
        assert!(plan.contains(index),"{sql}: {plan}");
        assert!(!plan.contains("TEMP B-TREE"),"{plan}");
    }
}

#[test]
fn legacy_versions_migrate_with_consistent_wal_backup_and_preserve_content() {
    for version in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tasks.sqlite3");
        let item = record(root.path(), 9, TaskStatus::Paused);
        let legacy = legacy(&path, version, std::slice::from_ref(&item));
        legacy
            .execute(
                "INSERT INTO favorites VALUES('Test/Repo',?1)",
                [serde_json::to_string(&crate::model::Favorite {
                    id: "favorite".into(),
                    repository: "Test/Repo".into(),
                    latest: None,
                    seen: vec![],
                    last_checked: None,
                    last_success: None,
                    next_check: None,
                    error: None,
                })
                .unwrap()],
            )
            .unwrap();
        assert!(
            std::fs::metadata(path.with_extension("sqlite3-wal"))
                .unwrap()
                .len()
                > 0
        );
        let store = Store::open(&path).unwrap();
        let migrated = store.task(&item.task.id).unwrap().unwrap();
        assert_eq!(
            migrated.task.details.queue_position,
            if version == 1 { 0 } else { 9 }
        );
        assert_eq!(store.last_directory().unwrap().unwrap(), "保留中文目录");
        assert_eq!(store.favorites().unwrap().len(), 1);
        if version == 2 {
            let json: String = store
                .connection
                .query_row("SELECT payload FROM tasks", [], |r| r.get(0))
                .unwrap();
            assert_eq!(json, serde_json::to_string(&item).unwrap());
        }
        let files = backup_files(root.path());
        assert_eq!(files.len(), 1);
        let backup = Connection::open(&files[0]).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            version
        );
        assert_eq!(
            backup
                .query_row("SELECT payload FROM tasks", [], |r| r.get::<_, String>(0))
                .unwrap(),
            serde_json::to_string(&item).unwrap()
        );
        assert_eq!(
            backup
                .query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            1
        );
        Store::check_integrity(&backup).unwrap();
        store.close().unwrap();
        drop(legacy);
        Store::open(&path).unwrap().close().unwrap();
        assert_eq!(backup_files(root.path()).len(), 1);
    }
}

#[test]
fn damaged_record_rolls_back_all_projection_and_schema_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let good = record(root.path(), 1, TaskStatus::Paused);
    let bad = record(root.path(), 2, TaskStatus::Completed);
    let connection = legacy(&path, 2, &[good.clone(), bad.clone()]);
    connection
        .execute(
            "UPDATE tasks SET payload='broken' WHERE id=?1",
            [&bad.task.id],
        )
        .unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(connection.prepare("SELECT status FROM tasks").is_err());
    assert_eq!(
        connection
            .query_row(
                "SELECT payload FROM tasks WHERE id=?1",
                [&good.task.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        serde_json::to_string(&good).unwrap()
    );
    assert_eq!(backup_files(root.path()).len(), 1);
}

#[test]
fn migration_refuses_newer_unknown_or_physically_damaged_database() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE keep(value TEXT); INSERT INTO keep VALUES('preserve'); PRAGMA user_version=4;").unwrap();
    assert!(Store::open(&path).is_err());
    connection.pragma_update(None, "user_version", 0).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        connection
            .query_row("SELECT value FROM keep", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
    let damaged = root.path().join("damaged.sqlite3");
    std::fs::write(&damaged, b"not a database").unwrap();
    assert!(Store::open(&damaged).is_err());
    assert_eq!(std::fs::read(damaged).unwrap(), b"not a database");
}

#[test]
fn migration_stops_before_writing_when_backup_directory_is_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let connection = legacy(&path, 2, &[]);
    std::fs::write(root.path().join("backups"), b"keep").unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(connection.prepare("SELECT status FROM tasks").is_err());
}

#[test]
fn disk_full_during_migration_rolls_back_and_keeps_backup() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let records: Vec<_> = (0..200)
        .map(|i| record(root.path(), i, TaskStatus::Completed))
        .collect();
    let mut connection = legacy(&path, 2, &records);
    let pages: u32 = connection
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    connection
        .pragma_update(None, "max_page_count", pages)
        .unwrap();
    let error = migration::initialize(&mut connection, &path).unwrap_err();
    assert!(error.message.contains("full"), "{error}");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(connection.prepare("SELECT status FROM tasks").is_err());
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get::<_, i32>(0))
            .unwrap(),
        200
    );
    assert_eq!(backup_files(root.path()).len(), 1);
}

#[test]
fn backup_exports_wal_data_and_never_overwrites_or_leaves_partial_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    store
        .connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    store
        .save(&record(root.path(), 1, TaskStatus::Completed))
        .unwrap();
    let output = root.path().join("export");
    std::fs::create_dir(&output).unwrap();
    let target = output.join("backup.sqlite3");
    store.export_backup(&target, &|| false).unwrap();
    let backup = Store::open_readonly(&target).unwrap();
    assert_eq!(backup.counts().unwrap(), (1, 1));
    Store::check_integrity(&backup.connection).unwrap();
    backup.close().unwrap();
    let bytes = std::fs::read(&target).unwrap();
    assert!(store.export_backup(&target, &|| false).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), bytes);
    assert!(store
        .export_backup(&output.join("cancelled.sqlite3"), &|| true)
        .is_err());
    assert!(store
        .export_backup(&output.join("missing/file.sqlite3"), &|| false)
        .is_err());
    assert!(store
        .export_backup(Path::new("relative.sqlite3"), &|| false)
        .is_err());
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 1);
    assert_eq!(store.counts().unwrap(), (1, 1));
}

#[test]
fn history_filters_unicode_literal_substrings_all_states_and_stable_ties() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    let statuses = [
        TaskStatus::Queued,
        TaskStatus::Probing,
        TaskStatus::Downloading,
        TaskStatus::Retrying,
        TaskStatus::WaitingNetwork,
        TaskStatus::Verifying,
        TaskStatus::Pausing,
        TaskStatus::Cancelling,
        TaskStatus::Paused,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];
    for (index, status) in statuses.iter().enumerate() {
        store
            .save(&record(root.path(), index as u64, *status))
            .unwrap();
    }
    for status in statuses {
        let page = store
            .query_history("  中文_fILE  ", Some(status), 1, 7)
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].task.status, status);
        assert_eq!(page.revision, 7);
    }
    let page = store.query_history("TEST/repo", None, 1, 0).unwrap();
    assert_eq!(page.total, 12);
    assert_eq!(
        page.items[0].task.id,
        record(root.path(), 11, TaskStatus::Cancelled).task.id
    );
    assert_eq!(store.query_history("%", None, 1, 0).unwrap().total, 0);
    assert_eq!(store.query_history("_", None, 1, 0).unwrap().total, 12);
    assert!(store.query_history("", None, 0, 0).is_err());
    assert!(store.query_history(&"x".repeat(1025), None, 1, 0).is_err());
    let empty = store.query_history("absent", None, u32::MAX, 0).unwrap();
    assert_eq!((empty.page, empty.total), (1, 0));
    assert!(empty.items.is_empty());
}

#[test]
fn history_only_decodes_current_page_and_clamps_after_last_page_removal() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    for i in 0..41 {
        store
            .save(&record(root.path(), i, TaskStatus::Completed))
            .unwrap();
    }
    let oldest = record(root.path(), 0, TaskStatus::Completed).task.id;
    store
        .connection
        .execute("UPDATE tasks SET payload='broken' WHERE id=?1", [&oldest])
        .unwrap();
    assert_eq!(store.query_history("", None, 1, 0).unwrap().items.len(), 20);
    assert!(store.query_history("", None, 3, 0).is_err());
    store.remove(&oldest).unwrap();
    let page = store.query_history("", None, 3, 0).unwrap();
    assert_eq!((page.page, page.total, page.items.len()), (2, 40, 20));
}

#[test]
fn projections_update_atomically_and_overflow_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    let mut item = record(root.path(), 1, TaskStatus::Paused);
    store.insert(&item).unwrap();
    assert!(store.insert(&item).is_err());
    item.task.status = TaskStatus::Completed;
    item.task.details.tag = Some("v2".into());
    store.save(&item).unwrap();
    assert_eq!(
        store
            .query_history("v2", Some(TaskStatus::Completed), 1, 0)
            .unwrap()
            .total,
        1
    );
    item.task.details.queue_position = u64::MAX;
    assert!(store.save(&item).is_err());
    assert_eq!(
        store
            .task(&item.task.id)
            .unwrap()
            .unwrap()
            .task
            .details
            .queue_position,
        1
    );
    item.task.details.queue_position = 1;
    item.task.created_at = u64::MAX;
    assert!(store.save(&item).is_err());
    assert_eq!(
        store.task(&item.task.id).unwrap().unwrap().task.created_at,
        100
    );
}

#[tokio::test]
async fn working_set_and_operations_cover_records_outside_recent_fifty() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    for i in 0..75 {
        store
            .save(&record(root.path(), i, TaskStatus::Completed))
            .unwrap();
    }
    for i in 75..78 {
        store
            .save(&record(root.path(), i, TaskStatus::Paused))
            .unwrap();
    }
    store.settings().unwrap();
    store.set_json("preferences", &Settings::default()).unwrap();
    store.close().unwrap();
    let manager = Manager::start(&path, Engine::production().unwrap(), Arc::new(|_| {})).unwrap();
    let snapshot = manager.snapshot().await.unwrap();
    assert_eq!(
        (
            snapshot.total_tasks,
            snapshot.completed_tasks,
            snapshot.tasks.len()
        ),
        (78, 75, 53)
    );
    let cold = record(root.path(), 0, TaskStatus::Completed).task.id;
    assert!(!snapshot.tasks.iter().any(|task| task.id == cold));
    assert_eq!(
        manager.task(cold.clone()).await.unwrap().status,
        TaskStatus::Completed
    );
    assert_eq!(
        manager
            .history("中文".into(), None, 4)
            .await
            .unwrap()
            .items
            .len(),
        18
    );
    manager.action(cold.clone(), Action::Remove).await.unwrap();
    assert!(manager.task(cold.clone()).await.is_err());
    assert!(manager.action(cold, Action::Remove).await.is_err());
    assert!(manager
        .remove_completed(snapshot.history_revision)
        .await
        .is_err());
    let current = manager.snapshot().await.unwrap();
    assert_eq!(current.tasks.len(), 53);
    let file = root.path().join("keep-download.zip");
    std::fs::write(&file, b"download").unwrap();
    manager
        .remove_completed(current.history_revision)
        .await
        .unwrap();
    let cleared = manager.snapshot().await.unwrap();
    assert_eq!(
        (
            cleared.total_tasks,
            cleared.completed_tasks,
            cleared.tasks.len()
        ),
        (3, 0, 3)
    );
    assert_eq!(std::fs::read(file).unwrap(), b"download");
    let target = root.path().join("manual.sqlite3");
    manager.export_backup(target.clone()).await.unwrap();
    assert_eq!(
        Store::open_readonly(&target).unwrap().counts().unwrap(),
        (3, 0)
    );
    manager.shutdown().await.unwrap();
    assert!(manager.history("".into(), None, 1).await.is_err());
    std::fs::rename(&path, root.path().join("closed.sqlite3")).unwrap();
}

#[test]
fn progress_does_not_invalidate_history_but_status_and_file_changes_do() {
    let root = tempfile::tempdir().unwrap();
    let old = record(root.path(), 1, TaskStatus::Downloading);
    let mut next = old.clone();
    next.task.downloaded = 20;
    next.task.speed = 300.0;
    next.task.revision += 1;
    assert!(!history::changed(&old.task, &next.task));
    next.task.status = TaskStatus::Completed;
    assert!(history::changed(&old.task, &next.task));
    next = old.clone();
    next.task.final_path = Some(root.path().join("renamed.zip"));
    assert!(history::changed(&old.task, &next.task));
}

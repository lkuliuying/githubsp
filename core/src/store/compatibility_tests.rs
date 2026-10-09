use super::{
    compatibility,
    tests::{backup_files, legacy, record},
    *,
};
use crate::model::{Checkpoint, Part, Settings, TaskStatus};
use serde_json::{json, Value};

// v0.2.0、v0.2.1、v0.2.2 的模型和存储文件 Git blob 完全相同。
#[allow(dead_code)]
mod v020 {
    pub mod model {
        include!("fixtures/v020/model.rs");
    }
    pub mod store {
        include!("fixtures/v020/store.rs");
    }
}
#[allow(dead_code)]
mod v023 {
    pub mod model {
        include!("fixtures/v023/model.rs");
    }
    pub mod store {
        include!("fixtures/v023/store.rs");
    }
}

fn raw(connection: &Connection, id: &str) -> Value {
    let payload: String = connection
        .query_row("SELECT payload FROM tasks WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    serde_json::from_str(&payload).unwrap()
}

fn dirty(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM githubsp_dirty_tasks", [], |r| {
            r.get(0)
        })
        .unwrap()
}

fn early_payload(record: &StoredTask) -> String {
    let task: v020::model::StoredTask =
        serde_json::from_str(&compatibility::encode(record).unwrap()).unwrap();
    serde_json::to_string(&task).unwrap()
}

macro_rules! roundtrip {
    ($test:ident, $version:ident) => {
        #[test]
        fn $test() {
            use $version::{model as old_model, store::Store as OldStore};
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("tasks.sqlite3");
            let store = Store::open(&path).unwrap();
            let mut item = record(root.path(), 1, TaskStatus::Paused);
            item.task.details.elapsed_ms = Some(500);
            item.checkpoint = Some(Checkpoint {
                route_id: "direct".into(), total: Some(100), etag: Some("\"stable\"".into()),
                expected_sha256: None, range_supported: true,
                parts: vec![Part {start:0,end:Some(99),downloaded:10,sha256:"saved-digest".into()}], publication: None,
            });
            store.save(&item).unwrap();
            store.save(&record(root.path(),2,TaskStatus::WaitingNetwork)).unwrap();
            store.set_json("preferences",&Settings {background_completion_notice:false,..Default::default()}).unwrap();
            assert_eq!(dirty(&store.connection),0);
            store.close().unwrap();

            let mut old = OldStore::open(&path).unwrap();
            let mut records = old.recover().unwrap();
            assert!(records.iter().all(|r| r.task.status == old_model::TaskStatus::Paused));
            let changed = records.iter_mut().find(|r| r.task.id == item.task.id).unwrap();
            changed.task.downloaded = 20;
            changed.checkpoint.as_mut().unwrap().parts[0].downloaded = 20;
            changed.task.details.queue_position = 90;
            changed.task.details.tag = Some("new-tag".into());
            changed.task.status = old_model::TaskStatus::Completed;
            old.save(changed).unwrap();
            old.save_order(&records).unwrap();
            let extra: old_model::StoredTask = serde_json::from_str(&compatibility::encode(&record(root.path(),3,TaskStatus::Paused)).unwrap()).unwrap();
            old.insert(&extra).unwrap();
            assert!(old.insert(&extra).is_err());
            old.remove(&records.iter().find(|r| r.task.id != item.task.id).unwrap().task.id).unwrap();
            let mut settings = old.settings().unwrap();
            settings.limit_kib = 123;
            old.set_json("preferences",&settings).unwrap();
            let favorite: old_model::Favorite = serde_json::from_value(json!({"id":"favorite","repository":"Test/Repo","latest":null,"seen":[],"lastChecked":null,"lastSuccess":null,"nextCheck":null,"error":null})).unwrap();
            old.save_favorite(&favorite).unwrap();
            assert_eq!(old.favorites().unwrap().len(),1);
            old.remove_favorite("Test/Repo").unwrap();
            old.save_favorite(&favorite).unwrap();
            old.close().unwrap();
            assert!(Store::open_readonly(&path).is_err());

            let mut store = Store::open(&path).unwrap();
            assert_eq!(dirty(&store.connection),0);
            let actual = store.task(&item.task.id).unwrap().unwrap();
            assert_eq!(actual.task.downloaded,20);
            assert_eq!(actual.checkpoint.unwrap().parts[0].downloaded,20);
            assert_eq!(actual.task.details.queue_position,90);
            assert_eq!(actual.task.details.elapsed_ms,Some(500));
            assert_eq!(store.counts().unwrap(),(2,1));
            assert_eq!(store.query_history("new-tag",Some(TaskStatus::Completed),1,0).unwrap().total,1);
            assert_eq!(store.settings().unwrap().limit_kib,123);
            assert!(!store.settings().unwrap().background_completion_notice);
            assert_eq!(store.favorites().unwrap().len(),1);
            assert_eq!(store.last_directory().unwrap(),Some(root.path().to_string_lossy().into()));
            let backup = root.path().join("compatible-backup.sqlite3");
            store.export_backup(&backup,&||false).unwrap();
            assert_eq!(OldStore::open(&backup).unwrap().recover().unwrap().len(),2);
            assert_eq!(store.remove_completed().unwrap(),1);
            assert_eq!(store.counts().unwrap(),(1,0));
            store.close().unwrap();
            assert_eq!(OldStore::open(&path).unwrap().recover().unwrap().len(),1);
        }
    }
}
roundtrip!(v020_published_store_roundtrip, v020);
roundtrip!(v021_published_store_roundtrip, v020);
roundtrip!(v022_published_store_roundtrip, v020);
roundtrip!(v023_published_store_roundtrip, v023);

#[test]
fn original_v023_can_write_waiting_then_v020_can_open_without_a_new_app_between() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    let mut item = record(root.path(), 1, TaskStatus::WaitingNetwork);
    item.task.details.elapsed_ms = Some(40);
    store.save(&item).unwrap();
    assert_eq!(
        store.task(&item.task.id).unwrap().unwrap().task.status,
        TaskStatus::WaitingNetwork
    );
    assert_eq!(
        raw(&store.connection, &item.task.id)["task"]["status"],
        "paused"
    );
    store.close().unwrap();
    let old = v023::store::Store::open(&path).unwrap();
    let mut item23 = old.recover().unwrap().remove(0);
    item23.task.status = v023::model::TaskStatus::WaitingNetwork;
    old.save(&item23).unwrap();
    old.close().unwrap();
    let old = v020::store::Store::open(&path).unwrap();
    let mut item20 = old.recover().unwrap().remove(0);
    assert_eq!(item20.task.status, v020::model::TaskStatus::Paused);
    item20.task.status = v020::model::TaskStatus::Downloading;
    item20.task.downloaded = 5;
    old.save(&item20).unwrap();
    old.close().unwrap();
    let old = v023::store::Store::open(&path).unwrap();
    let item23 = old.recover().unwrap().remove(0);
    assert_eq!(item23.task.details.elapsed_ms, Some(40));
    assert!(item23.task.details.elapsed_is_partial);
    assert!(item23.task.details.recovery_info.is_none());
    old.close().unwrap();
    let store = Store::open(&path).unwrap();
    assert_eq!(store.recover().unwrap()[0].task.status, TaskStatus::Paused);
}

#[test]
fn triggers_preserve_missing_fields_and_explicit_null_false_zero_with_both_recursion_modes() {
    for recursive in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tasks.sqlite3");
        let store = Store::open(&path).unwrap();
        store
            .connection
            .pragma_update(None, "recursive_triggers", recursive)
            .unwrap();
        let mut item = record(root.path(), 1, TaskStatus::Paused);
        item.task.details.elapsed_ms = Some(500);
        store.save(&item).unwrap();
        let legacy = early_payload(&item);
        for _ in 0..3 {
            store
                .connection
                .execute(
                    "UPDATE tasks SET payload=?1 WHERE id=?2",
                    params![legacy, item.task.id],
                )
                .unwrap();
        }
        let value = raw(&store.connection, &item.task.id);
        assert_eq!(value["task"]["elapsedMs"], 500);
        assert_eq!(value["task"]["elapsedIsPartial"], false);
        assert_eq!(dirty(&store.connection), 1);
        for elapsed in [Value::Null, json!(0)] {
            let mut value = value.clone();
            value["task"]["elapsedMs"] = elapsed.clone();
            value["task"]["elapsedIsPartial"] = json!(false);
            store
                .connection
                .execute(
                    "UPDATE tasks SET payload=?1 WHERE id=?2",
                    params![value.to_string(), item.task.id],
                )
                .unwrap();
            assert_eq!(
                raw(&store.connection, &item.task.id)["task"]["elapsedMs"],
                elapsed
            );
        }
        store
            .set_json(
                "preferences",
                &json!({"limitKib":1,"backgroundCompletionNotice":false}),
            )
            .unwrap();
        store
            .set_json("preferences", &json!({"limitKib":0}))
            .unwrap();
        assert!(!store.settings().unwrap().background_completion_notice);
        store
            .set_json("preferences", &json!({"backgroundCompletionNotice":null}))
            .unwrap();
        assert_eq!(
            store.get_json::<Value>("preferences").unwrap().unwrap()["backgroundCompletionNotice"],
            Value::Null
        );
        assert!(store.settings().is_err());
        store.set_json("preferences", &Settings::default()).unwrap();
        store.close().unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(dirty(&store.connection), 0);
        assert_eq!(
            store
                .task(&item.task.id)
                .unwrap()
                .unwrap()
                .task
                .details
                .elapsed_ms,
            Some(0)
        );
    }
}

#[test]
fn both_v3_layouts_convert_without_losing_new_records_and_keep_original_backup() {
    for fresh in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tasks.sqlite3");
        let item = record(root.path(), 1, TaskStatus::WaitingNetwork);
        let connection = legacy(&path, 2, std::slice::from_ref(&item));
        // 两种已发布 v3 结构分别来自新建数据库和原 v2 升级。
        connection.execute_batch("ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT ''; ALTER TABLE tasks ADD COLUMN search_text TEXT NOT NULL DEFAULT ''; ALTER TABLE tasks ADD COLUMN queue_position INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=3;").unwrap();
        if fresh {
            // 非空表不能直接添加无默认值的 NOT NULL 列，因此另建完整的新建版结构。
            drop(connection);
            let mut connection = Connection::open(&path).unwrap();
            let tx = connection.transaction().unwrap();
            tx.execute_batch("ALTER TABLE tasks RENAME TO old_tasks; CREATE TABLE tasks(id TEXT PRIMARY KEY,created_at INTEGER NOT NULL,payload TEXT NOT NULL,status TEXT NOT NULL,search_text TEXT NOT NULL,queue_position INTEGER NOT NULL); INSERT INTO tasks SELECT id,created_at,payload,'waiting_network','old',1 FROM old_tasks; DROP TABLE old_tasks;").unwrap();
            tx.commit().unwrap();
        } else {
            drop(connection);
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.counts().unwrap(), (1, 0));
        assert_eq!(
            raw(&store.connection, &item.task.id)["task"]["status"],
            "paused"
        );
        let backups = backup_files(root.path());
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            raw(&backup, &item.task.id)["task"]["status"],
            "waiting_network"
        );
        store.close().unwrap();
        let mut old = v020::store::Store::open(&path).unwrap();
        assert_eq!(old.recover().unwrap().len(), 1);
        let item: v020::model::StoredTask =
            serde_json::from_str(&early_payload(&record(root.path(), 2, TaskStatus::Paused)))
                .unwrap();
        old.insert(&item).unwrap();
        old.close().unwrap();
        assert_eq!(Store::open(&path).unwrap().counts().unwrap(), (2, 0));
        assert_eq!(backup_files(root.path()).len(), 1);
    }
}

#[test]
fn native_write_rolls_back_payload_and_indexes_when_dirty_acknowledgement_fails() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(&root.path().join("tasks.sqlite3")).unwrap();
    let mut item = record(root.path(), 1, TaskStatus::Paused);
    store.save(&item).unwrap();
    store.connection.execute_batch("CREATE TRIGGER fail_ack BEFORE DELETE ON githubsp_dirty_tasks BEGIN SELECT RAISE(ABORT,'fixture failure'); END").unwrap();
    item.task.status = TaskStatus::Completed;
    assert!(store.save(&item).is_err());
    assert_eq!(
        store.task(&item.task.id).unwrap().unwrap().task.status,
        TaskStatus::Paused
    );
    assert_eq!(dirty(&store.connection), 0);
}

#[test]
fn unsupported_internal_revision_and_damaged_trigger_are_not_silently_repaired() {
    for sql in [
        "UPDATE githubsp_storage_meta SET revision=2",
        "DROP TRIGGER githubsp_tasks_update",
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tasks.sqlite3");
        let store = Store::open(&path).unwrap();
        store
            .save(&record(root.path(), 1, TaskStatus::Paused))
            .unwrap();
        store.connection.execute_batch(sql).unwrap();
        store.close().unwrap();
        assert!(Store::open(&path).is_err());
        assert!(Store::open_readonly(&path).is_err());
        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            1
        );
    }
}

#[test]
fn dirty_repair_is_atomic_and_rejects_invalid_old_data() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    for id in [1, 2] {
        store
            .save(&record(root.path(), id, TaskStatus::Paused))
            .unwrap();
    }
    let first = record(root.path(), 1, TaskStatus::Completed);
    store
        .connection
        .execute(
            "UPDATE tasks SET payload=?1 WHERE id=?2",
            params![early_payload(&first), first.task.id],
        )
        .unwrap();
    let mut bad = record(root.path(), 2, TaskStatus::Paused);
    let id = bad.task.id.clone();
    bad.task.id = "broken-id".into();
    store
        .connection
        .execute(
            "UPDATE tasks SET payload=?1 WHERE id=?2",
            params![early_payload(&bad), id],
        )
        .unwrap();
    assert!(store
        .connection
        .execute("UPDATE tasks SET payload='not-json' WHERE id=?1", [&id])
        .is_err());
    store.close().unwrap();
    assert!(Store::open(&path).is_err());
    let connection = Connection::open(&path).unwrap();
    assert_eq!(dirty(&connection), 2);
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM tasks WHERE id=?1",
                [&first.task.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "paused"
    );
}

#[test]
fn repeated_native_start_does_not_decode_untouched_old_history() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let store = Store::open(&path).unwrap();
    for id in 0..65 {
        store
            .save(&record(root.path(), id, TaskStatus::Completed))
            .unwrap();
    }
    // 模拟磁盘损坏，保留合法结构和干净标记；只有读取该历史页才应失败。
    store
        .connection
        .execute_batch("DROP TRIGGER githubsp_tasks_update")
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE tasks SET payload='broken' WHERE id=?1",
            [record(root.path(), 0, TaskStatus::Completed).task.id],
        )
        .unwrap();
    store
        .connection
        .execute_batch(
            &compatibility::triggers()
                .into_iter()
                .find(|(name, _)| *name == "githubsp_tasks_update")
                .unwrap()
                .1,
        )
        .unwrap();
    store.close().unwrap();
    for _ in 0..2 {
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.recover().unwrap().len(), 50);
        assert_eq!(store.query_history("", None, 1, 0).unwrap().total, 65);
        assert!(store.query_history("", None, 4, 0).is_err());
        assert_eq!(dirty(&store.connection), 0);
        store.close().unwrap();
    }
}

#[test]
fn legacy_changes_repair_in_multiple_batches_and_deleted_rows_stay_deleted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    Store::open(&path).unwrap().close().unwrap();
    let mut old = v020::store::Store::open(&path).unwrap();
    for id in 0..260 {
        let item: v020::model::StoredTask = serde_json::from_str(&early_payload(&record(
            root.path(),
            id,
            TaskStatus::Completed,
        )))
        .unwrap();
        old.insert(&item).unwrap();
    }
    old.remove(&record(root.path(), 0, TaskStatus::Completed).task.id)
        .unwrap();
    old.close().unwrap();
    let mut store = Store::open(&path).unwrap();
    assert_eq!(dirty(&store.connection), 0);
    assert_eq!(store.counts().unwrap(), (259, 259));
    assert_eq!(
        store
            .query_history("中文", Some(TaskStatus::Completed), 13, 0)
            .unwrap()
            .items
            .len(),
        19
    );
    assert!(store
        .task(&record(root.path(), 0, TaskStatus::Completed).task.id)
        .unwrap()
        .is_none());
}

#[test]
fn invalid_preferences_roll_back_conversion_and_preserve_v3_records() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tasks.sqlite3");
    let item = record(root.path(), 1, TaskStatus::Completed);
    let connection = legacy(&path, 2, std::slice::from_ref(&item));
    connection.execute_batch("ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT ''; ALTER TABLE tasks ADD COLUMN search_text TEXT NOT NULL DEFAULT ''; ALTER TABLE tasks ADD COLUMN queue_position INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=3; INSERT INTO settings VALUES('preferences','broken');").unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        raw(&connection, &item.task.id)["task"]["status"],
        "completed"
    );
    assert!(connection
        .prepare("SELECT * FROM githubsp_storage_meta")
        .is_err());
    assert_eq!(backup_files(root.path()).len(), 1);
}

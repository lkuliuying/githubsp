use super::*;
use crate::model::{Favorite, Settings, StoredTask, Task, TaskDetails, TaskStatus, Verification};

fn fixture() -> (tempfile::TempDir, Locations, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("原始 数据");
    let target = root.path().join("新的 数据");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&target).unwrap();
    let source = files::directory(&source).unwrap();
    let target = files::directory(&target).unwrap();
    let mut store = Store::open(&source.join("tasks.sqlite3")).unwrap();
    fs::write(root.path().join("中文.zip"), b"downloaded").unwrap();
    store
        .insert(&StoredTask {
            task: Task {
                id: "00000000-0000-4000-8000-000000000001".into(),
                url: "https://github.com/test/repo/releases/download/v1/中文.zip".into(),
                filename: "中文.zip".into(),
                directory: root.path().to_owned(),
                status: TaskStatus::Completed,
                downloaded: 10,
                total: Some(10),
                speed: 0.0,
                eta: None,
                route: Some("direct".into()),
                verification: Verification::Unverified,
                error: None,
                final_path: Some(root.path().join("中文.zip")),
                created_at: 123,
                revision: 7,
                details: TaskDetails::default(),
            },
            checkpoint: None,
        })
        .unwrap();
    store
        .save_favorite(&Favorite {
            id: "favorite".into(),
            repository: "test/repo".into(),
            latest: None,
            seen: vec![1, 2, 3],
            last_checked: Some(123),
            last_success: Some(123),
            next_check: Some(456),
            error: None,
        })
        .unwrap();
    store
        .set_json(
            "preferences",
            &Settings {
                limit_kib: 128,
                ..Settings::default()
            },
        )
        .unwrap();
    store
        .set_json("迁移测试", &serde_json::json!({"中文": [1, 2, 3]}))
        .unwrap();
    fs::create_dir(source.join("backups")).unwrap();
    store
        .export_backup(&source.join("backups/既有备份.sqlite3"), &|| false)
        .unwrap();
    store.close().unwrap();
    let locations = Locations {
        default_directory: source.clone(),
        config_file: root.path().join("location.json"),
    };
    (root, locations, source, target)
}

#[test]
fn both_backup_choices_preserve_records_and_only_clean_owned_data_after_startup() {
    for keep in [true, false] {
        let (_root, locations, source, target) = fixture();
        fs::write(source.join("用户文件.txt"), "保留").unwrap();
        fs::create_dir(source.join(".githubsp-续传")).unwrap();
        fs::write(source.join(".githubsp-续传/part-0.bin"), "分片").unwrap();
        locations.migrate(&source, &target, keep).unwrap();
        assert!(source.join("tasks.sqlite3").exists());
        let state = locations.load().unwrap();
        assert_eq!(state.pending.as_ref().unwrap().phase, Phase::Switched);
        assert_eq!(locations.selected(&state), target);
        let new = Store::open(&target.join("tasks.sqlite3")).unwrap();
        assert_eq!(new.settings().unwrap().limit_kib, 128);
        let history = new.recent_terminal().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].task.filename, "中文.zip");
        assert_eq!(history[0].task.revision, 7);
        assert_eq!(new.favorites().unwrap()[0].seen, vec![1, 2, 3]);
        let downloaded = history[0].task.final_path.as_ref().unwrap();
        assert_eq!(fs::read(downloaded).unwrap(), b"downloaded");
        assert_eq!(
            new.get_json::<serde_json::Value>("迁移测试")
                .unwrap()
                .unwrap()["中文"],
            serde_json::json!([1, 2, 3])
        );
        locations.finish().unwrap();
        new.close().unwrap();
        assert_eq!(source.join("tasks.sqlite3").exists(), keep);
        assert_eq!(source.join("backups/既有备份.sqlite3").exists(), keep);
        assert!(target.join("backups/既有备份.sqlite3").exists());
        assert_eq!(fs::read(downloaded).unwrap(), b"downloaded");
        assert_eq!(
            fs::read_to_string(source.join("用户文件.txt")).unwrap(),
            "保留"
        );
        assert_eq!(
            fs::read_to_string(source.join(".githubsp-续传/part-0.bin")).unwrap(),
            "分片"
        );
        assert!(locations.load().unwrap().pending.is_none());
        assert!(locations.finish().unwrap().contains("已更改"));
    }
}

#[test]
fn preflight_failures_do_not_change_config_or_start_copying() {
    use crate::error::{DownloadError, ErrorKind};
    let (_root, locations, source, target) = fixture();
    for kind in [ErrorKind::DiskSpace, ErrorKind::Permission] {
        let failure = validate_target_with(&source, &target, |directory, bytes| {
            assert_eq!(directory, target);
            assert!(bytes > 0);
            if kind == ErrorKind::DiskSpace {
                preflight::ensure_space(Some(0), Some(bytes.saturating_mul(2)))
            } else {
                Err(DownloadError::new(kind, "模拟目录不可写"))
            }
        })
        .unwrap_err();
        assert_eq!(failure.kind, kind);
        if kind == ErrorKind::DiskSpace {
            assert!(failure.message.contains("迁移至少需要"));
        }
        assert!(!locations.config_file.exists());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        assert!(source.join("tasks.sqlite3").exists());
    }
}

#[test]
fn target_validation_rejects_relative_nested_same_and_nonempty_paths() {
    let (root, locations, source, target) = fixture();
    for path in [
        PathBuf::from("relative"),
        source.clone(),
        source.join("child"),
        root.path().to_owned(),
    ] {
        assert!(
            inspect_target(&source, &path).is_err(),
            "{}",
            path.display()
        );
    }
    fs::write(target.join("existing"), "keep").unwrap();
    assert!(locations.migrate(&source, &target, false).is_err());
    assert!(locations.load().unwrap().active.is_none());
    assert_eq!(fs::read_to_string(target.join("existing")).unwrap(), "keep");
    let missing = root.path().join("未创建");
    assert_eq!(
        inspect_target(&source, &missing).unwrap().state,
        preflight::DirectoryState::Missing
    );
    assert!(!missing.exists());
    fs::write(root.path().join("file"), "keep").unwrap();
    assert!(inspect_target(&source, &root.path().join("file/sub")).is_err());
}

#[test]
fn changed_source_or_new_wal_prevents_all_cleanup_but_keeps_target_selected() {
    for added_wal in [true, false] {
        let (_root, locations, source, target) = fixture();
        locations.migrate(&source, &target, false).unwrap();
        if added_wal {
            fs::write(source.join("tasks.sqlite3-wal"), "新增数据").unwrap();
        } else {
            fs::write(source.join("backups/既有备份.sqlite3"), "被修改").unwrap();
        }
        assert!(locations.finish().is_err());
        assert!(source.join("tasks.sqlite3").exists());
        let state = locations.load().unwrap();
        assert_eq!(locations.selected(&state), target);
        assert_eq!(state.pending.unwrap().phase, Phase::Cleanup);
        assert!(locations.rollback("不允许回退").is_err());
    }
}

#[test]
fn interrupted_prepare_and_verified_phases_return_to_source_without_deleting_target() {
    for phase in [Phase::Prepared, Phase::Verified, Phase::Switched] {
        let (_root, locations, source, target) = fixture();
        locations.migrate(&source, &target, false).unwrap();
        let mut state = locations.load().unwrap();
        state.pending.as_mut().unwrap().phase = phase;
        if phase != Phase::Switched {
            state.active = Some(source.clone());
        }
        locations.save(&state).unwrap();
        let restored = locations.rollback("模拟迁移或重启失败").unwrap();
        assert_eq!(locations.selected(&restored), source);
        assert!(restored.pending.is_none());
        assert!(target.join("tasks.sqlite3").exists());
        assert!(source.join("tasks.sqlite3").exists());
    }
}

#[test]
fn cleanup_resumes_after_partial_deletion_and_removes_only_empty_directories() {
    let (_root, locations, source, _target) = fixture();
    let target = _target;
    locations.migrate(&source, &target, false).unwrap();
    let mut state = locations.load().unwrap();
    state.pending.as_mut().unwrap().phase = Phase::Cleanup;
    locations.save(&state).unwrap();
    fs::remove_file(source.join("tasks.sqlite3")).unwrap();
    locations.finish().unwrap();
    assert!(!source.exists());
    assert!(target.join("tasks.sqlite3").exists());
}

#[test]
fn corrupted_configuration_missing_database_and_failed_config_write_never_create_empty_data() {
    let (_root, locations, source, target) = fixture();
    fs::write(&locations.config_file, b"invalid").unwrap();
    assert!(locations.load().is_err());
    assert!(locations.migrate(&source, &target, false).is_err());
    assert!(!target.join("tasks.sqlite3").exists());
    fs::remove_file(&locations.config_file).unwrap();
    fs::create_dir(&locations.config_file).unwrap();
    assert!(locations.migrate(&source, &target, false).is_err());
    assert!(!target.join("tasks.sqlite3").exists());
    let state = State {
        active: Some(target.clone()),
        ..State::default()
    };
    assert!(locations.require_database(&state).is_err());
    assert!(!target.join("tasks.sqlite3").exists());
    assert!(existing_database(&target).is_err());
    let empty = target.join("tasks.sqlite3");
    fs::write(&empty, []).unwrap();
    assert!(existing_database(&target).is_err());
    assert_eq!(fs::metadata(&empty).unwrap().len(), 0);
    let connection = rusqlite::Connection::open(&empty).unwrap();
    connection.execute_batch("VACUUM;").unwrap();
    connection.close().unwrap();
    let before = fs::read(&empty).unwrap();
    assert!(!before.is_empty());
    assert!(existing_database(&target).is_err());
    assert_eq!(fs::read(&empty).unwrap(), before);
}

#[test]
fn cleanup_rejects_untrusted_manifest_paths() {
    let (_root, locations, source, target) = fixture();
    locations.migrate(&source, &target, false).unwrap();
    let mut state = locations.load().unwrap();
    state.pending.as_mut().unwrap().files[0].name = PathBuf::from("../elsewhere.sqlite3");
    assert!(locations.save(&state).is_err());
    assert!(source.join("tasks.sqlite3").exists());
}

#[cfg(windows)]
#[test]
fn migration_copies_between_system_temp_and_workspace_volume() {
    let (_root, locations, source, _unused) = fixture();
    let output =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/verification/relocation-volumes");
    fs::create_dir_all(&output).unwrap();
    let target = tempfile::tempdir_in(&output).unwrap();
    let target_path = files::directory(target.path()).unwrap();
    locations.migrate(&source, &target_path, false).unwrap();
    locations.verify_before_start().unwrap();
    let store = Store::open(&target_path.join("tasks.sqlite3")).unwrap();
    assert_eq!(store.settings().unwrap().limit_kib, 128);
    locations.mark_started().unwrap();
    locations.finish().unwrap();
    store.close().unwrap();
    assert!(!source.exists());
    println!(
        "迁移文件系统根：{:?} -> {:?}",
        source.components().next().unwrap(),
        target_path.components().next().unwrap()
    );
}

#[test]
fn failures_at_publication_boundaries_preserve_original_and_recover_deterministically() {
    for phase in [Phase::Prepared, Phase::Verified, Phase::Switched] {
        let (_root, locations, source, target) = fixture();
        let before = fs::read(source.join("tasks.sqlite3")).unwrap();
        let result = locations.migrate_steps(&source, &target, false, &|at| {
            if phase == at {
                Err(error("模拟写入或提交失败"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(fs::read(source.join("tasks.sqlite3")).unwrap(), before);
        assert_eq!(locations.selected(&locations.load().unwrap()), source);
        locations.rollback("失败恢复").unwrap();
        assert_eq!(locations.selected(&locations.load().unwrap()), source);
    }
}

#[test]
fn competing_target_write_and_changed_or_missing_snapshot_never_replace_user_data() {
    let (_root, locations, source, target) = fixture();
    assert!(locations
        .migrate_steps(&source, &target, false, &|at| {
            if at == Phase::Prepared {
                fs::write(target.join("tasks.sqlite3"), "外部数据")?;
            }
            Ok(())
        })
        .is_err());
    assert_eq!(
        fs::read_to_string(target.join("tasks.sqlite3")).unwrap(),
        "外部数据"
    );
    assert!(source.join("tasks.sqlite3").exists());
    for missing in [false, true] {
        let (_root, locations, source, target) = fixture();
        locations.migrate(&source, &target, false).unwrap();
        if missing {
            fs::remove_file(target.join("tasks.sqlite3")).unwrap();
        } else {
            fs::write(target.join("tasks.sqlite3"), b"not a database").unwrap();
        }
        assert!(locations.verify_before_start().is_err());
        locations.rollback("目标不可用").unwrap();
        assert_eq!(locations.selected(&locations.load().unwrap()), source);
    }
}

#[cfg(windows)]
#[test]
fn directory_guard_prevents_replacement_and_file_lock_prevents_partial_cleanup() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_root, locations, source, target) = fixture();
    let guard = DirectoryGuard::acquire(&target).unwrap();
    assert!(fs::rename(&target, target.with_extension("replaced")).is_err());
    drop(guard);
    locations.migrate(&source, &target, false).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(source.join("tasks.sqlite3"))
        .unwrap();
    assert!(locations.finish().is_err());
    assert!(source.join("tasks.sqlite3").exists());
    drop(held);
    locations.finish().unwrap();
    assert!(!source.exists());
}

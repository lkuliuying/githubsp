use super::*;
use githubsp_lib::{model::Settings, store::Store};
use std::{
    fs,
    path::Path,
    process::{Child, Command},
    time::{Duration, Instant},
};

fn locations(root: &Path) -> Locations {
    Locations {
        default_directory: root.join("原目录"),
        config_file: root.join("位置.json"),
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.0.try_wait().is_ok_and(|s| s.is_some()) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn spawn(root: &Path, mode: &str, wait: Option<u32>) -> OwnedChild {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "data_location::tests::isolated_process_helper",
            "--nocapture",
        ])
        .env("GITHUBSP_RELOCATION_TEST_ROOT", root)
        .env("GITHUBSP_RELOCATION_TEST_MODE", mode)
        .creation_flags(0x08000000);
    if let Some(pid) = wait {
        command.env("GITHUBSP_RELOCATION_WAIT_PID", pid.to_string());
    }
    OwnedChild(command.spawn().unwrap())
}

fn until(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < end, "隔离进程未按时完成");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn restart_waits_for_previous_process_and_uses_injected_location_before_cleanup() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("test-owner"), "GitHubSP 迁移隔离验收").unwrap();
    let locations = locations(root.path());
    let target = root.path().join("目标目录");
    fs::create_dir(&locations.default_directory).unwrap();
    fs::create_dir(&target).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut api = runtime
        .block_on(Service::start(&locations.default_directory))
        .unwrap();
    Arc::get_mut(&mut api).unwrap().locations = Some(locations.clone());
    runtime
        .block_on(api.manager.settings(Settings {
            limit_kib: 384,
            ..Settings::default()
        }))
        .unwrap();
    api.closing.store(true, Ordering::Release);
    runtime
        .block_on(relocate(&api, target.clone(), false))
        .unwrap();
    assert!(platform::DirectoryLock::acquire(&target).is_err());
    assert!(locations.default_directory.join("tasks.sqlite3").exists());
    api.migration_lock.lock().unwrap().take();
    drop(api);
    let mut blocker = spawn(root.path(), "blocker", None);
    until(|| root.path().join("blocker-ready").exists());
    let mut next = spawn(root.path(), "start", Some(blocker.0.id()));
    std::thread::sleep(Duration::from_millis(150));
    assert!(next.0.try_wait().unwrap().is_none());
    assert!(!root.path().join("started").exists());
    assert!(locations.default_directory.join("tasks.sqlite3").exists());
    fs::write(root.path().join("release-blocker"), b"exit").unwrap();
    until(|| blocker.0.try_wait().unwrap().is_some());
    assert!(blocker.0.wait().unwrap().success());
    until(|| next.0.try_wait().unwrap().is_some());
    assert!(next.0.wait().unwrap().success());
    assert!(root.path().join("started").exists());
    assert!(!locations.default_directory.exists());
    assert!(locations.load().unwrap().pending.is_none());
}

#[test]
#[ignore = "仅由迁移隔离进程验收启动，不访问正式应用目录"]
fn isolated_process_helper() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GITHUBSP_RELOCATION_TEST_ROOT").expect("需要隔离目录"),
    );
    assert_eq!(
        fs::read_to_string(root.join("test-owner")).unwrap(),
        "GitHubSP 迁移隔离验收"
    );
    let locations = locations(&root);
    let startup = platform::resolve_directory(None, || Ok(locations.clone())).unwrap();
    let directory = startup.directory;
    match std::env::var("GITHUBSP_RELOCATION_TEST_MODE")
        .unwrap()
        .as_str()
    {
        "blocker" => {
            let _lock = platform::DirectoryLock::acquire(&directory).unwrap();
            fs::write(root.join("blocker-ready"), b"ready").unwrap();
            until(|| root.join("release-blocker").exists());
        }
        "start" => {
            platform::wait_for_exit(
                std::env::var("GITHUBSP_RELOCATION_WAIT_PID")
                    .unwrap()
                    .parse()
                    .unwrap(),
            )
            .unwrap();
            let _target = platform::DirectoryLock::acquire(&directory).unwrap();
            let _source = platform::DirectoryLock::acquire(&locations.default_directory).unwrap();
            locations.verify_before_start().unwrap();
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let api = runtime.block_on(Service::start(&directory)).unwrap();
            assert_eq!(
                runtime
                    .block_on(api.manager.snapshot())
                    .unwrap()
                    .settings
                    .limit_kib,
                384
            );
            locations.mark_started().unwrap();
            locations.finish().unwrap();
            runtime.block_on(api.shutdown()).unwrap();
            fs::write(root.join("started"), b"ready").unwrap();
        }
        _ => panic!("未知隔离验收模式"),
    }
}

#[test]
fn failed_target_start_rolls_back_only_before_new_directory_was_used() {
    let root = tempfile::tempdir().unwrap();
    let locations = locations(root.path());
    let target = root.path().join("目标");
    fs::create_dir(&locations.default_directory).unwrap();
    fs::create_dir(&target).unwrap();
    Store::open(&locations.default_directory.join("tasks.sqlite3"))
        .unwrap()
        .close()
        .unwrap();
    locations
        .migrate(&locations.default_directory, &target, false)
        .unwrap();
    let in_use = platform::DirectoryLock::acquire(&target).unwrap();
    assert!(rollback_failed_start(&locations, "另一实例正在启用目标").is_err());
    assert_eq!(
        locations.selected(&locations.load().unwrap()),
        githubsp_lib::relocation::existing_database(&target).unwrap()
    );
    drop(in_use);
    let source = platform::DirectoryLock::acquire(&locations.default_directory).unwrap();
    assert!(rollback_failed_start(&locations, "模拟启动失败").is_err());
    drop(source);
    assert!(rollback_failed_start(&locations, "模拟启动失败").unwrap());
    assert!(locations.default_directory.join("tasks.sqlite3").exists());
    assert!(target.join("tasks.sqlite3").exists());
    assert!(!rollback_failed_start(&locations, "没有待恢复迁移").unwrap());

    let next = root.path().join("再次迁移");
    fs::create_dir(&next).unwrap();
    locations
        .migrate(&locations.default_directory, &next, false)
        .unwrap();
    locations.mark_started().unwrap();
    assert!(!rollback_failed_start(&locations, "新目录已投入使用").unwrap());
    assert_eq!(
        locations.selected(&locations.load().unwrap()),
        githubsp_lib::relocation::existing_database(&next).unwrap()
    );
    let _source = platform::DirectoryLock::acquire(&locations.default_directory).unwrap();
    let error = finish_with_source_lock(&locations, &locations.default_directory).unwrap_err();
    assert!(error.contains(locations.default_directory.to_str().unwrap()));
    assert!(locations.default_directory.join("tasks.sqlite3").exists());
}

use super::*;

fn fixture() -> (
    tokio::runtime::Runtime,
    tempfile::TempDir,
    Arc<Service>,
    MainWindow,
) {
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        threading: true,
        mock_time: false,
        renderer_name: Some("software".into()),
    })))
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let service = runtime.block_on(Service::start(directory.path())).unwrap();
    let window = MainWindow::new().unwrap();
    presentation::bind_detail_selection(&window);
    controller::bind(&window, &service, runtime.handle());
    presentation::apply(
        &window,
        &runtime.block_on(service.manager.snapshot()).unwrap(),
    );
    window.global::<Motion>().set_reduce(true);
    window.global::<Workspace>().invoke_navigate(3);
    window
        .global::<Workspace>()
        .invoke_select_settings_section(0);
    (runtime, directory, service, window)
}

#[test]
#[ignore = "由 verify-native-ui.ps1 在独立进程中生成固定视觉场景"]
fn automation_visual_scenarios() {
    let (runtime, _directory, service, window) = fixture();
    window.show().unwrap();
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "settings-default", width, height);
        for label in ["保存全部设置", "恢复全部默认", "下载设置"] {
            assert_button_in_window(&window, label, width, height);
        }
    }
    window.hide().unwrap();
    runtime.block_on(service.shutdown()).unwrap();
}

#[cfg(feature = "automation-mcp")]
#[test]
#[ignore = "由自动化入口启动的隔离软件渲染 MCP 会话，不访问正式数据"]
fn automation_mcp_session() {
    let (runtime, _directory, service, window) = fixture();
    // 手动设置测试后端时，不经过正式后端选择器的 MCP 初始化入口。
    i_slint_backend_testing::mcp_server::init().unwrap();
    window.show().unwrap();
    let duration = std::env::var("GITHUBSP_MCP_SESSION_SECONDS")
        .unwrap_or_else(|_| "600".into())
        .parse::<u64>()
        .expect("MCP 会话时长必须是整数秒");
    assert!((1..=3600).contains(&duration));
    slint::Timer::single_shot(Duration::from_secs(duration), || {
        slint::quit_event_loop().unwrap();
    });
    // 设置保存的异步结果仍通过正式绑定更新，定时器仅驱动隔离会话刷新。
    let refresh = slint::Timer::default();
    let weak = window.as_weak();
    let mut state = service.state.clone();
    let stop_file = std::env::var_os("GITHUBSP_MCP_STOP_FILE").map(std::path::PathBuf::from);
    refresh.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            if stop_file.as_ref().is_some_and(|path| path.exists()) {
                slint::quit_event_loop().unwrap();
                return;
            }
            if !state.has_changed().unwrap_or(false) {
                return;
            }
            if let (Some(window), Some(snapshot)) =
                (weak.upgrade(), state.borrow_and_update().clone())
            {
                presentation::apply(&window, &snapshot);
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
    window.hide().unwrap();
    runtime.block_on(service.shutdown()).unwrap();
}

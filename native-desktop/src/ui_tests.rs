use crate::{
    controller, presentation, service::Service, AboutWindow, CompletionWindow, Favorites, History,
    MainWindow, Preferences, Sources, Storage, Updates, Workspace,
};
use githubsp_lib::{
    model::{
        DiagnosticContext, DiagnosticSource, Favorite, ReleaseSummary, RouteReport, StoredTask,
        Task, TaskDetails, TaskStatus, Verification,
    },
    store::Store,
};
use i_slint_backend_testing::{ElementHandle, TestingBackend, TestingBackendOptions};
use slint::{ComponentHandle, Model};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct CatalogFixture {
    address: String,
    stop: Arc<AtomicBool>,
    slow_requested: Arc<AtomicBool>,
    release_slow: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl CatalogFixture {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}/", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let slow_requested = Arc::new(AtomicBool::new(false));
        let release_slow = Arc::new(AtomicBool::new(false));
        let (ending, requested, release) =
            (stop.clone(), slow_requested.clone(), release_slow.clone());
        let worker = std::thread::spawn(move || {
            while !ending.load(Ordering::Acquire) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("本地目录服务无法接收请求：{error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..count]);
                let path = request.split_whitespace().nth(1).unwrap();
                let segments: Vec<_> = path.split('/').collect();
                let repository = format!("{}/{}", segments[2], segments[3]);
                if repository == "test/slow" {
                    requested.store(true, Ordering::Release);
                    while !release.load(Ordering::Acquire) && !ending.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
                if repository == "sample/project" {
                    let releases: Vec<_> = ["v1.0.0", "v0.9.2", "v0.9.1"]
                        .iter()
                        .enumerate()
                        .map(|(index, tag)| {
                            let assets: Vec<_> = [
                                ("project-windows-x64.zip", 86_300_000),
                                ("project-linux-x64.tar.gz", 82_100_000),
                                ("checksums.txt", 1200),
                            ]
                            .iter()
                            .enumerate()
                            .map(|(asset_index, (name, size))| {
                                serde_json::json!({
                                    "id": index * 10 + asset_index + 1, "name": name, "size": size,
                                    "browser_download_url": format!("https://github.com/{repository}/releases/download/{tag}/{name}")
                                })
                            })
                            .collect();
                            serde_json::json!({
                                "id": index + 1, "tag_name": tag, "name": tag,
                                "html_url": format!("https://github.com/{repository}/releases/tag/{tag}"),
                                "body": "布局验收样例，不访问外部网络。", "assets": assets
                            })
                        })
                        .collect();
                    let (status, body) = if path.contains("page=2") {
                        ("503 Service Unavailable", "{}".to_owned())
                    } else if path.contains("/tags/") {
                        ("200 OK", serde_json::to_string(&releases[0]).unwrap())
                    } else {
                        ("200 OK", serde_json::to_string(&releases).unwrap())
                    };
                    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    continue;
                }
                let body = serde_json::json!({
                    "id": 1, "tag_name": "v1.0.0", "name": "本地测试版本",
                    "html_url": format!("https://github.com/{repository}/releases/tag/v1.0.0"),
                    "body": "本地目录测试，不访问 GitHub。",
                    "assets": [{"id": 10, "name": "fixture.zip", "size": 4096,
                        "browser_download_url": format!("https://github.com/{repository}/releases/download/v1.0.0/fixture.zip")}]
                }).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        Self {
            address,
            stop,
            slow_requested,
            release_slow,
            worker: Some(worker),
        }
    }
}

impl Drop for CatalogFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn seed(path: &Path) {
    let store = Store::open(&path.join("tasks.sqlite3")).unwrap();
    // 与实际创建任务一致，避免临时路径的前缀或短名影响重复任务识别。
    let directory = githubsp_lib::preflight::inspect_directory(path)
        .unwrap()
        .directory;
    for index in 0..64 {
        let task = Task {
            id: format!("00000000-0000-4000-8000-{index:012}"),
            url: format!("https://github.com/test/project/releases/download/v1/中文附件-{index}-windows-x64.zip"),
            filename: format!("中文附件-{index}-windows-x64.zip"),
            directory: directory.clone(),
            status: if index == 0 {
                TaskStatus::Paused
            } else {
                TaskStatus::Completed
            },
            downloaded: 125_000_000,
            total: Some(500_000_000),
            speed: 0.0,
            eta: None,
            route: Some("GH-Proxy".into()),
            verification: Verification::Unverified,
            error: None,
            final_path: None,
            created_at: 1_790_000_000_000 + index as u64,
            revision: 0,
            details: TaskDetails {
                repository: Some("test/project".into()),
                tag: Some("v1.0.0".into()),
                elapsed_ms: Some(120_000),
                ..TaskDetails::default()
            },
        };
        store
            .save(&StoredTask {
                task,
                checkpoint: None,
            })
            .unwrap();
    }
    for index in 0..4 {
        store
            .save_favorite(&Favorite {
                id: format!("favorite-{index}"),
                repository: format!("test/project-{index}"),
                latest: Some(ReleaseSummary {
                    id: 1,
                    tag: "v1.2.3".into(),
                    url: format!("https://github.com/test/project-{index}/releases/tag/v1.2.3"),
                }),
                seen: vec![1],
                last_checked: Some(1_790_000_000_000),
                last_success: Some(1_790_000_000_000),
                next_check: None,
                error: None,
            })
            .unwrap();
    }
}

async fn ready(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < deadline, "界面异步操作未在 5 秒内结束");
        let (send, receive) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(10), move || {
            let _ = send.send(());
        });
        receive.await.unwrap();
    }
}

fn capture(window: &impl ComponentHandle, name: &str, width: u32, height: u32) {
    window
        .window()
        .set_size(slint::PhysicalSize::new(width, height));
    slint::platform::update_timers_and_animations();
    let pixels = window.window().take_snapshot().unwrap();
    assert_eq!((pixels.width(), pixels.height()), (width, height));
    let output = std::env::var_os("GITHUBSP_UI_RENDER_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/verification/ui-renders")
        });
    std::fs::create_dir_all(&output).unwrap();
    image::save_buffer(
        output.join(format!("{name}-{width}x{height}.png")),
        pixels.as_bytes(),
        width,
        height,
        image::ColorType::Rgba8,
    )
    .unwrap();
}

fn navigate(window: &MainWindow, label: &str) {
    let button = ElementHandle::find_by_accessible_label(window, label)
        .find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button))
        .expect("应有原页面导航按钮");
    let position = button.absolute_position();
    assert!(position.y >= 0.0 && position.y < 140.0, "导航应固定在顶部");
    button.invoke_accessible_default_action();
}

fn scroll_to_bottom(window: &MainWindow) {
    ElementHandle::find_by_element_id(window, "MainWindow::main-scroll")
        .next()
        .unwrap()
        .scroll(0.0, -10_000.0);
    slint::platform::update_timers_and_animations();
}

fn assert_button_in_window(window: &impl ComponentHandle, label: &str, width: u32, height: u32) {
    let button = ElementHandle::find_by_accessible_label(window, label)
        .find(|item| {
            item.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .unwrap_or_else(|| panic!("找不到可见按钮 {label}"));
    let position = button.absolute_position();
    let size = button.size();
    assert!(
        position.x >= 0.0 && position.x + size.width <= width as f32,
        "按钮 {label} 超出窗口水平边界：{position:?} {size:?}"
    );
    assert!(
        position.y >= 0.0 && position.y + size.height <= height as f32,
        "按钮 {label} 应能滚动到可视区：{position:?} {size:?}"
    );
}

fn scroll_to_button(window: &MainWindow, label: &str, height: u32) {
    let workspace = window.global::<Workspace>();
    let area = if workspace.get_page() == 3 {
        "SettingsPage::settings-scroll"
    } else if workspace.get_download_section() == 1 {
        "NewDownloadPanel::new-scroll"
    } else {
        "QueuePanel::queue-list"
    };
    let viewport = ElementHandle::find_by_element_id(window, area)
        .next()
        .unwrap();
    let viewport_top = viewport.absolute_position().y;
    let viewport_bottom = (viewport_top + viewport.size().height).min(height as f32 - 43.0);
    viewport.scroll(0.0, 100_000.0);
    window.window().take_snapshot().unwrap();
    let step = (viewport.size().height / 2.0).clamp(20.0, 150.0);
    for _ in 0..30 {
        if ElementHandle::find_by_accessible_label(window, label).any(|button| {
            let position = button.absolute_position();
            position.y >= viewport_top && position.y + button.size().height <= viewport_bottom
        }) {
            return;
        }
        ElementHandle::find_by_element_id(window, area)
            .next()
            .unwrap()
            .scroll(0.0, -step);
        slint::platform::update_timers_and_animations();
        window.window().take_snapshot().unwrap();
    }
    panic!("滚动后应能看到按钮 {label}");
}

fn verify_boundary_layouts(window: &MainWindow) {
    let workspace = window.global::<Workspace>();
    workspace.invoke_navigate(0);
    let original = workspace.get_tasks();
    let mut recovery = original.row_data(0).unwrap();
    recovery.filename =
        "很长的中文附件名称-with-a-long-version-number-v2026.10.08-windows-x86_64-portable.zip"
            .into();
    recovery.id = "layout-fixture".into();
    recovery.state = "等待线路恢复".into();
    recovery.failure = "线路暂时不可用，下载进度已保留。".repeat(4).into();
    recovery.feedback = "将在限定时间内重新检测线路，也可以立即重试、暂停或取消。".into();
    recovery.source_url = "https://github.com/test/project/releases/download/v1/fixture.zip".into();
    recovery.save_path = "F:\\测试目录\\包含很长中文名称的下载位置".into();
    recovery.size = "未知大小".into();
    recovery.percent = "—".into();
    recovery.suggestion = "候选线路可能更快，换线会重新下载全部内容，需要确认。".into();
    recovery.suggestion_id = "layout-suggestion".into();
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        workspace.set_tasks(presentation::model(vec![]));
        capture(window, "queue-empty", width, height);
        assert_button_in_window(window, "新建下载", width, height);
        workspace.set_tasks(presentation::model(vec![recovery.clone()]));
        workspace.set_expanded_task(recovery.id.clone());
        capture(window, "queue-recovery", width, height);
        scroll_to_button(window, "复制来源链接", height);
        capture(window, "queue-details", width, height);
        for label in ["另选目录重新下载", "复制来源链接", "收藏项目", "全部暂停"]
        {
            assert_button_in_window(window, label, width, height);
        }
    }
    workspace.set_expanded_task("".into());
    workspace.set_tasks(original);
}

async fn verify_module_flows(window: &MainWindow, api: &Arc<Service>, fixture: &CatalogFixture) {
    let workspace = window.global::<Workspace>();
    let sources = window.global::<Sources>();
    let settings = window.global::<Preferences>();
    workspace.invoke_navigate(0);
    workspace.invoke_select_download_section(1);
    workspace.set_directory(api.data_directory.to_string_lossy().into_owned().into());
    workspace.set_url("https://github.com/test/project/releases/download/v1.0.0/direct.zip".into());
    sources.set_batch_input("尚未提交的批量草稿".into());
    sources.set_repository("test/project".into());
    sources.invoke_edited();
    for mode in [1, 2, 0] {
        workspace.invoke_select_creation_mode(mode);
    }
    workspace.invoke_select_download_section(2);
    workspace.set_diagnostic_input("https://github.com/test/project".into());
    workspace.invoke_diagnose();
    ready(|| !workspace.get_diagnosing()).await;
    assert!(
        !workspace.get_message().is_empty(),
        "检测应独立校验输入，不能使用直链草稿"
    );
    assert!(workspace.get_url().ends_with("/direct.zip"));
    assert_eq!(sources.get_batch_input(), "尚未提交的批量草稿");
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "diagnostic-input-error", width, height);
        let error = ElementHandle::find_by_element_id(window, "MainWindow::error-message")
            .next()
            .expect("输入校验错误必须实际显示");
        assert!(error.size().width > 200.0 && error.size().height >= 24.0);
        assert!(
            ElementHandle::find_by_accessible_label(window, workspace.get_message().as_str())
                .next()
                .is_some(),
            "错误提示不能只有关闭按钮而没有文字"
        );
        assert_button_in_window(window, "关闭错误提示", width, height);
        assert_button_in_window(window, "检测线路", width, height);
    }
    workspace.set_message("".into());
    workspace.invoke_select_download_section(1);
    workspace.invoke_select_creation_mode(1);
    sources.invoke_browse(0);
    ready(|| !sources.get_busy()).await;
    assert!(
        sources.get_loaded(),
        "本地目录响应应绑定到附件模块：{}",
        sources.get_message()
    );
    assert_eq!(sources.get_assets().row_count(), 1);
    let attachment = sources.get_assets().row_data(0).unwrap().url;
    sources.invoke_select_asset(attachment.clone(), true);
    assert_eq!(sources.get_selected_size(), "4.1 KB");
    sources.invoke_preview_batch(true);
    ready(|| !sources.get_busy()).await;
    assert!(sources.get_preview_visible());
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "batch-preview", width, height);
        for label in ["返回修改", "创建 1 个有效任务"] {
            assert_button_in_window(window, label, width, height);
        }
    }
    sources.invoke_dismiss_preview();
    assert_eq!(sources.get_selected_count(), 1);
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "repository-populated", width, height);
    }

    // 对结果呈现边界注入已知结果，实际任务创建和传输仍由核心测试验证。
    let created = githubsp_lib::intake::BatchItem {
        input: attachment.to_string(),
        url: Some(attachment.to_string()),
        filename: Some("fixture.zip".into()),
        size: Some(4096),
        status: "created".into(),
        message: None,
        task_id: Some("fixture-id".into()),
    };
    let failed = githubsp_lib::intake::BatchItem {
        input: "https://github.com/test/project/releases/download/v1.0.0/failed.zip".into(),
        status: "failed".into(),
        message: Some("测试创建失败".into()),
        ..created.clone()
    };
    let mut result = githubsp_lib::intake::BatchResult {
        items: vec![created.clone(), failed.clone()],
        snapshot: api.manager.snapshot().await.unwrap(),
    };
    let remaining = crate::sources::complete_submission(window, &result, vec![], true);
    assert_eq!(remaining, vec![failed.input.clone()]);
    assert_eq!(workspace.get_download_section(), 1);
    assert_eq!(sources.get_batch_input(), "尚未提交的批量草稿");
    result.items = vec![created.clone()];
    crate::sources::complete_submission(window, &result, vec![], true);
    assert_eq!(workspace.get_download_section(), 0);
    assert_eq!(sources.get_batch_input(), "尚未提交的批量草稿");
    workspace.invoke_select_download_section(1);
    workspace.invoke_select_creation_mode(2);
    result.items = vec![created.clone(), failed.clone()];
    crate::sources::complete_submission(window, &result, vec!["无效链接".into()], false);
    assert_eq!(workspace.get_download_section(), 1);
    assert_eq!(
        sources.get_batch_input(),
        format!("{}\n无效链接", failed.input)
    );
    result.items = vec![created];
    crate::sources::complete_submission(window, &result, vec![], false);
    assert_eq!(workspace.get_download_section(), 0);
    assert!(sources.get_batch_input().is_empty());
    assert!(workspace.get_queue_message().contains("已创建 1 个任务"));
    workspace.set_queue_message("".into());

    workspace.invoke_select_download_section(1);
    workspace.invoke_select_creation_mode(1);
    sources.set_repository("test/slow".into());
    sources.invoke_edited();
    sources.invoke_browse(0);
    ready(|| fixture.slow_requested.load(Ordering::Acquire)).await;
    workspace.invoke_select_download_section(2);
    workspace.invoke_select_download_section(1);
    fixture.release_slow.store(true, Ordering::Release);
    ready(|| !sources.get_busy()).await;
    assert!(
        !sources.get_loaded(),
        "离开再返回后，过期目录结果仍不得写入"
    );
    assert_eq!(workspace.get_download_section(), 1);

    workspace.invoke_navigate(2);
    window
        .global::<Favorites>()
        .invoke_action("test/project-0".into(), "browse".into());
    assert_eq!(
        (
            workspace.get_page(),
            workspace.get_download_section(),
            workspace.get_creation_mode()
        ),
        (0, 1, 1)
    );
    ready(|| !sources.get_busy()).await;
    assert!(sources.get_loaded());
    verify_boundary_layouts(window);
    workspace.invoke_navigate(3);
    workspace.invoke_select_settings_section(0);
    window
        .window()
        .set_size(slint::PhysicalSize::new(1040, 740));
    window.window().take_snapshot().unwrap();
    let unlimited = ElementHandle::find_by_accessible_label(window, "不限速")
        .find(|element| element.accessible_checked().is_some())
        .unwrap();
    let slider = ElementHandle::find_by_accessible_label(window, "下载限速")
        .next()
        .unwrap();
    let input = ElementHandle::find_by_accessible_label(window, "0")
        .find(|element| element.accessible_value().is_some())
        .unwrap();
    for value in ["0", "0.0", " 0 "] {
        input.set_accessible_value(value);
        assert!(
            settings.get_unlimited(),
            "数值为零时必须统一为不限速：{value}"
        );
        assert_eq!(unlimited.accessible_checked(), Some(true));
        assert_eq!(settings.get_slider(), 0.0);
    }
    slider.set_accessible_value("12.5");
    assert_eq!(settings.get_limit(), "12.5");
    assert!(!settings.get_unlimited());
    assert_eq!(unlimited.accessible_checked(), Some(false));
    capture(window, "settings-limit-custom", 1040, 740);
    unlimited.invoke_accessible_default_action();
    assert_eq!(settings.get_limit(), "0");
    assert!(settings.get_unlimited());
    assert_eq!(slider.accessible_value().as_deref(), Some("0"));
    unlimited.invoke_accessible_default_action();
    assert_eq!(settings.get_limit(), "12.5");
    assert!(!settings.get_unlimited());
    input.set_accessible_value("0");
    assert_eq!(unlimited.accessible_checked(), Some(true));
    assert_eq!(slider.accessible_value().as_deref(), Some("0"));
    capture(window, "settings-limit-reset", 1040, 740);
    settings.set_limit("3.25".into());
    settings.invoke_edited();
    settings.set_close_to_tray(true);
    settings.set_completion(false);
    settings.set_auto_check(true);
    for section in [1, 2, 3, 4, 0] {
        workspace.invoke_select_settings_section(section);
    }
    workspace.invoke_navigate(0);
    workspace.invoke_navigate(3);
    assert_eq!(settings.get_limit(), "3.25");
    assert!(settings.get_close_to_tray());
    assert!(!settings.get_completion());
    assert!(settings.get_auto_check());
    let persisted = api.manager.snapshot().await.unwrap().settings;
    assert_eq!(persisted.limit_kib, 0);
    assert!(!persisted.close_to_tray, "切换模块不得自动保存草稿");
    settings.invoke_reset();
    workspace.invoke_select_settings_section(1);
    capture(window, "settings-keyboard", 720, 520);
    let navigation = ElementHandle::find_by_accessible_label(window, "窗口与提醒")
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .unwrap();
    let position = navigation.absolute_position();
    let size = navigation.size();
    let position = slint::LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
    for key in [slint::platform::Key::Tab, slint::platform::Key::Return] {
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.into() });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key.into() });
    }
    assert_eq!(
        settings.get_section(),
        2,
        "Tab 后按回车应进入下一个设置模块"
    );
}

fn scroll_confirmation_page(window: &MainWindow, viewport: &ElementHandle, delta_y: f32) {
    let position = viewport.absolute_position();
    let size = viewport.size();
    // 在右侧留白滚动外层正文，避免滚轮落入独立的附件列表。
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(
                position.x + size.width - 18.0,
                position.y + size.height / 2.0,
            ),
            delta_x: 0.0,
            delta_y,
        });
    window.window().take_snapshot().unwrap();
}

fn assert_regions_separate(window: &MainWindow, first: &str, second: &str) {
    let area = if window.global::<Workspace>().get_page() == 3 {
        "SettingsPage::settings-scroll"
    } else {
        "NewDownloadPanel::new-scroll"
    };
    let viewport = ElementHandle::find_by_element_id(window, area)
        .next()
        .unwrap();
    let scroll = |delta| {
        if window.global::<Sources>().get_preview_visible() {
            scroll_confirmation_page(window, &viewport, delta);
        } else {
            viewport.scroll(0.0, delta);
        }
    };
    // 测试后端只枚举可见元素，窄窗口需先滚动获取两块区域的句柄。
    let find = |id: &str| {
        for _ in 0..30 {
            if let Some(element) = ElementHandle::find_by_element_id(window, id).next() {
                return element;
            }
            scroll(-(viewport.size().height / 2.0).max(20.0));
            window.window().take_snapshot().unwrap();
        }
        panic!("滚动后应能找到区域 {id}");
    };
    let a = find(first);
    let b = find(second);
    let (a_pos, b_pos, a_size, b_size) = (
        a.absolute_position(),
        b.absolute_position(),
        a.size(),
        b.size(),
    );
    assert!(
        a_pos.x + a_size.width <= b_pos.x + 1.0 || a_pos.y + a_size.height <= b_pos.y + 1.0,
        "区域 {first} 和 {second} 不得重叠：{a_pos:?} {a_size:?} / {b_pos:?} {b_size:?}"
    );
    scroll(100_000.0);
    window.window().take_snapshot().unwrap();
}

async fn verify_direct_and_confirmation_layouts(window: &MainWindow, api: &Arc<Service>) {
    let workspace = window.global::<Workspace>();
    let sources = window.global::<Sources>();
    let previous_page = workspace.get_page();
    let previous_url = workspace.get_url();
    let previous_directory = workspace.get_directory();
    let previous_batch = sources.get_batch_input();
    let previous_route = sources.get_route();
    workspace.invoke_navigate(0);
    workspace.invoke_select_download_section(1);
    workspace.invoke_select_creation_mode(0);
    let url = "https://github.com/sample/project/releases/download/v1.0.0/project-windows-x64.zip";
    workspace.set_url(url.into());
    workspace.set_directory("D:\\Downloads\\GitHubSP".into());
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "compact-direct", width, height);
        assert_regions_separate(
            window,
            "DirectDownloadContent::form",
            "DirectDownloadContent::guide",
        );
        assert_button_in_window(window, "添加下载任务", width, height);
        scroll_to_button(window, "浏览", height);
        assert_button_in_window(window, "浏览", width, height);
    }
    workspace.set_url("".into());
    assert_eq!(
        ElementHandle::find_by_accessible_label(window, "添加下载任务")
            .next()
            .unwrap()
            .accessible_enabled(),
        Some(false),
        "空链接不可创建任务"
    );
    workspace.set_url(url.into());
    workspace.invoke_select_creation_mode(2);
    workspace.set_directory(api.data_directory.to_string_lossy().as_ref().into());
    let existing = api
        .manager
        .snapshot()
        .await
        .unwrap()
        .tasks
        .into_iter()
        .find(|task| task.status == TaskStatus::Paused)
        .unwrap();
    let expected_directory = githubsp_lib::preflight::inspect_directory(&api.data_directory)
        .unwrap()
        .directory;
    assert_eq!(existing.directory, expected_directory);
    let batch = format!("{url}\nhttps://github.com/sample/project/releases/download/v1.0.0/project-linux-x64.tar.gz\nhttps://github.com/sample/project/releases/download/v1.0.0/checksums.txt\n{}\nhttps://example.com/archive.zip", existing.url);
    sources.set_batch_input(batch.clone().into());
    sources.invoke_edited();
    sources.invoke_preview_batch(false);
    ready(|| !sources.get_busy()).await;
    assert!(sources.get_preview_visible());
    assert!(sources.get_can_submit());
    assert_eq!(
        (
            sources.get_preview_valid_count(),
            sources.get_preview_duplicate_count(),
            sources.get_preview_invalid_count()
        ),
        (3, 1, 1)
    );
    assert_eq!(sources.get_preview_known_size(), "168 MB");
    assert_eq!(sources.get_preview_unknown_count(), 0);
    assert_eq!(
        Path::new(sources.get_preview_directory().as_str()),
        expected_directory
    );
    assert!(!sources.get_preview_available_space().is_empty());
    let rows: Vec<_> = sources.get_preview().iter().collect();
    assert_eq!(rows[0].size, "86.3 MB");
    assert_eq!(rows[2].size, "1.2 KB");
    assert_eq!(rows[3].task_id, existing.id);
    assert_eq!(rows[3].tone, 3);
    assert_eq!(rows[4].tone, 2);
    assert!(!rows[4].detail.is_empty());

    // 固定图例便于与所选设计图比较，前面的断言已验证真实预览数据映射。
    sources.set_preview_directory("D:\\Downloads\\GitHubSP".into());
    sources.set_preview_available_space("42.2 GB".into());
    let mut illustrated = rows.clone();
    illustrated[3].name = "project-windows-arm64.zip".into();
    illustrated[3].detail = "存在未完成任务".into();
    illustrated[4].detail = "不支持的链接，请返回修改".into();
    sources.set_preview(presentation::model(illustrated));
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "compact-confirmation", width, height);
        assert_regions_separate(
            window,
            "BatchConfirmation::files",
            "BatchConfirmation::settings",
        );
        for label in ["返回修改", "创建 3 个有效任务"] {
            assert_button_in_window(window, label, width, height);
        }
        let viewport = ElementHandle::find_by_element_id(window, "NewDownloadPanel::new-scroll")
            .next()
            .unwrap();
        scroll_confirmation_page(window, &viewport, -100_000.0);
        capture(window, "compact-confirmation-settings", width, height);
        let picker = ElementHandle::find_by_element_id(window, "PreviewSettings::route-picker")
            .next()
            .expect("滚动后下载线路必须可达");
        assert!(picker.absolute_position().y + picker.size().height <= height as f32 - 65.0);
        assert_button_in_window(window, "创建 3 个有效任务", width, height);
        scroll_confirmation_page(window, &viewport, 100_000.0);
    }
    sources.set_busy(true);
    assert_eq!(
        ElementHandle::find_by_accessible_label(window, "创建 3 个有效任务")
            .next()
            .unwrap()
            .accessible_enabled(),
        Some(false)
    );
    sources.set_busy(false);
    sources.set_route(2);
    ElementHandle::find_by_accessible_label(window, "返回修改")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    assert!(!sources.get_preview_visible());
    assert_eq!(sources.get_batch_input(), batch);
    assert_eq!(sources.get_route(), 2);
    assert_eq!(workspace.get_url(), url);

    sources.set_batch_input(
        "https://github.com/test/project/releases/download/v1.0.0/unknown-size.zip".into(),
    );
    sources.invoke_edited();
    sources.invoke_preview_batch(false);
    ready(|| !sources.get_busy()).await;
    assert!(sources.get_can_submit());
    assert_eq!(sources.get_preview_unknown_count(), 1);
    assert_eq!(sources.get_preview_known_size(), "0 B");
    assert_eq!(sources.get_preview().row_data(0).unwrap().size, "未知");
    capture(window, "compact-confirmation-unknown", 1040, 740);

    // 百项列表与超长文本只测试呈现边界，不创建下载或发起外网请求。
    let many: Vec<_> = (0..100)
        .map(|index| {
            let mut row = rows[0].clone();
            row.name = format!("批量附件-{index:03}-windows-x64.zip").into();
            row
        })
        .collect();
    sources.set_preview(presentation::model(many));
    sources.set_preview_valid_count(100);
    sources.set_preview_duplicate_count(0);
    sources.set_preview_invalid_count(0);
    sources.set_preview_unknown_count(0);
    sources.set_preview_known_size("8.6 GB".into());
    capture(window, "compact-confirmation-many", 720, 520);
    let list = ElementHandle::find_by_element_id(window, "BatchConfirmation::preview-list")
        .next()
        .unwrap();
    for _ in 0..3 {
        list.scroll(0.0, -100_000.0);
        window.window().take_snapshot().unwrap();
    }
    let last = ElementHandle::find_by_accessible_label(window, "批量附件-099-windows-x64.zip")
        .next()
        .expect("列表末项应可滚动到");
    assert!(last.absolute_position().y >= list.absolute_position().y);
    assert!(
        last.absolute_position().y + last.size().height
            <= list.absolute_position().y + list.size().height
    );
    capture(window, "compact-confirmation-many-bottom", 720, 520);
    assert_button_in_window(window, "创建 100 个有效任务", 720, 520);
    sources.invoke_dismiss_preview();
    sources.set_batch_input("https://example.com/archive.zip".into());
    sources.invoke_edited();
    sources.invoke_preview_batch(false);
    ready(|| !sources.get_busy()).await;
    assert_eq!(sources.get_preview_valid_count(), 0);
    assert_eq!(sources.get_preview_invalid_count(), 1);
    assert!(!sources.get_can_submit());
    let mut invalid = sources.get_preview().row_data(0).unwrap();
    invalid.name = "不支持的超长中文链接内容".repeat(8).into();
    invalid.detail = "链接格式无效，请返回输入页面修改；当前输入不属于受支持的 GitHub 附件直链。"
        .repeat(5)
        .into();
    sources.set_preview(presentation::model(vec![invalid]));
    sources.set_preview_directory(
        "D:\\中文下载目录\\很长的项目文件夹名称\\多个版本与附件\\最终保存位置".into(),
    );
    for (width, height) in [(1040, 740), (720, 520)] {
        capture(window, "compact-confirmation-invalid", width, height);
        assert_button_in_window(window, "返回修改", width, height);
        assert_eq!(
            ElementHandle::find_by_accessible_label(window, "创建 0 个有效任务")
                .next()
                .unwrap()
                .accessible_enabled(),
            Some(false)
        );
    }
    sources.invoke_dismiss_preview();
    sources.set_batch_input(previous_batch);
    sources.set_route(previous_route);
    workspace.set_directory(previous_directory);
    workspace.set_url(previous_url);
    workspace.invoke_navigate(previous_page);
}

async fn verify_selected_download_layouts(window: &MainWindow, api: &Arc<Service>) {
    let workspace = window.global::<Workspace>();
    let sources = window.global::<Sources>();
    let previous_page = workspace.get_page();
    let previous_diagnostic_input = workspace.get_diagnostic_input();
    workspace.invoke_navigate(0);
    workspace.invoke_select_download_section(1);
    workspace.invoke_select_creation_mode(1);
    let directory = workspace.get_directory();
    workspace.set_directory("D:\\Downloads\\GitHubSP".into());
    sources.set_repository("sample/project".into());
    sources.invoke_edited();
    sources.invoke_browse(1);
    ready(|| !sources.get_busy()).await;
    assert_eq!(sources.get_releases().row_count(), 3);
    assert_eq!(sources.get_assets().row_count(), 3);
    let attachment = sources.get_assets().row_data(0).unwrap().url;
    sources.invoke_select_asset(attachment.clone(), true);
    assert_eq!(sources.get_selected_count(), 1);
    assert_eq!(sources.get_selected_size(), "86.3 MB");
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "selected-new-download", width, height);
        assert_regions_separate(window, "SourceBrowser::versions", "SourceBrowser::assets");
        assert_button_in_window(window, "预览所选附件", width, height);
        scroll_to_button(window, "浏览", height);
        assert_button_in_window(window, "浏览", width, height);
        capture(window, "selected-new-directory", width, height);
    }
    sources.invoke_select_release("2".into());
    let other = sources.get_assets().row_data(1).unwrap().url;
    sources.invoke_select_asset(other, true);
    assert_eq!(sources.get_selected_count(), 2);
    assert_eq!(sources.get_selected_size(), "168 MB");
    sources.invoke_select_release("1".into());
    workspace.invoke_select_download_section(2);
    workspace.invoke_select_download_section(1);
    assert_eq!(sources.get_selected_count(), 2);
    assert!(sources.get_assets().row_data(0).unwrap().selected);
    sources.invoke_browse(2);
    ready(|| !sources.get_busy()).await;
    assert!(!sources.get_message().is_empty());
    assert_eq!(sources.get_page(), 1);
    assert_eq!(sources.get_selected_count(), 2);
    assert_eq!(sources.get_selected_size(), "168 MB");
    capture(window, "selected-new-page-error", 1040, 740);
    sources.set_message("".into());
    workspace.set_directory(directory);

    let original = api.manager.snapshot().await.unwrap();
    let mut snapshot = original.clone();
    snapshot.diagnostic_context = Some(DiagnosticContext {
        source: DiagnosticSource::Default,
        filename: "project-windows-x64.zip".into(),
    });
    snapshot.diagnostics = presentation::ROUTES
        .iter()
        .skip(1)
        .enumerate()
        .map(|(index, (id, name))| RouteReport {
            id: (*id).into(),
            name: (*name).into(),
            checked_at: 1,
            bytes_per_second: [3_240_000.0, 128_360.0, 0.0][index],
            available: index < 2,
            error: (index == 2).then(|| "连接超时".into()),
        })
        .collect();
    workspace.invoke_select_download_section(2);
    workspace.set_diagnostic_input("".into());
    presentation::apply(window, &snapshot);
    assert_eq!(workspace.get_route_best_name(), "GitHub 直连");
    assert_eq!(workspace.get_route_best_speed(), "3.2 MB/s");
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "selected-routes", width, height);
        assert_button_in_window(window, "检测线路", width, height);
        let scroll = ElementHandle::find_by_element_id(window, "RoutePanel::route-scroll")
            .next()
            .unwrap();
        assert!(scroll.size().height >= 100.0);
        scroll.scroll(0.0, -10_000.0);
        capture(window, "selected-routes-info", width, height);
        assert_button_in_window(window, "检测线路", width, height);
        scroll.scroll(0.0, 100_000.0);
    }
    snapshot.diagnostic_context = None;
    presentation::apply(window, &snapshot);
    assert!(workspace.get_route_result().contains("历史检测结果"));
    snapshot.diagnosing = true;
    presentation::apply(window, &snapshot);
    assert!(workspace.get_route_best_name().is_empty());
    assert!(workspace.get_route_best_speed().is_empty());
    assert!(workspace
        .get_routes()
        .iter()
        .all(|route| route.state == "检测中" && route.speed == "—"));
    capture(window, "selected-routes-running", 1040, 740);
    snapshot.diagnosing = false;
    for route in &mut snapshot.diagnostics {
        route.available = false;
        route.error = Some("连接超时，请检查网络或更换附件直链后重新检测。".repeat(4));
    }
    presentation::apply(window, &snapshot);
    assert!(workspace.get_route_best_name().is_empty());
    assert_eq!(workspace.get_route_tone(), 2);
    for (width, height) in [(1040, 740), (720, 520)] {
        capture(window, "selected-routes-errors", width, height);
        assert_button_in_window(window, "检测线路", width, height);
    }
    presentation::apply(window, &original);
    workspace.set_diagnostic_input(previous_diagnostic_input);
    workspace.invoke_navigate(previous_page);
}

fn verify_presented_states(window: &MainWindow, original: &githubsp_lib::model::Snapshot) {
    let workspace = window.global::<Workspace>();
    workspace.invoke_navigate(0);
    let mut snapshot = original.clone();
    snapshot.total_tasks = 90;
    snapshot.tasks = presentation::STATUSES
        .iter()
        .enumerate()
        .map(|(index, status)| {
            let mut task = original.tasks[0].clone();
            task.id = format!("status-{index}");
            task.status = *status;
            task
        })
        .collect();
    presentation::apply(window, &snapshot);
    assert_eq!(workspace.get_total_tasks(), "90");
    assert_eq!(workspace.get_running_tasks(), "6");
    assert_eq!(workspace.get_queued_tasks(), "1");
    assert_eq!(workspace.get_recovering_tasks(), "1");
    snapshot.tasks.retain(|task| {
        matches!(
            task.status,
            TaskStatus::Downloading
                | TaskStatus::Queued
                | TaskStatus::WaitingNetwork
                | TaskStatus::Completed
                | TaskStatus::Failed
        )
    });
    snapshot.total_tasks = snapshot.tasks.len();
    for task in &mut snapshot.tasks {
        task.filename = format!("示例附件-{}-windows-x64.zip", task.status.as_str());
        if task.status == TaskStatus::Downloading {
            task.speed = 5_200_000.0;
            task.eta = Some(72);
        }
        if task.status == TaskStatus::Failed {
            task.error = Some("连接超时，请稍后重试或调整线路。".into());
        }
    }
    presentation::apply(window, &snapshot);
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(window, "queue-statuses", width, height);
    }
    snapshot.tasks.clear();
    snapshot.total_tasks = 0;
    presentation::apply(window, &snapshot);
    assert_eq!(workspace.get_total_tasks(), "0");
    assert_eq!(workspace.get_running_tasks(), "0");
    assert_eq!(workspace.get_queued_tasks(), "0");
    assert_eq!(workspace.get_recovering_tasks(), "0");
    presentation::apply(window, original);
}

fn verify_secondary_windows() {
    let about = AboutWindow::new().unwrap();
    crate::about::bind_licenses(&about);
    let mut complete = String::new();
    for index in 0..about.get_license_count() {
        about.invoke_license_page(index);
        complete.push_str(&about.get_licenses());
    }
    assert_eq!(
        complete,
        include_str!("../resources/THIRD_PARTY_NOTICES.txt")
    );
    about.invoke_license_page(0);
    about.show().unwrap();
    for (width, height) in [
        (1448, 1086),
        (1040, 740),
        (720, 520),
        (660, 520),
        (480, 360),
    ] {
        capture(&about, "about", width, height);
        for label in ["上一页", "下一页", "关闭"] {
            assert_button_in_window(&about, label, width, height);
        }
    }
    ElementHandle::find_by_accessible_label(&about, "下一页")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    assert_eq!(about.get_license_index(), 1);
    about.invoke_license_page(-1);
    assert_eq!(about.get_license_index(), 1);
    about.invoke_license_page(about.get_license_count());
    assert_eq!(about.get_license_index(), 1);
    about.hide().unwrap();
    let completion = CompletionWindow::new().unwrap();
    completion.set_message("3 个下载已完成".into());
    completion.set_detail("示例附件-windows-x64.zip · 用时 12.5 秒".into());
    completion.show().unwrap();
    capture(&completion, "completion", 380, 170);
    for label in ["查看下载", "关闭"] {
        assert_button_in_window(&completion, label, 380, 170);
    }
    let hovered = std::rc::Rc::new(std::cell::Cell::new(false));
    let current = hovered.clone();
    completion.on_hover(move |value| current.set(value));
    completion
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(40.0, 40.0),
        });
    assert!(hovered.get(), "提醒窗口的停留状态应传递给原计时器");
    completion
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerExited);
    assert!(!hovered.get());
    completion.hide().unwrap();
}

fn verify_dialog_keyboard(window: &MainWindow) {
    let update = window.global::<Updates>();
    update.set_stage(0);
    update.set_dialog(true);
    update.set_downloadable(true);
    update.set_has_more(true);
    update.set_duplicate_task("fixture-duplicate".into());
    update.set_dialog_message("已有同版本任务，可继续原任务或另选位置重新下载。".into());
    let actions = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = actions.clone();
    update.on_download(move || calls.borrow_mut().push("download"));
    let calls = actions.clone();
    update.on_load_more(move || calls.borrow_mut().push("more"));
    let calls = actions.clone();
    update.on_resume_duplicate(move || calls.borrow_mut().push("duplicate"));
    let calls = actions.clone();
    update.on_official(move |_| calls.borrow_mut().push("official"));
    capture(window, "update-duplicate", 1040, 740);
    for label in ["下载", "历史", "收藏", "设置", "收起到托盘"] {
        assert!(ElementHandle::find_by_accessible_label(window, label)
            .filter(|element| element.accessible_role()
                == Some(i_slint_backend_testing::AccessibleRole::Button))
            .all(|element| element.accessible_enabled() == Some(false)));
    }
    let key = |key: slint::platform::Key| {
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.into() });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key.into() });
    };
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Return);
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Return);
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: slint::platform::Key::Shift.into(),
        });
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Tab);
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased {
            text: slint::platform::Key::Shift.into(),
        });
    key(slint::platform::Key::Return);
    for _ in 0..3 {
        key(slint::platform::Key::Tab);
    }
    key(slint::platform::Key::Return);
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Return);
    assert_eq!(
        *actions.borrow(),
        ["download", "more", "download", "duplicate", "official"]
    );
    update.set_stage(2);
    key(slint::platform::Key::Escape);
    key(slint::platform::Key::Tab);
    key(slint::platform::Key::Return);
    assert!(update.get_dialog());
    assert_eq!(actions.borrow().len(), 5);
    update.set_stage(0);
    key(slint::platform::Key::Escape);
    assert!(!update.get_dialog());
}

#[test]
fn module_pages_bind_to_core_and_remain_usable_at_supported_sizes() {
    // 软件测试后端只用于确定性组件验收，正式程序仍使用 DX12。
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        threading: true,
        mock_time: false,
        renderer_name: Some("software".into()),
    })))
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let root = tempfile::tempdir().unwrap();
    seed(root.path());
    let fixture = CatalogFixture::new();
    let mut api = runtime.block_on(Service::start(root.path())).unwrap();
    let mut fixture_network = githubsp_lib::network::Network::production().unwrap();
    fixture_network.api_base = fixture.address.clone();
    fixture_network.client = Default::default();
    Arc::get_mut(&mut api).unwrap().catalog = githubsp_lib::catalog::Catalog::new(fixture_network);
    let window = MainWindow::new().unwrap();
    controller::bind(&window, &api, runtime.handle());
    presentation::apply(&window, &runtime.block_on(api.manager.snapshot()).unwrap());
    window.show().unwrap();
    let ui = window.as_weak();
    let api1 = api.clone();
    let test_runtime = runtime.handle().clone();
    let finished = std::rc::Rc::new(std::cell::Cell::new(false));
    let done = finished.clone();
    let job = slint::spawn_local(async move {
        let window = ui.unwrap();
        assert_eq!(window.global::<Workspace>().get_tasks().row_count(), 51);
        assert_eq!(window.global::<Workspace>().get_total_tasks(), "64", "总数须包括未显示的历史记录");
        assert_eq!(window.global::<Workspace>().get_running_tasks(), "0");
        assert_eq!(window.global::<Workspace>().get_queued_tasks(), "0");
        assert_eq!(window.global::<Workspace>().get_recovering_tasks(), "0");
        assert_eq!(window.global::<Storage>().get_directory().as_str(),api1.data_directory.to_string_lossy());
        for (width,height) in [(1448,1086), (1040,740), (720,520)] {
            window.global::<Workspace>().invoke_navigate(0);
            capture(&window,"downloads",width,height);
            assert_eq!(window.global::<Workspace>().get_download_section(),0);
            for label in ["全部继续","全部暂停","清空已完成","下载队列","线路检测"] {
                assert_button_in_window(&window,label,width,height);
            }
            let queue = ElementHandle::find_by_element_id(&window,"QueuePanel::queue-list").next().unwrap();
            let queue_position = queue.absolute_position();
            let queue_size = queue.size();
            assert!(queue_position.y < height as f32 - 150.0, "队列应在首屏可见");
            assert!(queue_size.height > 100.0, "队列须拥有可用滚动高度");
            assert!(queue_position.x + queue_size.width <= width as f32 - 16.0);
            // 虚拟列表在新条目布局后修正估算高度，连续滚动并刷新布局再检查末项。
            for _ in 0..3 {
                queue.scroll(0.0,-100_000.0);
                window.window().take_snapshot().unwrap();
            }
            capture(&window,"downloads-queue-bottom",width,height);
            let tasks = window.global::<Workspace>().get_tasks();
            let last_name = tasks.row_data(tasks.row_count()-1).unwrap().filename;
            let last_row = ElementHandle::find_by_accessible_label(&window,last_name.as_str()).next().expect("滚动后应能看到最后一个任务");
            assert!(last_row.absolute_position().y >= queue_position.y);
            assert!(last_row.absolute_position().y + last_row.size().height <= queue_position.y + queue_size.height,
                "最后任务底部 {} 应位于队列底部 {} 之内", last_row.absolute_position().y + last_row.size().height, queue_position.y + queue_size.height);
            assert_button_in_window(&window,"全部暂停",width,height);
            window.global::<Workspace>().invoke_select_download_section(1);
            for mode in 0..3 {
                window.global::<Workspace>().invoke_select_creation_mode(mode);
                capture(&window,&format!("new-download-{mode}"),width,height);
                if mode == 1 { assert_regions_separate(&window,"SourceBrowser::versions","SourceBrowser::assets"); }
                let action = ["添加下载任务","预览所选附件","预览批量链接"][mode as usize];
                assert_button_in_window(&window,action,width,height);
                for label in ["直链下载","仓库附件","批量链接"] { assert_button_in_window(&window,label,width,height); }
            }
            window.global::<Workspace>().invoke_select_download_section(2);
            capture(&window,"diagnostics",width,height);
            assert_button_in_window(&window,"检测线路",width,height);
            navigate(&window,"历史"); ready(||!window.global::<History>().get_busy()).await;
            assert_eq!(window.global::<History>().get_rows().row_count(),20);
            assert_eq!(window.global::<History>().get_pages(),4);
            capture(&window,"history",width,height);
            scroll_to_bottom(&window); capture(&window,"history-pagination",width,height);
            assert_button_in_window(&window,"跳转",width,height);
            window.global::<History>().invoke_load(4); ready(||!window.global::<History>().get_busy()).await;
            assert_eq!(window.global::<History>().get_rows().row_count(),4);
            navigate(&window,"收藏");
            assert_eq!(window.global::<Favorites>().get_rows().row_count(),4);
            capture(&window,"favorites",width,height);
            scroll_to_bottom(&window); capture(&window,"favorites-cards",width,height);
            navigate(&window,"设置");
            for section in 0..5 {
                window.global::<Workspace>().invoke_select_settings_section(section);
                capture(&window,&format!("settings-{section}"),width,height);
                if section == 1 { assert_regions_separate(&window,"WindowSettings::tray-controls","WindowSettings::tray-example"); }
                if section == 2 { assert_regions_separate(&window,"FavoriteSettings::config","FavoriteSettings::sample"); }
                for label in ["保存全部设置","恢复全部默认","下载设置","窗口与提醒","收藏检查","数据管理","软件更新与关于"] {
                    assert_button_in_window(&window,label,width,height);
                }
                if section == 3 {
                    for label in ["打开数据目录","导出备份"] {
                        scroll_to_button(&window,label,height);
                        assert_button_in_window(&window,label,width,height);
                    }
                    capture(&window,"settings-data-actions",width,height);
                }
            }
        }
        verify_module_flows(&window,&api1,&fixture).await;
        verify_selected_download_layouts(&window,&api1).await;
        verify_direct_and_confirmation_layouts(&window,&api1).await;
        let settings = window.global::<Preferences>();
        settings.set_limit("0.262144".into()); settings.set_close_to_tray(true); settings.invoke_save();
        ready(||!settings.get_busy()).await;
        assert!(settings.get_saved());
        let saved = api1.manager.snapshot().await.unwrap();
        assert_eq!(saved.settings.limit_kib,256); assert!(saved.settings.close_to_tray);
        settings.invoke_reset(); assert_eq!(api1.manager.snapshot().await.unwrap().settings.limit_kib,256);
        settings.invoke_save(); ready(||!settings.get_busy()).await;
        let restored = api1.manager.snapshot().await.unwrap();
        assert_eq!(restored.settings.limit_kib,0); assert!(!restored.settings.close_to_tray);
        window.global::<Workspace>().invoke_select_settings_section(1); settings.set_limit("NaN".into()); settings.invoke_edited(); settings.invoke_save(); assert_eq!(settings.get_section(),0); assert!(!settings.get_busy()); assert!(settings.get_message().contains("请输入"));
        let update = window.global::<Updates>();
        window.global::<Workspace>().invoke_select_settings_section(4);
        let update_view = window.global::<Workspace>().get_view_revision();
        assert!(crate::updates::may_show_result(&window,update_view));
        window.global::<Workspace>().invoke_select_settings_section(1);
        assert!(!crate::updates::may_show_result(&window,update_view));
        window.global::<Workspace>().invoke_select_settings_section(4);
        assert!(!crate::updates::may_show_result(&window,update_view), "离开再返回时，迟到的版本检查不得弹出更新窗口");
        update.set_can_open(true);
        update.set_notes(presentation::model(crate::notes::render(&"## 较长更新说明\n\n保留中文说明、功能清单和换行，内容需要在说明区域滚动。\n\n".repeat(30))));
        capture(&window,"settings-long-notes",720,520);
        assert_button_in_window(&window,"保存全部设置",720,520);
        scroll_to_button(&window,"打开官方发布页",520);
        assert_button_in_window(&window,"打开官方发布页",720,520);
        update.set_downloadable(true);
        update.set_dialog(true); update.set_versions(presentation::model(vec!["v1.2.3（最新正式版）".into()]));
        update.set_attachment("GitHubSP-v1.2.3-windows-x64.exe".into()); update.set_attachment_detail("37.4 MB · Windows x64 免安装程序".into());
        update.set_selected_notes(presentation::model(crate::notes::render("# 更新说明\n\n**中文** 与 `代码`\n\n- 下载体验\n- 续传校验\n\n> 提醒\n\n| 项目 | 状态 |\n| --- | --- |\n| 下载 | 完成 |\n\n[官方说明](https://github.com/test/project/releases/tag/v1.2.3)\n\n<img src=\"https://example.com/image.png\">")));
        for (width,height) in [(1448,1086),(1040,740),(720,520)] {
            capture(&window,"update-dialog",width,height);
            for label in ["取消","选择保存位置并下载"] { assert_button_in_window(&window,label,width,height); }
        }
        update.set_stage(2); update.invoke_close(); assert!(update.get_dialog());
        update.set_stage(1); update.invoke_close(); assert!(!update.get_dialog()); update.set_stage(0);
        update.set_dialog(true); capture(&window,"update-dialog-keyboard",720,520);
        let section_before_modal = window.global::<Preferences>().get_section();
        window.global::<Workspace>().invoke_select_settings_section(4);
        assert_eq!(window.global::<Preferences>().get_section(),section_before_modal);
        window.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Escape.into() });
        assert!(!update.get_dialog(), "Esc 应关闭尚未提交的更新弹窗");
        let history = window.global::<History>();
        window.global::<Workspace>().invoke_navigate(1); ready(||!history.get_busy()).await;
        history.set_query("不存在的附件".into()); history.invoke_load(1); ready(||!history.get_busy()).await;
        assert_eq!(history.get_total(),0); assert_eq!(history.get_pages(),1);
        history.set_jump("999".into()); history.invoke_jump_to(); assert!(history.get_message().contains("请输入"));
        history.invoke_reset(); window.global::<Workspace>().invoke_navigate(2); window.global::<Workspace>().invoke_navigate(1);
        ready(||!history.get_busy()).await; assert_eq!(history.get_total(),64);
        history.set_query("不存在的附件".into()); history.invoke_load(1);
        history.set_query("中文".into()); history.invoke_load(1);
        ready(||!history.get_busy()).await; assert_eq!(history.get_total(),64);
        let before = history.get_revision();
        let cold = "00000000-0000-4000-8000-000000000001".to_owned();
        api1.manager.action(cold,githubsp_lib::manager::Action::Remove).await.unwrap();
        presentation::apply(&window,&api1.manager.snapshot().await.unwrap());
        ready(||!history.get_busy()).await;
        assert_eq!(history.get_total(),63); assert_ne!(history.get_revision(),before);
        window.global::<Workspace>().invoke_navigate(2);
        let favorites = window.global::<Favorites>(); favorites.set_filter(2); favorites.invoke_filter_changed(); assert_eq!(favorites.get_rows().row_count(),0);
        favorites.set_filter(0); favorites.invoke_filter_changed(); assert_eq!(favorites.get_rows().row_count(),4);
        window.global::<Workspace>().invoke_navigate(0);
        window.global::<Sources>().set_batch_input("".into());
        let failed_root = tempfile::tempdir().unwrap();
        let failed_path = failed_root.path().to_owned();
        let failed_api = test_runtime.spawn(async move {
            let service = Service::start(&failed_path).await.map_err(|error| error.to_string())?;
            service.shutdown().await?;
            Ok::<_,String>(service)
        }).await.unwrap().unwrap();
        let failed_window = MainWindow::new().unwrap();
        controller::bind(&failed_window,&failed_api,&test_runtime);
        let failed_settings = failed_window.global::<Preferences>();
        failed_settings.set_limit("2.5".into());
        failed_settings.invoke_edited();
        failed_settings.invoke_save();
        ready(|| !failed_settings.get_busy()).await;
        assert_eq!(failed_settings.get_limit(),"2.5");
        assert!(!failed_settings.get_saved());
        assert!(!failed_settings.get_message().is_empty());
        verify_presented_states(&window,&api1.manager.snapshot().await.unwrap());
        verify_secondary_windows();
        crate::completion::tests::verify_lifecycle(&window).await;
        verify_dialog_keyboard(&window);
        done.set(true); slint::quit_event_loop().unwrap();
    }).unwrap();
    slint::run_event_loop_until_quit().unwrap();
    drop(job);
    assert!(finished.get());
    runtime.block_on(api.shutdown()).unwrap();
    let native_fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/verification/native-fixture-{}",
        githubsp_lib::model::now_ms()
    ));
    std::fs::create_dir_all(&native_fixture).unwrap();
    seed(&native_fixture);
}

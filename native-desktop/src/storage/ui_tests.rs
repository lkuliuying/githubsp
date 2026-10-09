use super::*;

pub(super) async fn verify(runtime: &tokio::runtime::Handle) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("应用 数据");
    let target = root.path().join("自定义 数据");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    seed(&source);
    let starting = source.clone();
    let mut api = runtime
        .spawn(async move { Service::start(&starting).await.map_err(|e| e.to_string()) })
        .await
        .unwrap()
        .unwrap();
    let locations = githubsp_lib::relocation::Locations {
        default_directory: source,
        config_file: root.path().join("位置配置.json"),
    };
    Arc::get_mut(&mut api).unwrap().locations = Some(locations.clone());
    let window = MainWindow::new().unwrap();
    controller::bind(&window, &api, runtime);
    presentation::apply(&window, &api.manager.snapshot().await.unwrap());
    window.show().unwrap();
    window.global::<Workspace>().invoke_navigate(3);
    window
        .global::<Workspace>()
        .invoke_select_settings_section(3);
    let storage = window.global::<Storage>();
    assert!(storage.get_allowed());
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "data-directory-settings", width, height);
        scroll_to_button(&window, "迁移并重启", height);
        assert_button_in_window(&window, "迁移并重启", width, height);
        assert_button_in_window(&window, "浏览", width, height);
        capture(&window, "data-directory-settings-actions", width, height);
    }
    storage.set_target(target.to_string_lossy().as_ref().into());
    let preferences = window.global::<Preferences>();
    preferences.set_limit("12".into());
    storage.invoke_request_migration();
    assert!(storage.get_message().contains("先点击"));
    assert!(!storage.get_dialog());
    assert!(!storage.get_busy());
    preferences.set_limit("0".into());
    storage.invoke_request_migration();
    ready(|| !storage.get_busy()).await;
    assert!(storage.get_dialog(), "{}", storage.get_message());
    assert!(!locations.config_file.exists());
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "data-migration-confirm", width, height);
        for label in ["保留备份并迁移", "不备份并迁移", "取消"] {
            assert_button_in_window(&window, label, width, height);
        }
    }
    let key = |text: slint::SharedString| {
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
    };
    key(slint::platform::Key::Escape.into());
    assert!(!storage.get_dialog());
    assert!(!locations.config_file.exists());
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    storage.invoke_confirm_migration(false);
    assert!(!api.is_closing());
    storage.invoke_request_migration();
    ready(|| !storage.get_busy()).await;
    assert!(storage.get_dialog());
    let actions = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = actions.clone();
    storage.on_confirm_migration(move |keep| captured.borrow_mut().push(keep));
    capture(&window, "data-migration-keyboard", 720, 520);
    key(slint::platform::Key::Tab.into());
    key(slint::platform::Key::Return.into());
    assert_eq!(actions.borrow().as_slice(), &[true]);
    key(slint::platform::Key::Tab.into());
    key(slint::platform::Key::Return.into());
    assert_eq!(actions.borrow().as_slice(), &[true, false]);
    storage.set_migrating(true);
    key(slint::platform::Key::Escape.into());
    key(slint::platform::Key::Return.into());
    assert!(storage.get_dialog());
    assert_eq!(actions.borrow().len(), 2);
    storage.set_migrating(false);
    storage.invoke_cancel_migration();
    storage.set_allowed(false);
    storage.invoke_request_migration();
    assert!(!storage.get_dialog());
    assert!(!locations.config_file.exists());
    window.hide().unwrap();
    api.shutdown().await.unwrap();
}

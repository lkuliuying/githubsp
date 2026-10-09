use super::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

struct RestoreCursor(windows_sys::Win32::Foundation::POINT);

impl Drop for RestoreCursor {
    fn drop(&mut self) {
        unsafe {
            SetCursorPos(self.0.x, self.0.y);
        }
    }
}

async fn until(label: &str, timeout: Duration, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() {
        assert!(Instant::now() < deadline, "等待超时：{label}");
        tick().await;
    }
}

async fn focus(window: &slint::Window) {
    window.with_winit_window(|native| native.focus_window());
    let handle = desktop::hwnd(window).unwrap();
    until(
        "测试窗口获得前台焦点",
        Duration::from_secs(2),
        || unsafe { GetForegroundWindow() == handle },
    )
    .await;
}

fn click(popup: &CompletionWindow, label: &str) {
    let element = ElementHandle::find_by_accessible_label(popup, label)
        .next()
        .unwrap();
    let position = element.absolute_position();
    let size = element.size();
    let scale = popup.window().scale_factor();
    let x = ((position.x + size.width / 2.0) * scale) as u16;
    let y = ((position.y + size.height / 2.0) * scale) as u16;
    let point = ((y as u32) << 16 | x as u32) as isize;
    let handle = desktop::hwnd(popup.window()).unwrap();
    // 仅向本测试创建的 HWND 投递鼠标消息，不移动全局光标或操作用户窗口。
    unsafe {
        assert_ne!(PostMessageW(handle, WM_MOUSEMOVE, 0, point), 0);
        assert_ne!(PostMessageW(handle, WM_LBUTTONDOWN, 1, point), 0);
        assert_ne!(PostMessageW(handle, WM_LBUTTONUP, 0, point), 0);
    }
}

#[test]
#[ignore = "需要真实 Windows 桌面和 DX12；单独进程运行，避免与软件测试后端共享事件循环"]
fn real_winit_dx12_lifecycle() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let graphics = runtime.block_on(crate::graphics::shared_device()).unwrap();
    let ui_graphics = graphics.clone();
    // 验收线程持有最后的 GPU 引用，等界面线程的 Slint 上下文析构后再正常释放，避免在 Windows TLS 析构中销毁驱动。
    let ui = std::thread::spawn(move || verify_native_windows(ui_graphics));
    let deadline = Instant::now() + Duration::from_secs(90);
    while !ui.is_finished() {
        assert!(
            Instant::now() < deadline,
            "界面线程未能在 90 秒内完成验收及资源清理"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    ui.join().unwrap();
    drop(graphics);
}

fn verify_native_windows(graphics: slint::wgpu_30::WGPUConfiguration) {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let creation = CreationContext::default();
    let hook = creation.clone();
    let mut builder = winit::event_loop::EventLoop::with_user_event();
    builder.with_any_thread(true);
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(graphics)
        .with_winit_event_loop_builder(builder)
        .with_winit_window_attributes_hook(move |attributes| hook.attributes(attributes))
        .select()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let api = runtime
        .block_on(crate::service::Service::start(root.path()))
        .unwrap();
    let main = MainWindow::new().unwrap();
    main.window()
        .set_size(slint::LogicalSize::new(1040.0, 740.0));
    crate::controller::bind(&main, &api, runtime.handle());
    let tray = desktop::install(&main, &api, runtime.handle()).unwrap();
    let host = Host::new(&main, creation);
    let event = Rc::new(RefCell::new(snapshot(0, &[])));
    let (events, event_host) = (event.clone(), host.clone());
    main.on_refresh_state(move || event_host.update(&events.borrow()));
    let other = crate::AboutWindow::new().unwrap();
    main.show().unwrap();
    other.show().unwrap();
    let done = Rc::new(Cell::new(false));
    let finished = done.clone();
    let main_test = main.clone_strong();
    let host_test = host.clone();
    let watchdog = Timer::default();
    watchdog.start(TimerMode::SingleShot, Duration::from_secs(75), || {
        panic!("真实窗口验收超过 75 秒")
    });
    let job = slint::spawn_local(async move {
        let (main, host) = (main_test, host_test);
        drop(main.window().winit_window().await);
        drop(other.window().winit_window().await);
        let mut revision = 0;
        for mode in ["失焦", "最小化", "托盘"] {
            main.invoke_show_main();
            focus(main.window()).await;
            match mode {
                "最小化" => {
                    main.window()
                        .with_winit_window(|native| native.set_minimized(true));
                }
                "托盘" => main.invoke_hide_to_tray(),
                _ => (),
            }
            focus(other.window()).await;
            assert!(desktop::is_background(&main));
            if mode == "最小化" {
                assert_ne!(
                    unsafe { IsIconic(desktop::hwnd(main.window()).unwrap()) },
                    0
                );
            }
            if mode == "托盘" {
                assert!(!main.window().is_visible());
            }
            let foreground = unsafe { GetForegroundWindow() };
            revision += 1;
            event.replace(snapshot(revision, &[mode]));
            host.update(&event.borrow());
            let (id, _, popup) = current(&host);
            assert!(
                desktop::hwnd(popup.window()).is_none(),
                "首次创建应覆盖句柄异步就绪场景"
            );
            assert!(!popup.window().is_visible());
            until("首次有效渲染", Duration::from_secs(5), || {
                host.state.borrow().deadline().is_some()
            })
            .await;
            let handle = desktop::hwnd(popup.window()).unwrap();
            assert_ne!(unsafe { IsWindowVisible(handle) }, 0);
            assert_eq!(
                unsafe { GetForegroundWindow() },
                foreground,
                "提醒不能抢焦点：{mode}"
            );
            // Winit 通过 ITaskbarList::DeleteTab 跳过任务栏，而不是设置 WS_EX_TOOLWINDOW。
            for normal in [main.window(), other.window()] {
                assert_eq!(
                    unsafe { GetWindowLongPtrW(desktop::hwnd(normal).unwrap(), GWL_EXSTYLE) }
                        & WS_EX_TOOLWINDOW as isize,
                    0
                );
            }
            let started = host.state.borrow().deadline().unwrap() - Duration::from_secs(8);
            delay(Duration::from_secs(7)).await;
            assert!(popup.window().is_visible(), "8 秒前不能关闭：{mode}");
            until("8 秒自动收起", Duration::from_secs(3), || {
                !popup.window().is_visible()
            })
            .await;
            let elapsed = started.elapsed();
            assert!(elapsed >= Duration::from_secs(8) && elapsed < Duration::from_secs(10));
            assert_eq!(unsafe { IsWindowVisible(handle) }, 0);
            assert!(host.window(id).is_none());
            eprintln!(
                "真实 Winit/DX12 {mode}：句柄延迟就绪、未抢焦点、自动收起 {:.3} 秒",
                elapsed.as_secs_f64()
            );
            drop(popup);
            until("释放原生提醒窗口", Duration::from_secs(1), || unsafe {
                IsWindow(handle) == 0
            })
            .await;
        }

        revision += 1;
        event.replace(snapshot(revision, &["mouse-close"]));
        host.update(&event.borrow());
        let (_, _, popup) = current(&host);
        until("点击前渲染", Duration::from_secs(5), || {
            host.state.borrow().deadline().is_some()
        })
        .await;
        let handle = desktop::hwnd(popup.window()).unwrap();
        click(&popup, "关闭");
        until("鼠标点击关闭", Duration::from_secs(1), || {
            !popup.window().is_visible()
        })
        .await;
        assert_eq!(unsafe { IsWindowVisible(handle) }, 0);
        eprintln!("真实 Winit/DX12：关闭按钮的 Windows 鼠标消息触发收起。");

        revision += 1;
        event.replace(snapshot(revision, &["hover-focus"]));
        host.update(&event.borrow());
        let (_, _, popup) = current(&host);
        until("交互前渲染", Duration::from_secs(5), || {
            host.state.borrow().deadline().is_some()
        })
        .await;
        {
            let mut cursor = windows_sys::Win32::Foundation::POINT::default();
            assert_ne!(unsafe { GetCursorPos(&mut cursor) }, 0);
            let _restore = RestoreCursor(cursor);
            let mut area = windows_sys::Win32::Foundation::RECT::default();
            assert_ne!(
                unsafe { GetWindowRect(desktop::hwnd(popup.window()).unwrap(), &mut area) },
                0
            );
            // TrackMouseEvent 根据真实光标发送离开消息；持续悬停需要移动并恢复实际光标。
            assert_ne!(unsafe { SetCursorPos(area.left + 30, area.top + 30) }, 0);
            until("悬停暂停", Duration::from_secs(1), || {
                host.state.borrow().deadline().is_none()
            })
            .await;
            delay(Duration::from_millis(200)).await;
            assert!(host.state.borrow().deadline().is_none());
            assert_ne!(
                unsafe { GetWindowRect(desktop::hwnd(other.window()).unwrap(), &mut area) },
                0
            );
            assert_ne!(unsafe { SetCursorPos(area.left + 30, area.top + 30) }, 0);
            until("离开恢复", Duration::from_secs(1), || {
                host.state.borrow().deadline().is_some()
            })
            .await;
        }
        focus(popup.window()).await;
        until("键盘焦点暂停", Duration::from_secs(1), || {
            host.state.borrow().deadline().is_none()
        })
        .await;
        focus(other.window()).await;
        until("失焦恢复", Duration::from_secs(1), || {
            host.state.borrow().deadline().is_some()
        })
        .await;
        main.global::<crate::Workspace>().set_page(3);
        main.global::<crate::Workspace>().set_download_section(2);
        click(&popup, "查看下载");
        until(
            "查看下载激活主窗口",
            Duration::from_secs(2),
            || unsafe { GetForegroundWindow() == desktop::hwnd(main.window()).unwrap() },
        )
        .await;
        assert!(!popup.window().is_visible());
        assert_eq!(main.global::<crate::Workspace>().get_page(), 0);
        assert_eq!(main.global::<crate::Workspace>().get_download_section(), 0);
        eprintln!("真实 Winit/DX12：悬停与焦点暂停恢复、查看下载进入队列并激活主窗口。");
        host.shutdown();
        other.hide().unwrap();
        main.hide().unwrap();
        finished.set(true);
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
    watchdog.stop();
    job.abort();
    host.shutdown();
    drop(tray);
    runtime.block_on(api.shutdown()).unwrap();
    assert!(done.get());
}

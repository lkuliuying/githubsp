use super::*;
use githubsp_lib::model::{CompletionSummary, Notice, NoticeKind, Settings};
use i_slint_backend_testing::ElementHandle;
use std::collections::VecDeque;

#[path = "native_tests.rs"]
mod native_tests;

type Rendered = Rc<RefCell<Box<dyn FnMut()>>>;

struct FakePlatform {
    pending: RefCell<VecDeque<oneshot::Sender<Result<(), String>>>>,
    rendered: RefCell<Vec<Rendered>>,
    background: Cell<bool>,
    fail_create: Cell<bool>,
    fail_observe: Cell<bool>,
    fail_place: Cell<bool>,
    fail_hide: Cell<bool>,
    reenter_hide: Cell<bool>,
    places: Cell<usize>,
    hides: Cell<usize>,
    dropped_waits: Rc<Cell<usize>>,
}

impl Default for FakePlatform {
    fn default() -> Self {
        Self {
            pending: Default::default(),
            rendered: Default::default(),
            background: Cell::new(true),
            fail_create: Default::default(),
            fail_observe: Default::default(),
            fail_place: Default::default(),
            fail_hide: Default::default(),
            reenter_hide: Default::default(),
            places: Default::default(),
            hides: Default::default(),
            dropped_waits: Default::default(),
        }
    }
}

impl PopupPlatform for FakePlatform {
    fn create(&self) -> Result<CompletionWindow, String> {
        if self.fail_create.replace(false) {
            Err("注入创建失败".into())
        } else {
            CompletionWindow::new().map_err(|error| error.to_string())
        }
    }
    fn ready(&self, popup: slint::Weak<CompletionWindow>) -> Ready {
        struct Released(Rc<Cell<usize>>);
        impl Drop for Released {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let (send, receive) = oneshot::channel();
        self.pending.borrow_mut().push_back(send);
        let released = Released(self.dropped_waits.clone());
        Box::pin(async move {
            let _released = released;
            let _popup = popup.upgrade().ok_or("提醒窗口已释放")?;
            receive.await.map_err(|error| error.to_string())?
        })
    }
    fn observe_render(
        &self,
        _: &CompletionWindow,
        rendered: Box<dyn FnMut()>,
    ) -> Result<(), String> {
        if self.fail_observe.replace(false) {
            return Err("注入渲染通知注册失败".into());
        }
        self.rendered
            .borrow_mut()
            .push(Rc::new(RefCell::new(rendered)));
        Ok(())
    }
    fn place(&self, popup: &CompletionWindow, _: &MainWindow) -> Result<(), String> {
        assert!(!popup.window().is_visible(), "必须先定位后显示");
        self.places.set(self.places.get() + 1);
        if self.fail_place.replace(false) {
            Err("注入定位失败".into())
        } else {
            Ok(())
        }
    }
    fn background(&self, _: &MainWindow) -> bool {
        self.background.get()
    }
    fn hide(&self, popup: &CompletionWindow) -> Result<(), String> {
        self.hides.set(self.hides.get() + 1);
        if self.reenter_hide.replace(false) {
            popup.invoke_hover(false);
            popup.invoke_dismiss();
        }
        if self.fail_hide.replace(false) {
            return Err("注入隐藏失败".into());
        }
        popup.hide().map_err(|error| error.to_string())
    }
}

impl FakePlatform {
    fn finish(&self, result: Result<(), String>) {
        self.pending
            .borrow_mut()
            .pop_front()
            .unwrap()
            .send(result)
            .unwrap();
    }
    fn rendered(&self) {
        let callback = self.rendered.borrow().last().unwrap().clone();
        callback.borrow_mut()();
    }
}

fn snapshot(revision: u64, ids: &[&str]) -> Snapshot {
    Snapshot {
        tasks: vec![],
        total_tasks: ids.len(),
        completed_tasks: ids.len(),
        history_revision: revision,
        last_directory: None,
        error: None,
        revision,
        settings: Settings::default(),
        queue_revision: 0,
        diagnostics: vec![],
        diagnostic_context: None,
        diagnosing: false,
        favorites: vec![],
        notices: ids
            .iter()
            .map(|id| Notice {
                id: id.to_string(),
                kind: NoticeKind::DownloadCompleted,
                task_id: Some(id.to_string()),
                message: String::new(),
                created_at: 0,
                completion: Some(CompletionSummary {
                    filename: format!("{id}.zip"),
                    elapsed_ms: Some(1234),
                    elapsed_is_partial: false,
                }),
            })
            .collect(),
    }
}

async fn tick() {
    delay(Duration::from_millis(20)).await;
}
async fn delay(duration: Duration) {
    let (send, receive) = oneshot::channel();
    Timer::single_shot(duration, move || {
        let _ = send.send(());
    });
    receive.await.unwrap();
}

fn current(host: &Host) -> (u64, u64, CompletionWindow) {
    let slot = host.popup.borrow();
    let session = slot.as_ref().unwrap();
    (session.id, session.revision, session.window.clone_strong())
}

async fn shown(
    host: &Rc<Host>,
    platform: &FakePlatform,
    event: &Snapshot,
) -> (u64, u64, CompletionWindow) {
    host.update(event);
    let current = current(host);
    assert!(!current.2.window().is_visible());
    platform.finish(Ok(()));
    tick().await;
    assert!(current.2.window().is_visible());
    assert!(
        host.state.borrow().deadline().is_none(),
        "首次渲染之前不启动倒计时"
    );
    current
}

#[test]
fn creation_context_restores_normal_and_nested_window_attributes() {
    let context = CreationContext::default();
    assert!(
        context
            .attributes(winit::window::Window::default_attributes())
            .active
    );
    let result: Result<(), ()> = context.during(|| {
        assert!(
            !context
                .attributes(winit::window::Window::default_attributes())
                .active
        );
        context.during(|| assert!(context.0.get()));
        assert!(context.0.get());
        Err(())
    });
    assert!(result.is_err());
    assert!(!context.0.get());
    assert!(
        context
            .attributes(winit::window::Window::default_attributes())
            .active
    );
}

// 与现有组件验收共用唯一的软件事件循环，避免 Slint 全局代理被第二个测试覆盖。
pub(crate) async fn verify_lifecycle(main: &MainWindow) {
    let platform = Rc::new(FakePlatform::default());
    let host = Host::with_platform(main, CreationContext::default(), platform.clone());
    host.update(&snapshot(1, &["one"]));
    let (old_id, old_revision, old) = current(&host);
    assert!(!old.window().is_visible());
    tick().await;
    assert!(!old.window().is_visible());
    assert_eq!(platform.places.get(), 0);
    assert!(host.state.borrow().deadline().is_none());
    platform.finish(Ok(()));
    tick().await;
    assert!(old.window().is_visible());
    platform.rendered();
    assert!(host.state.borrow().deadline().is_some());
    let button = ElementHandle::find_by_accessible_label(&old, "关闭")
        .next()
        .unwrap();
    button.invoke_accessible_default_action();
    assert!(!old.window().is_visible());
    assert!(host.popup.borrow().is_none());
    old.invoke_dismiss();
    host.expire(
        old_id,
        old_revision,
        Instant::now() + Duration::from_secs(9),
    );
    assert!(host.popup.borrow().is_none());

    let (id, revision, popup) = shown(&host, &platform, &snapshot(2, &["one", "two"])).await;
    let now = Instant::now();
    host.presented(id, revision, now);
    assert_eq!(
        host.state.borrow().deadline(),
        Some(now + Duration::from_secs(8))
    );
    host.interaction(id, Some(true), None, now + Duration::from_secs(2));
    assert!(host.state.borrow().deadline().is_none());
    host.interaction(id, None, Some(true), now + Duration::from_secs(3));
    host.interaction(id, Some(false), None, now + Duration::from_secs(4));
    assert!(host.state.borrow().deadline().is_none());
    host.interaction(id, None, Some(false), now + Duration::from_secs(5));
    assert_eq!(
        host.state.borrow().deadline(),
        Some(now + Duration::from_secs(11))
    );
    host.expire(id, revision, now + Duration::from_secs(10));
    assert!(popup.window().is_visible());
    host.expire(id, revision, now + Duration::from_secs(11));
    assert!(!popup.window().is_visible());

    let (id, revision, popup) =
        shown(&host, &platform, &snapshot(3, &["one", "two", "three"])).await;
    host.presented(id, revision, now);
    host.update(&snapshot(4, &["one", "two", "three", "four"]));
    let (_, merged_revision, _) = current(&host);
    assert_eq!(popup.get_message(), "2 个下载已完成");
    assert_eq!(platform.places.get(), 3, "合并内容不重复显示或定位窗口");
    assert!(host.state.borrow().deadline().is_none());
    host.expire(id, revision, now + Duration::from_secs(100));
    assert!(popup.window().is_visible());
    host.presented(id, revision, now);
    assert!(host.state.borrow().deadline().is_none());
    host.presented(id, merged_revision, now + Duration::from_secs(7));
    assert_eq!(
        host.state.borrow().deadline(),
        Some(now + Duration::from_secs(15))
    );
    old.invoke_dismiss();
    old.invoke_open();
    old.invoke_hover(true);
    host.prepared(old_id, Err("迟到失败".into()));
    host.initialization_expired(old_id);
    host.presented(old_id, old_revision, now);
    let old_render = platform.rendered.borrow()[0].clone();
    old_render.borrow_mut()();
    host.expire(old_id, old_revision, now + Duration::from_secs(100));
    assert!(popup.window().is_visible());
    assert_eq!(
        host.state.borrow().deadline(),
        Some(now + Duration::from_secs(15))
    );
    platform.fail_hide.set(true);
    popup.invoke_dismiss();
    assert!(popup.window().is_visible());
    assert!(host.has_phase(id, Phase::Closing));
    assert!(main.get_message().contains("注入隐藏失败"));
    platform.reenter_hide.set(true);
    let hides = platform.hides.get();
    popup.invoke_dismiss();
    assert!(!popup.window().is_visible());
    assert!(host.popup.borrow().is_none());
    assert_eq!(
        platform.hides.get(),
        hides + 1,
        "重入关闭不能再次调用平台隐藏"
    );

    // 每种失败后都重新送达同一通知，再发送新通知，验证既不重放也不会永久失效。
    for failure in [
        "create",
        "observe",
        "ready",
        "place",
        "timeout",
        "foreground",
        "disabled",
    ] {
        let platform = Rc::new(FakePlatform::default());
        let host = Host::with_platform(main, CreationContext::default(), platform.clone());
        platform.fail_create.set(failure == "create");
        platform.fail_observe.set(failure == "observe");
        platform.fail_place.set(failure == "place");
        let event = snapshot(1, &["failed"]);
        host.update(&event);
        if !matches!(failure, "create" | "observe") {
            let (id, _, waiting) = current(&host);
            let weak_window = waiting.as_weak();
            tick().await;
            match failure {
                "ready" => platform.finish(Err("注入就绪失败".into())),
                "place" => platform.finish(Ok(())),
                "timeout" => delay(INITIALIZATION_TIMEOUT + Duration::from_millis(100)).await,
                "foreground" => {
                    platform.background.set(false);
                    host.update(&event);
                }
                "disabled" => {
                    let mut disabled = event.clone();
                    disabled.settings.background_completion_notice = false;
                    host.update(&disabled);
                }
                _ => unreachable!(),
            }
            tick().await;
            assert!(!waiting.window().is_visible(), "失败场景：{failure}");
            drop(waiting);
            assert!(
                weak_window.upgrade().is_none(),
                "失败后需释放组件：{failure}"
            );
            assert_eq!(
                platform.dropped_waits.get(),
                1,
                "取消后需唤醒并释放任务：{failure}"
            );
            if let Some(late) = platform.pending.borrow_mut().pop_front() {
                assert!(late.send(Ok(())).is_err());
            }
            host.prepared(id, Ok(()));
        }
        assert!(host.popup.borrow().is_none(), "失败场景：{failure}");
        assert!(host.state.borrow().view().notice.is_none());
        assert!(host.preparation.borrow().is_none());
        assert!(!host.timer.running());
        assert!(!host.initialization_timer.running());
        assert_eq!(event.completed_tasks, 1);
        assert!(event.notices[0].completion.is_some());
        platform.background.set(true);
        host.update(&snapshot(2, &["failed"]));
        assert!(host.popup.borrow().is_none(), "失败通知不能重放：{failure}");
        let (_, _, next) = shown(&host, &platform, &snapshot(3, &["failed", "next"])).await;
        assert_eq!(next.get_message(), "1 个下载已完成");
        next.invoke_dismiss();
        assert!(!next.window().is_visible());
    }

    let platform = Rc::new(FakePlatform::default());
    let host = Host::with_platform(main, CreationContext::default(), platform.clone());
    let (_, _, popup) = shown(&host, &platform, &snapshot(1, &["open"])).await;
    main.global::<crate::Workspace>().set_page(3);
    main.global::<crate::Workspace>().set_download_section(2);
    let weak_popup = popup.as_weak();
    let weak_main = main.as_weak();
    let opened = Rc::new(Cell::new(false));
    let observed = opened.clone();
    main.on_show_main(move || {
        assert!(
            !weak_popup.unwrap().window().is_visible(),
            "先关闭提醒再显示主窗口"
        );
        observed.set(true);
        weak_main.unwrap().show().unwrap();
    });
    ElementHandle::find_by_accessible_label(&popup, "查看下载")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    assert!(opened.get());
    assert_eq!(main.global::<crate::Workspace>().get_page(), 0);
    assert_eq!(main.global::<crate::Workspace>().get_download_section(), 0);
    assert!(host.popup.borrow().is_none());
    main.on_show_main(|| {});
    eprintln!("完成提醒软件回归通过：延迟就绪、关闭、暂停恢复、合并、旧回调、七类失败、隐藏重试与查看下载。");
}

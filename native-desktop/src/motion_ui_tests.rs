use super::*;
use crate::FavoriteRow;
use slint::{
    platform::{PointerEventButton, WindowEvent},
    LogicalPosition, VecModel,
};
use std::{cell::RefCell, rc::Rc};

fn element(window: &MainWindow, id: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(window, id)
        .next()
        .unwrap_or_else(|| panic!("应存在组件 {id}"))
}

fn card(window: &MainWindow, key: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(window, key)
        .find(|item| {
            item.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Groupbox)
        })
        .unwrap_or_else(|| panic!("应存在收藏卡 {key}"))
}

fn surface(card: &ElementHandle) -> ElementHandle {
    card.query_descendants()
        .match_id("BentoCard::surface")
        .find_first()
        .expect("卡片应有独立视觉层")
}

fn tick(ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        std::thread::sleep(remaining.min(Duration::from_millis(16)));
        slint::platform::update_timers_and_animations();
    }
}

fn pointer(window: &MainWindow, position: LogicalPosition) {
    // 软件截图可能耗时超过一个动画周期，发送新输入前先推进事件时钟。
    slint::platform::update_timers_and_animations();
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    slint::platform::update_timers_and_animations();
}

fn hover(window: &MainWindow, item: &ElementHandle) -> LogicalPosition {
    let p = item.absolute_position();
    // 固定命中区的留白，避开内部操作按钮。
    let point = LogicalPosition::new(p.x + 10.0, p.y + 20.0);
    pointer(window, point);
    point
}

fn click(window: &MainWindow, item: &ElementHandle) {
    let p = item.absolute_position();
    let size = item.size();
    let position = LogicalPosition::new(p.x + size.width / 2.0, p.y + size.height / 2.0);
    pointer(window, position);
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
    slint::platform::update_timers_and_animations();
}

fn aligned(indicator: &ElementHandle, visual: &ElementHandle) {
    let p = indicator.absolute_position();
    let q = visual.absolute_position();
    let a = indicator.size();
    let b = visual.size();
    assert!(
        (p.x - q.x).abs() < 1.5 && (p.y - q.y).abs() < 1.5,
        "磁吸框应与卡片对齐：{p:?} / {q:?}"
    );
    assert!(
        (a.width - b.width).abs() < 1.5 && (a.height - b.height).abs() < 1.5,
        "磁吸框应跟随卡片尺寸：{a:?} / {b:?}"
    );
}

fn rows(count: usize) -> Rc<VecModel<FavoriteRow>> {
    Rc::new(VecModel::from(
        (0..count)
            .map(|i| FavoriteRow {
                repository: format!("motion/project-{i:03}").into(),
                tag: "v1.2.0".into(),
                state: "检查成功".into(),
                checked: "刚刚".into(),
                published: if i % 2 == 0 { "2026-10-26" } else { "" }.into(),
                summary: if i % 3 == 0 {
                    "版本摘要用于验证不同高度的收藏卡及响应式换行。"
                } else {
                    ""
                }
                .into(),
                downloadable: true,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
}

pub(super) fn verify_navigation_motion() {
    fn button(window: &MainWindow, label: &str) -> ElementHandle {
        ElementHandle::find_by_accessible_label(window, label)
            .find(|item| {
                item.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
            })
            .unwrap_or_else(|| panic!("应有导航入口 {label}"))
    }

    let window = MainWindow::new().unwrap();
    crate::presentation::bind_detail_selection(&window);
    let workspace = window.global::<Workspace>();
    window.show().unwrap();
    window
        .window()
        .set_size(slint::PhysicalSize::new(1040, 740));
    window.window().take_snapshot().unwrap();
    tick(250);
    let indicator = element(&window, "WorkspaceNavigation::primary-indicator");
    let start = indicator.absolute_position().y;
    slint::platform::update_timers_and_animations();
    workspace.set_page(2);
    slint::platform::update_timers_and_animations();
    let target = button(&window, "收藏").absolute_position().y;
    assert!(
        (indicator.absolute_position().y - start).abs() < 4.0,
        "主导航应从当前位置继续移动"
    );
    tick(70);
    let middle = indicator.absolute_position().y;
    assert!(middle > start && middle < target, "主导航应存在连续过渡态");
    workspace.set_page(1);
    // 在同一绘制帧比较重定向位置，排除软件布局本身消耗的墙钟时间。
    i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
    assert!(
        (indicator.absolute_position().y - middle).abs() < 4.0,
        "反向切换不得闪跳"
    );
    window.global::<Motion>().set_reduce(true);
    slint::platform::update_timers_and_animations();
    assert!(
        (indicator.absolute_position().y - button(&window, "历史").absolute_position().y).abs()
            < 4.0,
        "减少动效应立即定位导航选中层"
    );

    workspace.set_page(3);
    let settings = window.global::<Preferences>();
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "linear-settings", width, height);
        let content = element(&window, "SettingsPage::settings-content");
        assert!(content.size().width <= 760.0, "设置内容应限制阅读宽度");
        for label in [
            "下载设置",
            "窗口与提醒",
            "收藏检查",
            "数据管理",
            "软件更新与关于",
        ] {
            let item = button(&window, label);
            let p = item.absolute_position();
            let size = item.size();
            assert!(p.y >= 0.0 && p.y + size.height <= height as f32);
            if width >= 900 {
                assert!(p.x + size.width <= 220.0, "宽屏设置入口应位于统一侧栏");
            } else {
                assert!(p.x >= 64.0, "窄屏设置入口应保留在正文顶部");
            }
        }
        assert_button_in_window(&window, "保存全部设置", width, height);
        assert_button_in_window(&window, "恢复全部默认", width, height);
    }

    let tabs = element(&window, "ModuleNavigation::selection");
    let initial_x = tabs.absolute_position().x;
    window.global::<Motion>().set_reduce(false);
    slint::platform::update_timers_and_animations();
    assert_eq!(tabs.absolute_position().x, initial_x);
    settings.set_section(4);
    slint::platform::update_timers_and_animations();
    assert!((tabs.absolute_position().x - initial_x).abs() < 4.0);
    tick(70);
    let middle_x = tabs.absolute_position().x;
    let last_x = button(&window, "软件更新与关于").absolute_position().x;
    assert!(
        middle_x > initial_x && middle_x < last_x,
        "标签选中层应平滑滑动"
    );
    settings.set_section(1);
    i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
    assert!((tabs.absolute_position().x - middle_x).abs() < 4.0);
    window.global::<Motion>().set_reduce(true);
    slint::platform::update_timers_and_animations();
    assert!(
        (tabs.absolute_position().x - button(&window, "窗口与提醒").absolute_position().x).abs()
            < 4.0
    );
    capture(&window, "magpie-reduced-navigation", 720, 520);

    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    workspace.on_navigate(move |page| recorded.borrow_mut().push(page));
    click(&window, &button(&window, "历史"));
    assert_eq!(
        calls.borrow().as_slice(),
        &[1],
        "一次点击只发出一次页面操作"
    );
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert_eq!(calls.borrow().as_slice(), &[1, 1], "键盘仍能操作导航按钮");
    window.hide().unwrap();
}

pub(super) fn verify_favorites_motion() {
    let window = MainWindow::new().unwrap();
    crate::presentation::bind_detail_selection(&window);
    let favorites = window.global::<Favorites>();
    let model = rows(200);
    favorites.set_rows(model.clone().into());
    favorites.set_total(200);
    window.global::<Workspace>().set_page(2);
    window.set_page_entering(false);
    window.show().unwrap();
    capture(&window, "motion-title-before", 1448, 1086);
    let title = element(&window, "MotionReveal::surface");
    assert!(title.computed_opacity() < 0.01);
    slint::platform::update_timers_and_animations();
    window.set_page_entering(true);
    slint::platform::update_timers_and_animations();
    assert!(title.computed_opacity() < 1.0);
    tick(75);
    assert!(title.computed_opacity() > 0.0 && title.computed_opacity() < 1.0);
    capture(&window, "motion-title-entering", 1448, 1086);
    tick(200);
    assert_eq!(title.computed_opacity(), 1.0);

    let actions = Rc::new(RefCell::new(Vec::new()));
    let called = actions.clone();
    favorites.on_action(move |key, action| called.borrow_mut().push((key, action)));
    let downloads = Rc::new(RefCell::new(Vec::new()));
    let called = downloads.clone();
    favorites.on_download(move |key| called.borrow_mut().push(key));

    let first = card(&window, "motion/project-000");
    let second = card(&window, "motion/project-001");
    hover(&window, &first);
    let indicator = element(&window, "BentoGrid::indicator");
    assert_eq!(
        ElementHandle::find_by_element_type_name(&window, "MagneticIndicator").count(),
        1,
        "列表只保留一个共享指示框"
    );
    // 首次命中直接落位，不能从原点飞入。
    aligned(&indicator, &surface(&first));
    tick(400);
    aligned(&indicator, &surface(&first));
    let fixed = first.absolute_position();
    assert!((surface(&first).absolute_position().y - fixed.y + 2.0).abs() < 0.1);
    capture(&window, "motion-hover", 1448, 1086);

    let initial = indicator.absolute_position();
    hover(&window, &second);
    let start = indicator.absolute_position();
    assert!((start.x - initial.x).abs() < 10.0, "换卡应从当前位置继续");
    tick(65);
    let intermediate = indicator.absolute_position();
    assert!(intermediate.x > initial.x && intermediate.x < second.absolute_position().x);
    hover(&window, &first);
    let reversed = indicator.absolute_position();
    assert!(
        (reversed.x - intermediate.x).abs() < 10.0,
        "反向切换不得跳回起点"
    );
    tick(400);
    aligned(&indicator, &surface(&first));

    let p = first.absolute_position();
    pointer(
        &window,
        LogicalPosition::new(p.x + first.size().width + 5.0, p.y + 20.0),
    );
    tick(20);
    assert!(
        indicator.computed_opacity() > 0.95,
        "短暂经过间隙应保留指示框"
    );
    hover(&window, &second);
    tick(400);
    assert_eq!(
        indicator.computed_opacity(),
        1.0,
        "旧目标离开不得清除新目标"
    );
    aligned(&indicator, &surface(&second));
    pointer(&window, LogicalPosition::new(8.0, 8.0));
    tick(75);
    tick(100);
    assert_eq!(indicator.computed_opacity(), 0.0, "退出网格后应隐藏");

    let point = hover(&window, &first);
    tick(400);
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: PointerEventButton::Left,
    });
    tick(110);
    assert!((surface(&first).absolute_position().y - fixed.y - 1.0).abs() < 0.1);
    capture(&window, "motion-press", 1448, 1086);
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position: point,
            button: PointerEventButton::Left,
        });
    tick(400);
    assert!(actions.borrow().is_empty(), "空白卡片不新增业务点击行为");
    let collapsed_height = first.size().height;
    let more =
        ElementHandle::find_by_accessible_label(&window, "更多收藏操作 · motion/project-000")
            .next()
            .unwrap();
    click(&window, &more);
    tick(400);
    aligned(&indicator, &surface(&first));
    assert!(
        first.size().height > collapsed_height,
        "展开操作区仍由内容决定高度"
    );
    let browse = first
        .query_descendants()
        .match_accessible_role(i_slint_backend_testing::AccessibleRole::Button)
        .match_predicate(|item| {
            item.accessible_label()
                .is_some_and(|label| label == "查看附件")
        })
        .find_first()
        .unwrap();
    click(&window, &browse);
    assert_eq!(
        actions.borrow().as_slice(),
        &[("motion/project-000".into(), "browse".into())]
    );
    assert!(downloads.borrow().is_empty());
    // 鼠标点击后保留原按钮焦点，空格仍只触发一次原操作。
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert_eq!(actions.borrow().len(), 2);
    assert_eq!(
        actions.borrow()[1],
        ("motion/project-000".into(), "browse".into())
    );

    let scroll_position = hover(&window, &first);
    tick(400);
    let list = element(&window, "FavoritesPage::favorite-list");
    slint::platform::update_timers_and_animations();
    let before = indicator.absolute_position();
    let visual_before = surface(&first).absolute_position();
    window
        .window()
        .dispatch_event(WindowEvent::PointerScrolled {
            position: scroll_position,
            delta_x: 0.0,
            delta_y: -40.0,
        });
    i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
    window.window().take_snapshot().unwrap();
    assert!(
        ElementHandle::find_by_element_id(&window, "BentoGrid::indicator")
            .next()
            .is_some(),
        "滚动首帧仍应保留可见的磁吸框"
    );
    let visual_after = surface(&first).absolute_position();
    let indicator_after = indicator.absolute_position();
    // 嵌套滚动与虚拟行修正均可能改变窗口坐标，应跟随实际卡片位移。
    assert!(
        visual_after.y < visual_before.y,
        "滚轮应穿过卡片观察器并滚动列表"
    );
    assert!(
        ((indicator_after.y - before.y) - (visual_after.y - visual_before.y)).abs() < 1.5,
        "滚动时框体与卡片应即时同步：框 {before:?} → {indicator_after:?}；卡片 {visual_before:?} → {visual_after:?}"
    );
    aligned(&indicator, &surface(&first));
    list.scroll(0.0, 100_000.0);
    window.window().take_snapshot().unwrap();
    hover(&window, &first);
    tick(400);
    hover(&window, &second);
    tick(45);
    window.global::<Motion>().set_reduce(true);
    slint::platform::update_timers_and_animations();
    aligned(&indicator, &surface(&second));
    assert_eq!(
        surface(&second).absolute_position().y,
        second.absolute_position().y
    );
    capture(&window, "motion-reduced", 1448, 1086);
    pointer(&window, LogicalPosition::new(8.0, 8.0));
    assert_eq!(indicator.computed_opacity(), 0.0, "减少动效不保留离开延迟");

    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "motion-200", width, height);
        let first = card(&window, "motion/project-000");
        assert!(
            ElementHandle::find_by_element_type_name(&window, "FavoriteCard").count() < 200,
            "200 项收藏应继续使用虚拟列表"
        );
        hover(&window, &first);
        aligned(&indicator, &surface(&first));
        assert!(first.size().width > 250.0);
        capture(&window, "motion-reduced-hover", width, height);
        favorites.set_sort(1 - favorites.get_sort());
        tick(1);
        assert_eq!(indicator.computed_opacity(), 0.0, "排序后清理目标");
        hover(&window, &first);
        favorites.set_filter(1 - favorites.get_filter());
        tick(1);
        assert_eq!(indicator.computed_opacity(), 0.0, "筛选后清理目标");
        hover(&window, &first);
        favorites.set_list(true);
        tick(1);
        assert_eq!(indicator.computed_opacity(), 0.0, "列表模式停用磁吸反馈");
        favorites.set_list(false);
        window.window().take_snapshot().unwrap();
        assert_eq!(
            title.computed_opacity(),
            1.0,
            "排序、筛选和窗口变化不重播标题"
        );
    }
    hover(&window, &card(&window, "motion/project-000"));
    model.remove(0);
    tick(1);
    assert_eq!(indicator.computed_opacity(), 0.0, "移除目标后不能留下旧框");
    assert_eq!(title.computed_opacity(), 1.0);
    window.global::<Workspace>().set_page(0);
    window.window().take_snapshot().unwrap();
    assert!(!indicator.is_valid(), "离开页面销毁瞬态目标");
    favorites.set_rows(rows(0).into());
    favorites.set_total(0);
    window.global::<Workspace>().set_page(2);
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "motion-empty", width, height);
        assert!(
            ElementHandle::find_by_accessible_label(&window, "还没有收藏项目")
                .next()
                .is_some()
        );
    }
    window.hide().unwrap();
}

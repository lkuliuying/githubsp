use super::*;
use crate::{HistoryRow, TaskRow};
use slint::{
    platform::{PointerEventButton, WindowEvent},
    LogicalPosition, VecModel,
};
use std::{cell::RefCell, rc::Rc};

fn element(window: &MainWindow, id: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(window, id)
        .next()
        .unwrap_or_else(|| panic!("应存在 {id}"))
}

fn button(window: &MainWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(window, label)
        .find(|item| {
            item.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .unwrap_or_else(|| panic!("应保留按钮 {label}"))
}

fn detail_contains(window: &MainWindow, kind: &str, text: &str) -> bool {
    let text = text.to_owned();
    ElementHandle::find_by_element_type_name(window, kind).any(|detail| {
        let text = text.clone();
        detail
            .query_descendants()
            .match_predicate(move |item| item.accessible_label().as_deref() == Some(text.as_str()))
            .find_first()
            .is_some()
    })
}

fn assert_panes(window: &MainWindow, list: &str, pane: &str) {
    let kind = if pane == "QueuePanel::task-scroll" {
        "TaskDetail"
    } else {
        "HistoryDetail"
    };
    let list = element(window, list);
    let pane = element(window, pane);
    assert!(
        list.absolute_position().x + list.size().width < pane.absolute_position().x,
        "列表与详情不得重叠"
    );
    assert!(pane.absolute_position().x + pane.size().width <= window.window().size().width as f32);
    let body = ElementHandle::find_by_element_type_name(window, kind)
        .next()
        .unwrap();
    assert!(
        (body.absolute_position().y - pane.absolute_position().y).abs() < 1.0,
        "详情应从侧栏顶部开始，不随窗口高度居中"
    );
}

fn click(window: &MainWindow, item: &ElementHandle) {
    slint::platform::update_timers_and_animations();
    let p = item.absolute_position();
    let size = item.size();
    let position = LogicalPosition::new(p.x + size.width / 2.0, p.y + size.height / 2.0);
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
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

pub(super) fn verify_master_detail() {
    let window = MainWindow::new().unwrap();
    presentation::bind_detail_selection(&window);
    window.global::<Motion>().set_reduce(true);
    let workspace = window.global::<Workspace>();
    let records = window.global::<History>();
    let tasks = Rc::new(VecModel::from((0..5).map(|index| TaskRow {
        id: format!("detail-{index}").into(),
        filename: ["GitHubSP-v0.3.0-windows-x64.exe", "GitHubSP-v0.2.8-windows-x64.exe", "GitHubSP-v0.2.7-windows-x64.exe", "SnowLuma-v2.1.1-win-x64.zip", "v2rayN-windows-64-desktop.zip"][index].into(),
        repository: "lkuliuying/githubsp".into(),
        state: if index == 0 { "正在下载" } else if index < 3 { "等待下载" } else { "下载完成" }.into(),
        progress: if index == 0 { 0.62 } else { 1.0 }, percent: if index == 0 { "62%" } else { "100%" }.into(),
        downloaded: "33.4 MB / 53.8 MB".into(), size: "53.8 MB".into(), speed: "2.4 MB/s".into(),
        route: "GH-Proxy".into(), eta: "—".into(), elapsed: "14 秒".into(),
        source_url: "https://github.com/lkuliuying/githubsp/releases/download/v0.3.0/GitHubSP-v0.3.0-windows-x64.exe".into(),
        save_path: "D:\\Downloads\\GitHubSP-v0.3.0-windows-x64.exe".into(),
        pause: index == 0, cancel: index < 3, resume: index > 0 && index < 3,
        queued: index > 0 && index < 3, completed: index >= 3,
        tone: if index > 0 && index < 3 { 3 } else { 1 }, ..Default::default()
    }).collect::<Vec<_>>()));
    let preview_tasks = tasks.iter().collect::<Vec<_>>();
    workspace.set_tasks(tasks.clone().into());
    workspace.set_total_tasks("15".into());
    workspace.set_running_tasks("1".into());
    workspace.set_queued_tasks("2".into());
    workspace.set_recovering_tasks("0".into());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    workspace.on_task_action(move |id, action| {
        recorded
            .borrow_mut()
            .push((id.to_string(), action.to_string()))
    });
    window.show().unwrap();
    capture(&window, "master-detail-queue", 1448, 1086);
    // 宽屏未选中行的操作在悬停时显现，按钮仍只调用原有业务动作。
    window.global::<Motion>().set_reduce(false);
    let resume = button(&window, "继续或重试下载");
    assert!(resume.computed_opacity() < 0.1);
    let row = button(&window, "查看任务详情：GitHubSP-v0.2.8-windows-x64.exe");
    let position = row.absolute_position();
    window.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(position.x + 30.0, position.y + 20.0),
    });
    slint::platform::update_timers_and_animations();
    std::thread::sleep(Duration::from_millis(150));
    slint::platform::update_timers_and_animations();
    assert!(resume.computed_opacity() > 0.9, "悬停后操作必须可见");
    click(&window, &resume);
    assert_eq!(
        calls.borrow().as_slice(),
        &[("detail-1".into(), "resume".into())]
    );
    calls.borrow_mut().clear();
    window.global::<Motion>().set_reduce(true);
    assert!(detail_contains(
        &window,
        "TaskDetail",
        "GitHubSP-v0.3.0-windows-x64.exe"
    ));
    click(
        &window,
        &button(&window, "查看任务详情：GitHubSP-v0.2.8-windows-x64.exe"),
    );
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    window.window().take_snapshot().unwrap();
    assert_eq!(workspace.get_expanded_task(), "detail-1");
    assert!(calls.borrow().is_empty(), "选择行不应新增业务操作");
    assert!(detail_contains(
        &window,
        "TaskDetail",
        "GitHubSP-v0.2.8-windows-x64.exe"
    ));
    let mut selected = tasks.row_data(1).unwrap();
    selected.filename = "刷新后的完整文件名.zip".into();
    tasks.set_row_data(1, selected.clone());
    window.window().take_snapshot().unwrap();
    assert!(
        detail_contains(&window, "TaskDetail", "刷新后的完整文件名.zip"),
        "原模型的行更新必须刷新详情"
    );
    let first = tasks.row_data(0).unwrap();
    tasks.set_vec(vec![selected.clone(), first.clone()]);
    window.window().take_snapshot().unwrap();
    assert!(
        detail_contains(&window, "TaskDetail", "刷新后的完整文件名.zip"),
        "重排不得按旧序号选错任务"
    );
    click(&window, &button(&window, "取消下载"));
    assert_eq!(
        calls.borrow().as_slice(),
        &[("detail-1".into(), "cancel".into())]
    );
    tasks.set_vec(vec![first.clone()]);
    window.window().take_snapshot().unwrap();
    assert!(
        detail_contains(&window, "TaskDetail", first.filename.as_str()),
        "删除目标后应选中仍存在的记录"
    );
    tasks.set_vec(preview_tasks);
    workspace.set_expanded_task("".into());
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "master-detail-queue", width, height);
        let list = element(&window, "QueuePanel::queue-list");
        assert!(list.size().height >= 120.0);
        if width >= 1040 {
            assert_panes(&window, "QueuePanel::queue-list", "QueuePanel::task-scroll");
        } else {
            button(&window, "查看任务详情：GitHubSP-v0.3.0-windows-x64.exe")
                .invoke_accessible_default_action();
            window.window().take_snapshot().unwrap();
            assert!(detail_contains(
                &window,
                "TaskDetail",
                first.filename.as_str()
            ));
            assert_button_in_window(&window, "收起任务详情", width, height);
            button(&window, "收起任务详情").invoke_accessible_default_action();
            assert_eq!(workspace.get_expanded_task(), "");
        }
    }
    tasks.set_vec(vec![]);
    capture(&window, "master-detail-queue-empty", 720, 520);
    assert!(
        ElementHandle::find_by_element_type_name(&window, "TaskDetail")
            .next()
            .is_none()
    );

    let history_rows = Rc::new(VecModel::from(
        (0..5)
            .map(|index| HistoryRow {
                id: format!("history-{index}").into(),
                name: if index == 0 {
                    "SnowLuma-v2.1.1-win-x64.zip".into()
                } else {
                    format!("GitHubSP-v0.2.{index}-windows-x64.exe").into()
                },
                repository: "SnowLuma/SnowLuma".into(),
                tag: "v2.1.1".into(),
                state: "下载完成".into(),
                file_state: "文件缺失".into(),
                tone: 2,
                date: "2026/10/23 16:28:22".into(),
                completed: "2026/10/23 16:28:22".into(),
                path: "D:\\Downloads\\SnowLuma-v2.1.1-win-x64.zip".into(),
                removable: true,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ));
    records.set_rows(history_rows.clone().into());
    records.set_total(15);
    records.set_completed(15);
    records.set_missing(15);
    workspace.set_page(1);
    for (width, height) in [(1448, 1086), (1040, 740), (720, 520)] {
        capture(&window, "master-detail-history", width, height);
        assert_button_in_window(&window, "跳转", width, height);
        if width >= 1040 {
            assert_panes(
                &window,
                "HistoryPage::records",
                "HistoryPage::history-scroll",
            );
        } else {
            button(&window, "查看历史详情：SnowLuma-v2.1.1-win-x64.zip")
                .invoke_accessible_default_action();
            window.window().take_snapshot().unwrap();
            let list = element(&window, "HistoryPage::records");
            let mut complete_visible = false;
            let mut missing_visible = false;
            for _ in 0..20 {
                window.window().take_snapshot().unwrap();
                complete_visible |= detail_contains(&window, "HistoryDetail", "下载完成");
                missing_visible |= detail_contains(&window, "HistoryDetail", "文件缺失");
                if complete_visible && missing_visible {
                    break;
                }
                list.scroll(0.0, -40.0);
            }
            assert!(
                complete_visible && missing_visible,
                "窄屏滚动后应能分别看到下载状态和文件状态"
            );
            capture(&window, "master-detail-history-inline", width, height);
        }
        if width >= 1040 {
            assert!(detail_contains(&window, "HistoryDetail", "下载完成"));
            assert!(detail_contains(&window, "HistoryDetail", "文件缺失"));
        }
    }
    capture(&window, "master-detail-history", 1448, 1086);
    click(&window, &button(&window, "重新下载"));
    assert_eq!(
        calls.borrow().last().unwrap(),
        &("history-0".into(), "redownload".into())
    );
    let mut updated = history_rows.row_data(0).unwrap();
    updated.name = "历史刷新后的完整文件名.zip".into();
    history_rows.set_row_data(0, updated);
    window.window().take_snapshot().unwrap();
    assert!(detail_contains(
        &window,
        "HistoryDetail",
        "历史刷新后的完整文件名.zip"
    ));
    button(&window, "查看历史详情：GitHubSP-v0.2.1-windows-x64.exe")
        .invoke_accessible_default_action();
    window.window().take_snapshot().unwrap();
    assert!(detail_contains(
        &window,
        "HistoryDetail",
        "GitHubSP-v0.2.1-windows-x64.exe"
    ));
    records.set_page(2);
    window.window().take_snapshot().unwrap();
    assert!(
        detail_contains(&window, "HistoryDetail", "历史刷新后的完整文件名.zip"),
        "翻页应清理上一页的选择"
    );
    let loads = Rc::new(RefCell::new(Vec::new()));
    let requested = loads.clone();
    records.on_load(move |page| requested.borrow_mut().push(page));
    window.global::<Updates>().set_dialog(true);
    window.window().take_snapshot().unwrap();
    for label in ["搜索 / 刷新", "跳转", "重置"] {
        let control = button(&window, label);
        assert_eq!(
            control.accessible_enabled(),
            Some(false),
            "更新弹窗应禁用历史页操作"
        );
        control.invoke_accessible_default_action();
    }
    assert!(loads.borrow().is_empty());
    window.global::<Updates>().set_dialog(false);
    window.window().take_snapshot().unwrap();
    button(&window, "搜索 / 刷新").invoke_accessible_default_action();
    assert_eq!(
        loads.borrow().as_slice(),
        &[1],
        "关闭弹窗后搜索仍从第一页加载"
    );
    history_rows.set_vec(vec![]);
    capture(&window, "master-detail-history-empty", 720, 520);
    assert!(
        ElementHandle::find_by_element_type_name(&window, "HistoryDetail")
            .next()
            .is_none()
    );
    window.hide().unwrap();
}

use crate::{MainWindow, RouteRow, TaskRow, Workspace};
use githubsp_lib::model::{Snapshot, Task, TaskStatus, Verification};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

pub const STATUSES: [TaskStatus; 12] = [
    TaskStatus::Queued,
    TaskStatus::Probing,
    TaskStatus::Downloading,
    TaskStatus::Retrying,
    TaskStatus::WaitingNetwork,
    TaskStatus::Verifying,
    TaskStatus::Pausing,
    TaskStatus::Cancelling,
    TaskStatus::Paused,
    TaskStatus::Completed,
    TaskStatus::Failed,
    TaskStatus::Cancelled,
];
pub const ROUTES: [(&str, &str); 4] = [
    ("", "自动选择"),
    ("github", "GitHub 直连"),
    ("gh-proxy", "GH-Proxy"),
    ("ghproxy-net", "ghproxy.net"),
];

pub fn status(value: TaskStatus) -> &'static str {
    match value {
        TaskStatus::Queued => "等待下载",
        TaskStatus::Probing => "检测下载线路",
        TaskStatus::Downloading => "正在下载",
        TaskStatus::Retrying => "等待重试",
        TaskStatus::WaitingNetwork => "等待线路恢复",
        TaskStatus::Verifying => "正在完成下载",
        TaskStatus::Pausing => "正在保存进度",
        TaskStatus::Cancelling => "正在取消",
        TaskStatus::Paused => "已暂停",
        TaskStatus::Completed => "下载完成",
        TaskStatus::Failed => "下载失败",
        TaskStatus::Cancelled => "已取消",
    }
}

pub fn bytes(value: u64) -> String {
    size(value as f64)
}
fn size(value: f64) -> String {
    if !value.is_finite() || value < 0.0 {
        return "未知大小".into();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut amount = value;
    let mut unit = 0;
    while amount >= 1000.0 && unit < 4 {
        amount /= 1000.0;
        unit += 1;
    }
    if amount >= 999.5 && unit < 4 {
        amount /= 1000.0;
        unit += 1;
    }
    if unit == 0 || amount >= 100.0 {
        format!("{amount:.0} {}", units[unit])
    } else {
        format!("{amount:.1} {}", units[unit])
    }
}

pub fn elapsed(value: Option<u64>) -> String {
    match value {
        None => "未知".into(),
        Some(0..1000) => "不足 1 秒".into(),
        Some(ms) => {
            let s = ms / 1000;
            if s < 60 {
                format!("{s} 秒")
            } else if s < 3600 {
                format!("{} 分 {} 秒", s / 60, s % 60)
            } else {
                format!("{} 小时 {:02} 分 {:02} 秒", s / 3600, s % 3600 / 60, s % 60)
            }
        }
    }
}

pub fn eta(value: Option<u64>) -> String {
    match value {
        None => "计算中".into(),
        Some(s) if s < 60 => format!("{} 秒", s.max(1)),
        Some(s) if s < 3600 => format!("{} 分钟", s.div_ceil(60)),
        Some(s) => format!("{} 小时 {} 分钟", s / 3600, (s % 3600).div_ceil(60)),
    }
}

pub fn pausable(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Queued
            | TaskStatus::Probing
            | TaskStatus::Downloading
            | TaskStatus::Retrying
            | TaskStatus::WaitingNetwork
            | TaskStatus::Verifying
    )
}

pub fn task_row(task: &Task, first: Option<&str>, last: Option<&str>) -> TaskRow {
    let total = task.total.map(bytes).unwrap_or_else(|| "未知大小".into());
    let mut failure = task
        .details
        .route_failures
        .iter()
        .map(|r| format!("{}：{}", r.route_name, r.message))
        .collect::<Vec<_>>();
    let integrity = task.verification == Verification::Failed
        || task
            .details
            .failure
            .as_ref()
            .is_some_and(|f| f.category == "integrity");
    if let Some(error) = &task.error {
        failure.insert(
            0,
            if integrity {
                "下载文件内容异常，请重新下载。".into()
            } else {
                error.clone()
            },
        );
    }
    if let Some(info) = &task.details.failure {
        failure.push(if integrity {
            "请切换线路重新下载；不要运行不完整文件。".into()
        } else {
            info.action.clone()
        });
    }
    let speed =
        if task.status == TaskStatus::Downloading && task.speed.is_finite() && task.speed >= 0.0 {
            format!("{}/s", size(task.speed))
        } else if task.status == TaskStatus::Completed
            && !task.details.elapsed_is_partial
            && task.details.elapsed_ms.is_some_and(|ms| ms > 0)
        {
            format!(
                "平均 {}/s",
                size(
                    task.total.unwrap_or(task.downloaded) as f64
                        / task.details.elapsed_ms.unwrap() as f64
                        * 1000.0
                )
            )
        } else {
            "—".into()
        };
    let feedback = if task.status == TaskStatus::WaitingNetwork {
        task.details
            .recovery_info
            .as_ref()
            .map(|r| {
                format!(
                    "线路暂时不可用，已保留当前进度。{} 剩余自动恢复额度 {}。",
                    if r.waiting_for_slot {
                        "正在下载其他任务，恢复计时已暂停。".into()
                    } else {
                        format!("{} 秒后重新检测线路。", r.retry_in_ms.div_ceil(1000))
                    },
                    elapsed(Some(r.remaining_ms))
                )
            })
            .unwrap_or_else(|| "等待下载位置空闲后重新检测。".into())
    } else {
        task.details
            .retry_info
            .as_ref()
            .map(|r| {
                format!(
                    "{}，{} 秒后重试（第 {}/{} 次）",
                    r.reason,
                    r.retry_in_ms.div_ceil(1000),
                    r.attempt,
                    r.max_attempts
                )
            })
            .unwrap_or_default()
    };
    let progress = if task.status == TaskStatus::Completed {
        1.0
    } else {
        task.total.filter(|t| *t > 0).map_or(0.0, |t| {
            (task.downloaded as f64 / t as f64).clamp(0.0, 1.0) as f32
        })
    };
    TaskRow {
        id: task.id.as_str().into(), filename: task.filename.as_str().into(), state: status(task.status).into(),
        tone: if matches!(task.status, TaskStatus::Downloading | TaskStatus::Completed) { 1 } else if task.status == TaskStatus::Failed { 2 } else if task.status == TaskStatus::WaitingNetwork { 5 } else if task.status == TaskStatus::Queued { 3 } else if task.status.running() { 4 } else { 0 },
        size: total.clone().into(), progress,
        percent: if task.total.is_some_and(|t|t>0) || task.status == TaskStatus::Completed { format!("{:.0}%", progress*100.0).into() } else { "—".into() },
        speed: speed.into(), eta: if task.status == TaskStatus::Downloading { eta(task.eta).into() } else if task.status == TaskStatus::Completed { "已完成".into() } else { "—".into() },
        route: task.route.as_deref().unwrap_or("—").into(),
        source_url: task.url.as_str().into(),
        save_path: task.final_path.as_ref().unwrap_or(&task.directory).display().to_string().into(),
        downloaded: format!("{} / {}", bytes(task.downloaded), total).into(),
        elapsed: format!("{}{}", elapsed(task.details.elapsed_ms), if task.details.elapsed_is_partial { "（记录不完整）" } else { "" }).into(),
        failure: failure.join("\n").into(), feedback: feedback.into(),
        suggestion: task.details.route_suggestion.as_ref().map(|s|format!("持续低速，可尝试 {}\n当前线路预计还需 {}；换线从头下载预计需 {}。将重新下载已完成的 {}。", s.route_name, eta(Some(s.current_seconds)), eta(Some(s.suggested_seconds)), bytes(s.restart_bytes))).unwrap_or_default().into(),
        suggestion_id: task.details.route_suggestion.as_ref().map(|s|s.id.as_str()).unwrap_or("").into(),
        resume: task.status.resumable() || task.status == TaskStatus::WaitingNetwork, pause: pausable(task.status),
        cancel: !matches!(task.status, TaskStatus::Completed | TaskStatus::Cancelled), stopping: matches!(task.status, TaskStatus::Pausing | TaskStatus::Cancelling), completed: task.status == TaskStatus::Completed,
        removable: matches!(task.status, TaskStatus::Completed | TaskStatus::Cancelled), queued: task.status == TaskStatus::Queued,
        first: first == Some(task.id.as_str()), last: last == Some(task.id.as_str()), repository: task.details.repository.as_deref().unwrap_or("").into(),
        preferred_route: ROUTES.iter().position(|r|Some(r.0) == task.details.preferred_route.as_deref()).unwrap_or(0) as i32,
        route_editable: matches!(task.status, TaskStatus::Queued | TaskStatus::Paused | TaskStatus::Failed | TaskStatus::WaitingNetwork),
    }
}

pub fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}
pub fn update_model<T: Clone + PartialEq + 'static>(old: ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(current) = old.as_any().downcast_ref::<VecModel<T>>() {
        if current.row_count() != rows.len() {
            current.set_vec(rows);
        } else {
            for (index, row) in rows.into_iter().enumerate() {
                if current.row_data(index).as_ref() != Some(&row) {
                    current.set_row_data(index, row);
                }
            }
        }
        old
    } else {
        model(rows)
    }
}

pub fn apply(window: &MainWindow, snapshot: &Snapshot) {
    let ui = window.global::<Workspace>();
    ui.set_total_tasks(snapshot.total_tasks.to_string().into());
    ui.set_running_tasks(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.status.running())
            .count()
            .to_string()
            .into(),
    );
    ui.set_queued_tasks(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Queued)
            .count()
            .to_string()
            .into(),
    );
    ui.set_recovering_tasks(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::WaitingNetwork)
            .count()
            .to_string()
            .into(),
    );
    let queued: Vec<_> = snapshot
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Queued)
        .collect();
    let first = queued.first().map(|t| t.id.as_str());
    let last = queued.last().map(|t| t.id.as_str());
    ui.set_tasks(update_model(
        ui.get_tasks(),
        snapshot
            .tasks
            .iter()
            .map(|t| task_row(t, first, last))
            .collect(),
    ));
    ui.set_status(
        format!(
            "共 {} 个任务 · {} 个传输中 · {} 个排队中 · {} 个等待线路恢复（保留最近 50 条结束记录，完整记录见历史）",
            snapshot.total_tasks,
            snapshot.tasks.iter().filter(|t| t.status.running()).count(),
            queued.len(),
            snapshot
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::WaitingNetwork)
                .count()
        )
        .into(),
    );
    ui.set_can_pause(snapshot.tasks.iter().any(|t| pausable(t.status)));
    ui.set_can_resume(snapshot.tasks.iter().any(|t| t.status.resumable()));
    ui.set_can_clear(snapshot.completed_tasks > 0);
    ui.set_notices(
        snapshot
            .notices
            .iter()
            .rev()
            .map(|n| n.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .into(),
    );
    ui.set_diagnosing(snapshot.diagnosing);
    ui.set_route_target(
        snapshot
            .diagnostic_context
            .as_ref()
            .map(|c| {
                format!(
                    "{}：{}",
                    match c.source {
                        githubsp_lib::model::DiagnosticSource::Default => "默认测试文件",
                        githubsp_lib::model::DiagnosticSource::Input => "指定附件",
                        githubsp_lib::model::DiagnosticSource::Download => "下载自动检测",
                    },
                    c.filename
                )
            })
            .unwrap_or_default()
            .into(),
    );
    let routes = ROUTES
        .iter()
        .skip(1)
        .map(|(id, name)| {
            let report = snapshot
                .diagnostics
                .iter()
                .find(|r| r.id == *id && r.checked_at > 0)
                .filter(|_| !snapshot.diagnosing);
            RouteRow {
                id: (*id).into(),
                name: (*name).into(),
                speed: report
                    .filter(|r| r.available)
                    .map(|r| format!("{}/s", size(r.bytes_per_second)))
                    .unwrap_or_else(|| "—".into())
                    .into(),
                state: if snapshot.diagnosing {
                    "检测中"
                } else {
                    report.map_or("尚未检测", |r| {
                        if r.available {
                            "可用"
                        } else {
                            "不可用"
                        }
                    })
                }
                .into(),
                detail: report.and_then(|r| r.error.as_deref()).unwrap_or("").into(),
                tone: report.map_or(0, |r| if r.available { 1 } else { 2 }),
            }
        })
        .collect();
    ui.set_routes(update_model(ui.get_routes(), routes));
    let best = snapshot
        .diagnostics
        .iter()
        .filter(|r| !snapshot.diagnosing && r.available && r.checked_at > 0)
        .max_by(|a, b| a.bytes_per_second.total_cmp(&b.bytes_per_second));
    let checked = !snapshot.diagnosing && snapshot.diagnostics.iter().any(|r| r.checked_at > 0);
    ui.set_route_best_name(best.map_or("", |r| r.name.as_str()).into());
    ui.set_route_best_speed(
        best.map(|r| format!("{}/s", size(r.bytes_per_second)))
            .unwrap_or_default()
            .into(),
    );
    ui.set_route_tone(if best.is_some() {
        1
    } else if checked {
        2
    } else {
        0
    });
    ui.set_route_result(
        if snapshot.diagnosing {
            "正在依次检测内置线路，请稍候。"
        } else if best.is_some() {
            if snapshot.diagnostic_context.is_some() {
                "实际下载仍会针对目标文件选择线路。"
            } else {
                "历史检测结果，重新检测可获取当前速度。"
            }
        } else if checked {
            "本轮线路均不可用，请稍后重试或更换附件直链。"
        } else {
            "点击检测线路，比较当前网络下的可用性与速度。"
        }
        .into(),
    );
    if let Some(error) = &snapshot.error {
        window.set_message(error.as_str().into());
    }
    crate::favorites::apply(window, snapshot);
    let history = window.global::<crate::History>();
    if ui.get_page() == 1
        && !history.get_busy()
        && history.get_revision().as_str() != snapshot.history_revision.to_string()
    {
        history.invoke_load(history.get_page());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_and_statuses_match_webview() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1024), "1.0 KB");
        assert_eq!(bytes(999_999), "1.0 MB");
        assert_eq!(status(TaskStatus::WaitingNetwork), "等待线路恢复");
        assert!(!pausable(TaskStatus::Pausing));
        assert!(pausable(TaskStatus::WaitingNetwork));
        assert_eq!(elapsed(Some(3_661_000)), "1 小时 01 分 01 秒");
    }
}

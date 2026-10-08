use crate::model::{NoticeKind, Snapshot};
use serde::Serialize;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
const DISPLAY_TIME: Duration = Duration::from_secs(8);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionNotice {
    pub count: u32,
    pub filename: String,
    pub elapsed_ms: Option<u64>,
    pub elapsed_is_partial: bool,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeView {
    pub revision: u64,
    pub notice: Option<CompletionNotice>,
}

#[derive(Default)]
pub struct NoticeState {
    pub(crate) view: NoticeView,
    pub(crate) snapshot_revision: Option<u64>,
    pub(crate) seen: HashSet<String>,
    pub(crate) presented: Option<u64>,
    pub(crate) deadline: Option<Instant>,
    pub(crate) remaining: Duration,
    pub(crate) hovering: bool,
    pub(crate) focused: bool,
}

impl NoticeState {
    pub fn view(&self) -> &NoticeView {
        &self.view
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn ingest(&mut self, snapshot: &Snapshot, background: bool) -> bool {
        if self
            .snapshot_revision
            .is_some_and(|r| snapshot.revision <= r)
        {
            return false;
        }
        self.snapshot_revision = Some(snapshot.revision);
        let mut changed = false;
        for notice in &snapshot.notices {
            if !self.seen.contains(&notice.id)
                && notice.kind == NoticeKind::DownloadCompleted
                && snapshot.settings.background_completion_notice
                && background
            {
                if let Some(task) = &notice.completion {
                    let count = self
                        .view
                        .notice
                        .as_ref()
                        .map_or(1, |n| n.count.saturating_add(1));
                    self.view.notice = Some(CompletionNotice {
                        count,
                        filename: task.filename.clone(),
                        elapsed_ms: task.elapsed_ms,
                        elapsed_is_partial: task.elapsed_is_partial,
                    });
                    changed = true;
                }
            }
        }
        // 已读清理和旧快照不能重放提醒；只保留当前最多 100 个通知标识。
        self.seen = snapshot.notices.iter().map(|n| n.id.clone()).collect();
        if !snapshot.settings.background_completion_notice || !background {
            return self.dismiss(None);
        }
        if changed {
            self.view.revision += 1;
            self.presented = None;
            self.deadline = None;
            self.remaining = DISPLAY_TIME;
        }
        changed
    }

    pub fn dismiss(&mut self, revision: Option<u64>) -> bool {
        if revision.is_some_and(|r| r != self.view.revision) || self.view.notice.is_none() {
            return false;
        }
        self.view.revision += 1;
        self.view.notice = None;
        self.presented = None;
        self.deadline = None;
        self.hovering = false;
        self.focused = false;
        true
    }

    pub fn presented(&mut self, revision: u64, now: Instant) {
        if revision != self.view.revision
            || self.view.notice.is_none()
            || self.presented == Some(revision)
        {
            return;
        }
        self.presented = Some(revision);
        if !self.hovering && !self.focused {
            self.deadline = Some(now + self.remaining);
        }
    }

    pub fn interaction(&mut self, hovering: Option<bool>, focused: Option<bool>, now: Instant) {
        let was_paused = self.hovering || self.focused;
        if let Some(value) = hovering {
            self.hovering = value;
        }
        if let Some(value) = focused {
            self.focused = value;
        }
        let paused = self.hovering || self.focused;
        if paused && !was_paused {
            if let Some(deadline) = self.deadline.take() {
                self.remaining = deadline.saturating_duration_since(now);
            }
        } else if !paused
            && was_paused
            && self.presented == Some(self.view.revision)
            && self.view.notice.is_some()
        {
            self.deadline = Some(now + self.remaining);
        }
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.dismiss(None)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notice, Settings, Task, TaskDetails, TaskStatus, Verification};

    fn snapshot(revision: u64, ids: &[&str]) -> Snapshot {
        Snapshot {
            total_tasks: ids.len(),
            completed_tasks: ids.len(),
            history_revision: revision,
            revision,
            last_directory: None,
            error: None,
            settings: Settings::default(),
            queue_revision: 0,
            diagnostics: vec![],
            diagnostic_context: None,
            diagnosing: false,
            favorites: vec![],
            tasks: ids
                .iter()
                .map(|id| Task {
                    id: id.to_string(),
                    url: String::new(),
                    filename: format!("{id}.zip"),
                    directory: Default::default(),
                    status: TaskStatus::Completed,
                    downloaded: 1,
                    total: Some(1),
                    speed: 0.0,
                    eta: None,
                    route: None,
                    verification: Verification::Unverified,
                    error: None,
                    final_path: None,
                    created_at: 0,
                    revision: 1,
                    details: TaskDetails {
                        elapsed_ms: Some(1234),
                        ..Default::default()
                    },
                })
                .collect(),
            notices: ids
                .iter()
                .map(|id| Notice {
                    completion: Some(crate::model::CompletionSummary {
                        filename: format!("{id}.zip"),
                        elapsed_ms: Some(1234),
                        elapsed_is_partial: false,
                    }),
                    id: id.to_string(),
                    kind: NoticeKind::DownloadCompleted,
                    task_id: Some(id.to_string()),
                    message: String::new(),
                    created_at: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn completion_summary_survives_eviction_from_download_working_set() {
        let mut event = snapshot(1, &["old-download"]);
        event.tasks.clear();
        event.total_tasks = 1000;
        let mut state = NoticeState::default();
        assert!(state.ingest(&event, true));
        let notice = state.view.notice.as_ref().unwrap();
        assert_eq!(notice.filename, "old-download.zip");
        assert_eq!(notice.elapsed_ms, Some(1234));
    }

    #[test]
    fn only_background_completions_show_and_foreground_events_never_replay() {
        let mut state = NoticeState::default();
        assert!(!state.ingest(&snapshot(1, &["front"]), false));
        assert!(!state.ingest(&snapshot(2, &["front"]), true));
        assert!(state.ingest(&snapshot(3, &["front", "back"]), true));
        assert_eq!(state.view.notice.as_ref().unwrap().count, 1);
        assert!(!state.ingest(&snapshot(3, &["front", "back"]), true));
        assert!(!state.ingest(&snapshot(2, &["old"]), true));
        let mut failed = snapshot(4, &["front", "back", "failure"]);
        failed.notices[2].kind = NoticeKind::DownloadFailed;
        assert!(!state.ingest(&failed, true));
        assert!(state.ingest(&snapshot(5, &["front", "back"]), false));
        assert!(state.view.notice.is_none());
        assert!(!state.ingest(&snapshot(6, &["front", "back"]), true));
    }

    #[test]
    fn switch_off_dismisses_and_does_not_replay_after_enabling() {
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        let mut disabled = snapshot(2, &["one", "two"]);
        disabled.settings.background_completion_notice = false;
        assert!(state.ingest(&disabled, true));
        assert!(state.view.notice.is_none());
        assert!(!state.ingest(&snapshot(3, &["one", "two"]), true));
    }

    #[test]
    fn merge_restarts_timeout_and_stale_callbacks_cannot_close_new_content() {
        let now = Instant::now();
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        let old = state.view.revision;
        state.presented(old, now);
        assert!(!state.expire(now + Duration::from_secs(7)));
        state.ingest(&snapshot(2, &["one", "two"]), true);
        assert_eq!(state.view.notice.as_ref().unwrap().count, 2);
        assert!(!state.dismiss(Some(old)));
        state.presented(old, now + Duration::from_secs(7));
        assert!(state.deadline.is_none());
        state.presented(state.view.revision, now + Duration::from_secs(7));
        assert!(!state.expire(now + Duration::from_secs(8)));
        assert!(state.expire(now + Duration::from_secs(15)));
        assert!(!state.ingest(&snapshot(3, &["one", "two"]), true));
    }

    #[test]
    fn hover_and_keyboard_focus_independently_pause_remaining_time() {
        let now = Instant::now();
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        state.presented(state.view.revision, now);
        state.interaction(Some(true), None, now + Duration::from_secs(2));
        state.interaction(None, Some(true), now + Duration::from_secs(3));
        state.interaction(Some(false), None, now + Duration::from_secs(5));
        assert!(!state.expire(now + Duration::from_secs(100)));
        state.interaction(None, Some(false), now + Duration::from_secs(100));
        assert!(!state.expire(now + Duration::from_secs(105)));
        assert!(state.expire(now + Duration::from_secs(106)));
    }

    #[test]
    fn failed_presentation_is_discarded_once_and_later_completions_can_show() {
        let mut state = NoticeState::default();
        state.ingest(&snapshot(1, &["one"]), true);
        assert!(state.dismiss(None));
        assert!(!state.ingest(&snapshot(2, &["one"]), true));
        assert!(state.ingest(&snapshot(3, &["one", "two"]), true));
        assert_eq!(state.view.notice.as_ref().unwrap().count, 1);
        assert_eq!(state.view.notice.as_ref().unwrap().filename, "two.zip");
    }
}

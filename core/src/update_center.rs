use crate::{
    catalog::Release,
    error::{DownloadError, ErrorKind, Result},
    model::{now_ms, ReleaseSummary},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEvent {
    pub id: u64,
    pub repository: String,
    pub release: ReleaseSummary,
    /// 列表省略正文，详情返回完整来源正文；不截断后冒充完整说明。
    pub notes: Option<String>,
    pub metadata_complete: bool,
    pub prerelease: bool,
    pub discovered_at: u64,
    pub seen_at: Option<u64>,
    pub skipped: bool,
    pub downloaded_tasks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ledger {
    pub schema: u32,
    pub epoch: String,
    pub events: Vec<UpdateEvent>,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            schema: 1,
            epoch: uuid::Uuid::new_v4().to_string(),
            events: Vec::new(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePage {
    pub events: Vec<UpdateEvent>,
    pub next_cursor: String,
    pub has_more: bool,
    pub notes_omitted: bool,
}

impl Ledger {
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 || uuid::Uuid::parse_str(&self.epoch).is_err() {
            return Err(DownloadError::new(
                ErrorKind::Storage,
                "更新记录版本不受支持或游标标识损坏，原数据已保留",
            ));
        }
        Ok(())
    }
    pub fn append(
        &mut self,
        repository: &str,
        previous: Option<&ReleaseSummary>,
        latest: &ReleaseSummary,
        full: Option<&Release>,
    ) -> Result<bool> {
        self.validate()?;
        if let Some(event) = self.events.iter_mut().find(|event| {
            event.repository.eq_ignore_ascii_case(repository) && event.release.tag == latest.tag
        }) {
            if let Some(full) = full.filter(|release| !release.atom) {
                event.release = latest.clone();
                event.notes = Some(full.notes.clone());
                event.metadata_complete = true;
                event.prerelease = full.prerelease;
            }
            return Ok(false);
        }
        if previous.is_none_or(|old| old.tag == latest.tag) {
            return Ok(false);
        }
        self.events.push(UpdateEvent {
            id: self.events.last().map_or(1, |event| event.id + 1),
            repository: repository.to_owned(),
            release: latest.clone(),
            notes: full.map(|release| release.notes.clone()),
            metadata_complete: full.is_some_and(|release| !release.atom),
            prerelease: full.is_some_and(|release| release.prerelease),
            discovered_at: now_ms(),
            seen_at: None,
            skipped: false,
            downloaded_tasks: Vec::new(),
        });
        Ok(true)
    }
    pub fn page(
        &self,
        cursor: Option<&str>,
        limit: u32,
        repository: Option<&str>,
    ) -> Result<UpdatePage> {
        self.validate()?;
        if !(1..=100).contains(&limit) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "更新列表每页需为 1–100 条",
            ));
        }
        let after = self.position(cursor)?.unwrap_or(0);
        let mut matching = self.events.iter().filter(|event| {
            event.id > after
                && repository.is_none_or(|repo| repo.eq_ignore_ascii_case(&event.repository))
        });
        let events: Vec<_> = matching
            .by_ref()
            .take(limit as usize)
            .cloned()
            .map(|mut event| {
                event.notes = None;
                event
            })
            .collect();
        let has_more = matching.next().is_some();
        let position = if has_more {
            events.last().map_or(after, |event| event.id)
        } else {
            self.events.last().map_or(0, |event| event.id)
        };
        Ok(UpdatePage {
            events,
            next_cursor: format!("{}:{position}", self.epoch),
            has_more,
            notes_omitted: true,
        })
    }
    pub fn recent(
        &self,
        cursor: Option<&str>,
        limit: u32,
        repository: Option<&str>,
    ) -> Result<UpdatePage> {
        self.validate()?;
        if !(1..=100).contains(&limit) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "更新列表每页需为 1–100 条",
            ));
        }
        let before = self.position(cursor)?.unwrap_or(u64::MAX);
        let mut matching = self.events.iter().rev().filter(|event| {
            event.id < before
                && repository.is_none_or(|repo| repo.eq_ignore_ascii_case(&event.repository))
        });
        let events: Vec<_> = matching
            .by_ref()
            .take(limit as usize)
            .cloned()
            .map(|mut event| {
                event.notes = None;
                event
            })
            .collect();
        let has_more = matching.next().is_some();
        let position = events.last().map_or(0, |event| event.id);
        Ok(UpdatePage {
            events,
            next_cursor: format!("{}:{position}", self.epoch),
            has_more,
            notes_omitted: true,
        })
    }
    fn position(&self, cursor: Option<&str>) -> Result<Option<u64>> {
        let Some(cursor) = cursor else {
            return Ok(None);
        };
        let (epoch, position) = cursor
            .rsplit_once(':')
            .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "更新游标格式无效"))?;
        let position: u64 = position
            .parse()
            .map_err(|_| DownloadError::new(ErrorKind::InvalidInput, "更新游标位置无效"))?;
        if epoch != self.epoch || position > self.events.last().map_or(0, |event| event.id) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "更新游标不属于当前数据或超出恢复点，请重新读取基线",
            ));
        }
        Ok(Some(position))
    }
    pub fn event(&self, id: u64) -> Result<&UpdateEvent> {
        self.validate()?;
        self.events
            .iter()
            .find(|event| event.id == id)
            .ok_or_else(|| DownloadError::new(ErrorKind::NotFound, "更新记录不存在"))
    }
    pub fn action(&mut self, id: u64, action: &str) -> Result<()> {
        self.event(id)?;
        let event = self
            .events
            .iter_mut()
            .find(|event| event.id == id)
            .expect("已检查记录存在");
        match action {
            "seen" => {
                event.seen_at.get_or_insert_with(now_ms);
            }
            "skip" => {
                event.skipped = true;
                event.seen_at.get_or_insert_with(now_ms);
            }
            "unskip" => event.skipped = false,
            _ => {
                return Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    "更新处理只支持 seen/skip/unskip",
                ))
            }
        }
        Ok(())
    }
    pub fn downloaded(&mut self, repository: &str, tag: &str, task_id: &str) -> Result<bool> {
        self.validate()?;
        let mut changed = false;
        for event in self.events.iter_mut().filter(|event| {
            event.repository.eq_ignore_ascii_case(repository) && event.release.tag == tag
        }) {
            if !event.downloaded_tasks.iter().any(|id| id == task_id) {
                event.downloaded_tasks.push(task_id.to_owned());
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(id: u64) -> ReleaseSummary {
        ReleaseSummary {
            id,
            tag: format!("v{id}"),
            url: format!("https://github.com/o/r/releases/tag/v{id}"),
            published_at: Some(id),
            atom: false,
        }
    }
    #[test]
    fn baseline_dedup_retry_cursor_and_explicit_dispositions() {
        let mut ledger = Ledger::default();
        assert!(!ledger.append("o/r", None, &release(1), None).unwrap());
        let empty = ledger.page(None, 1, None).unwrap();
        assert!(ledger
            .append("o/r", Some(&release(1)), &release(2), None)
            .unwrap());
        assert!(!ledger
            .append("O/R", Some(&release(1)), &release(2), None)
            .unwrap());
        ledger
            .append("o/r", Some(&release(2)), &release(3), None)
            .unwrap();
        let page = ledger.page(Some(&empty.next_cursor), 1, None).unwrap();
        let retried = ledger.page(Some(&empty.next_cursor), 1, None).unwrap();
        assert_eq!(page.events[0].id, retried.events[0].id);
        assert!(page.has_more);
        let newest = ledger.recent(None, 1, None).unwrap();
        assert_eq!(newest.events[0].release.id, 3);
        assert_eq!(
            ledger
                .recent(Some(&newest.next_cursor), 1, None)
                .unwrap()
                .events[0]
                .release
                .id,
            2
        );
        assert!(ledger.event(1).unwrap().seen_at.is_none());
        assert_eq!(
            ledger
                .page(Some(&page.next_cursor), 1, None)
                .unwrap()
                .events[0]
                .release
                .id,
            3
        );
        ledger.action(1, "skip").unwrap();
        let seen = ledger.event(1).unwrap().seen_at;
        ledger.action(1, "seen").unwrap();
        assert_eq!(ledger.event(1).unwrap().seen_at, seen);
        assert!(ledger.event(1).unwrap().skipped);
        assert!(ledger.downloaded("o/r", "v2", "task-1").unwrap());
        assert!(!ledger.downloaded("o/r", "v2", "task-1").unwrap());
        assert!(ledger.page(Some("other:1"), 10, None).is_err());
        let restored: Ledger =
            serde_json::from_str(&serde_json::to_string(&ledger).unwrap()).unwrap();
        assert_eq!(
            restored
                .page(Some(&page.next_cursor), 10, None)
                .unwrap()
                .events
                .len(),
            1
        );
    }
}

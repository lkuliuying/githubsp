use crate::{service::Service, Favorites, History, MainWindow, Preferences, Sources, Updates};
use slint::ComponentHandle;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Clone, Default)]
pub struct Epoch(Arc<AtomicU64>);

#[derive(Clone)]
pub struct Ticket {
    epoch: Epoch,
    value: u64,
}

impl Epoch {
    pub fn invalidate(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
    pub fn ticket(&self) -> Ticket {
        Ticket {
            epoch: self.clone(),
            value: self.0.load(Ordering::Acquire),
        }
    }
}

impl Ticket {
    pub fn current(&self) -> bool {
        self.epoch.0.load(Ordering::Acquire) == self.value
    }
    pub fn ensure_current(&self, service: &Service) -> Result<(), String> {
        if self.current() && !service.is_closing() {
            Ok(())
        } else {
            Err("操作已取消或页面内容已变化，请重新操作。".into())
        }
    }
}

#[derive(Clone, Copy)]
pub enum Area {
    Commands,
    Diagnostics,
    Sources,
    History,
    Favorites,
    Preferences,
    Storage,
    UpdateCheck,
    UpdateList,
    UpdateDownload,
}

impl Area {
    pub fn busy(self, window: &MainWindow) -> bool {
        match self {
            Self::Commands => window.get_busy(),
            Self::Diagnostics => window.global::<crate::Workspace>().get_diagnosing(),
            Self::Sources => window.global::<Sources>().get_busy(),
            Self::History => window.global::<History>().get_busy(),
            Self::Favorites => window.global::<Favorites>().get_busy(),
            Self::Preferences => window.global::<Preferences>().get_busy(),
            Self::Storage => window.global::<crate::Storage>().get_busy(),
            Self::UpdateCheck => window.global::<Updates>().get_busy(),
            Self::UpdateList => {
                window.global::<Updates>().get_listing()
                    || window.global::<Updates>().get_stage() != 0
            }
            Self::UpdateDownload => {
                window.global::<Updates>().get_stage() != 0
                    || window.global::<Updates>().get_listing()
            }
        }
    }
    fn set_busy(self, window: &MainWindow, value: bool) {
        match self {
            Self::Commands => window.set_busy(value),
            Self::Diagnostics => window.global::<crate::Workspace>().set_diagnosing(value),
            Self::Sources => window.global::<Sources>().set_busy(value),
            Self::History => window.global::<History>().set_busy(value),
            Self::Favorites => window.global::<Favorites>().set_busy(value),
            Self::Preferences => window.global::<Preferences>().set_busy(value),
            Self::Storage => window.global::<crate::Storage>().set_busy(value),
            Self::UpdateCheck => window.global::<Updates>().set_busy(value),
            Self::UpdateList => window.global::<Updates>().set_listing(value),
            Self::UpdateDownload => window
                .global::<Updates>()
                .set_stage(if value { 1 } else { 0 }),
        }
    }
    pub fn message(self, window: &MainWindow, text: String) {
        match self {
            Self::Commands | Self::Diagnostics => window.set_message(text.into()),
            Self::Sources => window.global::<Sources>().set_message(text.into()),
            Self::History => window.global::<History>().set_message(text.into()),
            Self::Favorites => window.global::<Favorites>().set_message(text.into()),
            Self::Preferences => window.global::<Preferences>().set_message(text.into()),
            Self::Storage => window.global::<crate::Storage>().set_message(text.into()),
            Self::UpdateCheck => window.global::<Updates>().set_message(text.into()),
            Self::UpdateList | Self::UpdateDownload => {
                window.global::<Updates>().set_dialog_message(text.into())
            }
        }
    }
}

pub fn run<T, F>(
    window: &MainWindow,
    service: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
    area: Area,
    ticket: Ticket,
    future: F,
    done: impl FnOnce(&MainWindow, T) + Send + 'static,
) where
    T: Send + 'static,
    F: Future<Output = Result<T, String>> + Send + 'static,
{
    if area.busy(window) || service.is_closing() {
        return;
    }
    area.set_busy(window, true);
    area.message(window, String::new());
    let weak = window.as_weak();
    let service = service.clone();
    let worker = runtime.spawn(future);
    runtime.spawn(async move {
        let result = worker
            .await
            .unwrap_or_else(|error| Err(format!("后台操作异常，请重试：{error}")));
        if service.is_closing() {
            return;
        }
        let _ = weak.upgrade_in_event_loop(move |window| {
            area.set_busy(&window, false);
            if !ticket.current() {
                if matches!(area, Area::History)
                    && window.global::<crate::Workspace>().get_page() == 1
                {
                    window.global::<History>().invoke_load(1);
                }
                return;
            }
            match result {
                Ok(value) => done(&window, value),
                Err(error) => area.message(&window, error),
            }
        });
    });
}

pub async fn confirmed(title: &str, text: impl Into<String>) -> bool {
    rfd::AsyncMessageDialog::new()
        .set_title(title)
        .set_description(text)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await
        == rfd::MessageDialogResult::Yes
}

pub async fn choose_directory(start: &str) -> Result<Option<std::path::PathBuf>, String> {
    let mut picker = rfd::AsyncFileDialog::new().set_title("选择已存在的保存目录");
    if std::path::Path::new(start).is_dir() {
        picker = picker.set_directory(start);
    }
    let Some(path) = picker.pick_folder().await else {
        return Ok(None);
    };
    let inspected = githubsp_lib::preflight::inspect_directory_async(path.path().to_owned())
        .await
        .map_err(|e| e.to_string())?;
    if inspected.state != githubsp_lib::preflight::DirectoryState::Existing {
        return Err("所选目录已不存在，请重新选择目录".into());
    }
    Ok(Some(inspected.directory))
}

pub async fn prepare_directory(
    input: String,
    selected: bool,
    ticket: &Ticket,
    service: &Service,
) -> Result<Option<std::path::PathBuf>, String> {
    use githubsp_lib::preflight::{self, DirectoryState};
    let inspected = preflight::inspect_directory_async(input.into())
        .await
        .map_err(|e| e.to_string())?;
    ticket.ensure_current(service)?;
    if inspected.state == DirectoryState::Missing {
        if selected {
            return Err("所选目录已不存在，请重新选择目录".into());
        }
        if !confirmed(
            "当前目录不存在，是否新建目录？",
            format!(
                "{}\n将创建该目录及缺失的父目录，然后继续刚才的操作。",
                inspected.directory.display()
            ),
        )
        .await
        {
            return Ok(None);
        }
        ticket.ensure_current(service)?;
        preflight::create_directory_async(inspected.directory.clone())
            .await
            .map_err(|e| e.to_string())?;
    }
    ticket.ensure_current(service)?;
    Ok(Some(inspected.directory))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_or_closing_invalidates_pending_results() {
        let epoch = Epoch::default();
        let old = epoch.ticket();
        assert!(old.current());
        epoch.invalidate();
        assert!(!old.current());
        assert!(epoch.ticket().current());
    }
}

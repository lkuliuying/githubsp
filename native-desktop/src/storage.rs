use crate::{
    bridge::{self, Area, Epoch},
    service::Service,
    MainWindow, Storage,
};
use slint::ComponentHandle;
use std::sync::Arc;

pub fn bind(window: &MainWindow, service: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    window
        .global::<Storage>()
        .set_directory(service.data_directory.to_string_lossy().as_ref().into());
    let (weak, api, runtime1) = (window.as_weak(), service.clone(), runtime.clone());
    window.global::<Storage>().on_open_directory(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let directory = api.data_directory.clone();
        bridge::run(
            &window,
            &api,
            &runtime1,
            Area::Storage,
            Epoch::default().ticket(),
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::system_actions::open_directory(&directory)
                })
                .await
                .map_err(|e| e.to_string())?
            },
            |_, ()| {},
        );
    });
    let (weak, api, runtime) = (window.as_weak(), service.clone(), runtime.clone());
    window.global::<Storage>().on_export_backup(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let service = api.clone();
        bridge::run(
            &window,
            &api,
            &runtime,
            Area::Storage,
            Epoch::default().ticket(),
            async move {
                let name = format!("GitHubSP-data-{}.sqlite3", githubsp_lib::model::now_ms());
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title("导出本地数据备份")
                    .set_file_name(name)
                    .add_filter("SQLite 数据备份", &["sqlite3"])
                    .save_file()
                    .await
                else {
                    return Ok(None);
                };
                if service.is_closing() {
                    return Ok(None);
                }
                let target = file.path().to_owned();
                service
                    .manager
                    .export_backup(target.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Some(target))
            },
            |window, target| {
                if let Some(path) = target {
                    window.global::<Storage>().set_message(
                        format!("备份已导出并通过完整性检查：{}", path.display()).into(),
                    );
                }
            },
        );
    });
}

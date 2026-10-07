use crate::{
    engine::Engine,
    manager::{Action, Manager},
    model::Snapshot,
};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
use tauri::{Emitter, Manager as TauriManager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn browse_releases(
    input: String,
    page: u32,
    include_prerelease: bool,
    catalog: State<'_, crate::catalog::Catalog>,
) -> std::result::Result<crate::catalog::CatalogPage, String> {
    catalog
        .browse(&input, page, include_prerelease)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn preview_batch(
    urls: Vec<String>,
    directory: String,
    manager: State<'_, Manager>,
    catalog: State<'_, crate::catalog::Catalog>,
) -> std::result::Result<crate::intake::BatchPreview, String> {
    crate::intake::preview(&manager, &catalog, urls, std::path::Path::new(&directory))
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn create_batch(
    urls: Vec<String>,
    directory: String,
    route: Option<String>,
    manager: State<'_, Manager>,
    catalog: State<'_, crate::catalog::Catalog>,
) -> std::result::Result<crate::intake::BatchResult, String> {
    crate::intake::create_batch(
        &manager,
        &catalog,
        urls,
        std::path::Path::new(&directory),
        route,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
async fn change_route(
    id: String,
    route: Option<String>,
    restart: bool,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager
        .route(id, route, restart)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn diagnose_routes(
    url: String,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager.diagnose(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(
    settings: crate::model::Settings,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager.settings(settings).await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn reorder_queue(
    ids: Vec<String>,
    revision: u64,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager
        .reorder(ids, revision)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn query_history(
    query: String,
    status: Option<crate::model::TaskStatus>,
    page: u32,
    manager: State<'_, Manager>,
) -> std::result::Result<crate::history::HistoryPage, String> {
    let snapshot = manager.snapshot().await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::history::query(snapshot, &query, status, page)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
#[tauri::command]
async fn acknowledge_notices(manager: State<'_, Manager>) -> std::result::Result<Snapshot, String> {
    manager.acknowledge().await.map_err(|e| e.to_string())
}
#[tauri::command]
fn hide_to_tray(app: tauri::AppHandle) -> std::result::Result<(), String> {
    if app.tray_by_id("main-tray").is_none() {
        return Err("托盘不可用，请保持主窗口打开".into());
    }
    app.get_webview_window("main")
        .ok_or("主窗口不可用")?
        .hide()
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn add_favorite(
    input: String,
    library: State<'_, crate::library::Library>,
) -> std::result::Result<Snapshot, String> {
    library.add(input).await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn remove_favorite(
    repository: String,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager
        .remove_favorite(repository)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn check_favorites(
    repository: Option<String>,
    library: State<'_, crate::library::Library>,
) -> std::result::Result<Snapshot, String> {
    library.check(repository).await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn check_app_update(
    catalog: State<'_, crate::catalog::Catalog>,
) -> std::result::Result<crate::updates::UpdateResult, String> {
    Ok(crate::updates::check(
        &catalog,
        crate::updates::configured_repository(),
        env!("CARGO_PKG_VERSION"),
    )
    .await)
}
#[tauri::command]
fn open_release(url: String) -> std::result::Result<(), String> {
    if !matches!(
        crate::source::parse_resource(&url),
        Ok(crate::source::Resource::Release(_, _))
    ) {
        return Err("只允许打开 GitHub 官方版本页面".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(url)
            .spawn()
            .map_err(|e| format!("无法打开官方发布页：{e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("打开发布页仅支持 Windows".into())
    }
}

#[tauri::command]
async fn list_tasks(manager: State<'_, Manager>) -> std::result::Result<Snapshot, String> {
    manager.snapshot().await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn inspect_directory(
    directory: String,
) -> std::result::Result<crate::preflight::DirectoryInspection, String> {
    crate::preflight::inspect_directory_async(PathBuf::from(directory.trim()))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn create_directory(directory: String) -> std::result::Result<PathBuf, String> {
    crate::preflight::create_directory_async(PathBuf::from(directory.trim()))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn create_task(
    url: String,
    directory: String,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    manager
        .create(url, PathBuf::from(directory))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn task_action(
    id: String,
    action: String,
    manager: State<'_, Manager>,
) -> std::result::Result<Snapshot, String> {
    let action = match action.as_str() {
        "pause" => Action::Pause,
        "resume" => Action::Resume,
        "cancel" => Action::Cancel,
        "remove" => Action::Remove,
        _ => return Err("无效任务操作".into()),
    };
    manager
        .action(id, action)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn open_directory(
    id: String,
    manager: State<'_, Manager>,
) -> std::result::Result<(), String> {
    let snapshot = manager
        .snapshot()
        .await
        .map_err(|error| error.to_string())?;
    let task = snapshot
        .tasks
        .iter()
        .find(|task| task.id == id)
        .ok_or("下载任务不存在")?;
    let directory =
        dunce::canonicalize(&task.directory).map_err(|_| "保存目录已不存在或不可访问")?;
    if !directory.is_dir() {
        return Err("保存位置不是目录".into());
    }
    // 只打开已登记任务的目录，不向 shell 传递任意命令或运行下载文件。
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(directory)
            .spawn()
            .map_err(|error| format!("无法打开文件夹：{error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = directory;
        Err("首版仅支持 Windows 打开目录".into())
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(crate::desktop::DesktopState::default())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window
                    .unminimize()
                    .and_then(|()| window.show())
                    .and_then(|()| window.set_focus())
                {
                    eprintln!("无法激活现有窗口：{error}");
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            crate::desktop::install(app.handle())?;
            let initialize = || -> std::result::Result<Manager, Box<dyn std::error::Error>> {
                let directory = app.path().app_local_data_dir()?;
                std::fs::create_dir_all(&directory)?;
                let handle = app.handle().clone();
                let emit = Arc::new(move |snapshot: Snapshot| {
                    crate::desktop::update(&handle, &snapshot);
                    if let Err(error) = handle.emit("downloads-changed", snapshot) {
                        eprintln!("界面事件发送失败：{error}");
                    }
                });
                let engine = Engine::production()?;
                app.manage(crate::catalog::Catalog::new(engine.network.clone()));
                let manager = tauri::async_runtime::block_on(async {
                    Manager::start(&directory.join("tasks.sqlite3"), engine, emit)
                })?;
                Ok(manager)
            };
            match initialize() {
                Ok(manager) => {
                    let snapshot = tauri::async_runtime::block_on(manager.snapshot())?;
                    crate::desktop::update(app.handle(), &snapshot);
                    let library = crate::library::Library::new(
                        app.state::<crate::catalog::Catalog>().inner().clone(),
                        manager.clone(),
                    );
                    tauri::async_runtime::block_on(async {
                        library.start();
                    });
                    app.manage(library);
                    app.manage(manager);
                }
                Err(error) => {
                    app.dialog()
                        .message(format!(
                            "无法启动 GitHubSP：{error}\n原有任务数据未被删除。"
                        ))
                        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                        .title("GitHubSP 启动失败")
                        .blocking_show();
                    return Err(error);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            inspect_directory,
            create_directory,
            create_task,
            task_action,
            open_directory,
            browse_releases,
            preview_batch,
            create_batch,
            change_route,
            diagnose_routes,
            save_settings,
            reorder_queue,
            query_history,
            acknowledge_notices,
            hide_to_tray,
            add_favorite,
            remove_favorite,
            check_favorites,
            check_app_update,
            open_release
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                if app
                    .state::<crate::desktop::DesktopState>()
                    .close_to_tray
                    .load(Ordering::SeqCst)
                    && app.tray_by_id("main-tray").is_some()
                {
                    if let Err(error) = window.hide() {
                        eprintln!("收起窗口失败：{error}");
                    }
                } else {
                    crate::desktop::exit(app);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("GitHubSP 桌面运行时启动失败");
}

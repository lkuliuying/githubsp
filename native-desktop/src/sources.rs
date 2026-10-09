use crate::{
    bridge::{self, Area, Epoch},
    presentation::{self, model},
    service::Service,
    AssetRow, MainWindow, PreviewRow, ReleaseRow, Sources, Workspace,
};
use githubsp_lib::{
    catalog::CatalogPage,
    intake::{self, BatchPreview},
};
use slint::ComponentHandle;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Default)]
pub struct Inputs {
    pub epoch: Epoch,
    pub selected_directory: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    repository_input: String,
    page: Option<CatalogPage>,
    selected: Vec<String>,
    release_id: Option<u64>,
    preview: Option<BatchPreview>,
}

fn render(window: &MainWindow, state: &State) {
    let ui = window.global::<Sources>();
    ui.set_loaded(state.page.is_some());
    if let Some(page) = &state.page {
        ui.set_page(page.page as i32);
        ui.set_has_more(page.has_more);
        ui.set_releases(model(
            page.releases
                .iter()
                .map(|r| ReleaseRow {
                    id: r.id.to_string().into(),
                    tag: r.tag.as_str().into(),
                    detail: format!(
                        "{} · {} 个附件",
                        if r.prerelease {
                            "预发布"
                        } else {
                            "正式版"
                        },
                        r.assets.len()
                    )
                    .into(),
                })
                .collect(),
        ));
        if let Some(release) = page
            .releases
            .iter()
            .find(|r| Some(r.id) == state.release_id)
        {
            ui.set_selected_release(release.id.to_string().into());
            ui.set_release_title(format!("发布版本：{}", release.tag).into());
            ui.set_release_description(
                format!("{} 个附件 · 请确认平台和架构", release.assets.len()).into(),
            );
            ui.set_assets(model(
                release
                    .assets
                    .iter()
                    .map(|a| AssetRow {
                        url: a.url.as_str().into(),
                        name: a.name.as_str().into(),
                        size: presentation::bytes(a.size).into(),
                        detail: a
                            .unavailable
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|| {
                                if a.hints.is_empty() {
                                    "其他附件".into()
                                } else {
                                    a.hints.join(" · ")
                                }
                            })
                            .into(),
                        unavailable: a.unavailable.is_some(),
                        selected: state.selected.contains(&a.url),
                    })
                    .collect(),
            ));
        } else {
            ui.set_selected_release("".into());
            ui.set_release_title("版本附件".into());
            ui.set_release_description("此页暂无符合条件的版本".into());
            ui.set_assets(model(vec![]));
        }
    } else {
        ui.set_releases(model(vec![]));
        ui.set_assets(model(vec![]));
        ui.set_release_title("版本附件".into());
        ui.set_release_description("选择版本后，附件会显示在这里".into());
    }
    ui.set_selected_count(state.selected.len() as i32);
    let selected_size = state.selected.iter().try_fold(0u64, |total, url| {
        let asset = state
            .page
            .as_ref()?
            .releases
            .iter()
            .flat_map(|release| &release.assets)
            .find(|asset| &asset.url == url)?;
        total.checked_add(asset.size)
    });
    ui.set_selected_size(
        selected_size
            .map(presentation::bytes)
            .unwrap_or_else(|| "大小未知".into())
            .into(),
    );
    ui.set_preview_visible(state.preview.is_some());
    if let Some(preview) = &state.preview {
        let valid = preview.items.iter().filter(|r| r.status == "valid").count();
        ui.set_can_submit(valid > 0);
        ui.set_preview_valid_count(valid as i32);
        ui.set_preview_duplicate_count(
            preview
                .items
                .iter()
                .filter(|r| r.status == "duplicate")
                .count() as i32,
        );
        ui.set_preview_invalid_count(
            preview
                .items
                .iter()
                .filter(|r| r.status == "invalid")
                .count() as i32,
        );
        ui.set_preview_directory(
            preview
                .preflight
                .directory
                .to_string_lossy()
                .as_ref()
                .into(),
        );
        ui.set_preview_known_size(presentation::bytes(preview.known_size).into());
        ui.set_preview_unknown_count(preview.unknown_count as i32);
        ui.set_preview_available_space(
            preview
                .preflight
                .available
                .map(presentation::bytes)
                .unwrap_or_else(|| "未知".into())
                .into(),
        );
        ui.set_preview_space_note(
            preview
                .preflight
                .warning
                .as_deref()
                .unwrap_or("已按分片与合并文件检查空间。")
                .into(),
        );
        ui.set_preview(model(
            preview
                .items
                .iter()
                .map(|r| PreviewRow {
                    name: r.filename.as_deref().unwrap_or(&r.input).into(),
                    size: r
                        .size
                        .map(presentation::bytes)
                        .unwrap_or_else(|| "未知".into())
                        .into(),
                    state: match r.status.as_str() {
                        "valid" => "有效",
                        "duplicate" => "重复",
                        _ => "无效",
                    }
                    .into(),
                    detail: r.message.as_deref().unwrap_or("").into(),
                    task_id: r.task_id.as_deref().unwrap_or("").into(),
                    tone: match r.status.as_str() {
                        "valid" => 1,
                        "duplicate" => 3,
                        _ => 2,
                    },
                })
                .collect(),
        ));
    }
}

fn normalized(input: &str) -> String {
    let trimmed = input.trim();
    if !trimmed.contains("://")
        && trimmed.split('/').count() == 2
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-".contains(c))
    {
        format!("https://github.com/{trimmed}")
    } else {
        trimmed.into()
    }
}

pub(crate) fn complete_submission(
    window: &MainWindow,
    result: &intake::BatchResult,
    invalid: Vec<String>,
    from_selection: bool,
) -> Vec<String> {
    let failed: Vec<_> = result
        .items
        .iter()
        .filter(|item| item.status != "created")
        .collect();
    let remaining: Vec<_> = failed
        .iter()
        .map(|item| item.input.clone())
        .chain(invalid)
        .collect();
    let created = result
        .items
        .iter()
        .filter(|item| item.status == "created")
        .count();
    let ui = window.global::<Sources>();
    // 附件选择和批量文本使用不同草稿，提交其中一种不能清空另一种。
    if !from_selection {
        ui.set_batch_input(remaining.join("\n").into());
    }
    let retry = if remaining.is_empty() {
        ""
    } else if from_selection {
        "未成功的附件已保留，请重新预览后重试。"
    } else {
        "未成功的链接已保留，可修改后重试。"
    };
    ui.set_message(
        format!(
            "已创建 {created} 个任务。{retry}{}",
            failed
                .iter()
                .filter_map(|item| item.message.as_deref())
                .collect::<Vec<_>>()
                .join("；")
        )
        .into(),
    );
    if remaining.is_empty() && created > 0 {
        let workspace = window.global::<Workspace>();
        workspace.set_queue_message(ui.get_message());
        ui.set_message("".into());
        workspace.invoke_select_download_section(0);
    }
    remaining
}

pub fn bind(
    window: &MainWindow,
    api: &Arc<Service>,
    runtime: &tokio::runtime::Handle,
    inputs: &Inputs,
) {
    let state = Arc::new(Mutex::new(State::default()));
    {
        let (weak, state, inputs) = (window.as_weak(), state.clone(), inputs.clone());
        window.global::<Sources>().on_edited(move || {
            inputs.epoch.invalidate();
            if let Some(window) = weak.upgrade() {
                match state.lock() {
                    Ok(mut state) => {
                        let repository = window.global::<Sources>().get_repository().to_string();
                        if state.repository_input != repository {
                            state.repository_input = repository;
                            state.page = None;
                            state.release_id = None;
                            state.selected.clear();
                        }
                        state.preview = None;
                        render(&window, &state);
                    }
                    Err(_) => window
                        .global::<Sources>()
                        .set_message("附件状态无法访问，请重新启动应用。".into()),
                }
            }
        });
    }
    {
        let (weak, state, api, runtime, inputs) = (
            window.as_weak(),
            state.clone(),
            api.clone(),
            runtime.clone(),
            inputs.clone(),
        );
        window.global::<Sources>().on_browse(move |page_number| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let ui = window.global::<Sources>();
            if ui.get_busy() || page_number < 0 {
                return;
            }
            inputs.epoch.invalidate();
            let source = if page_number > 0 {
                match state.lock() {
                    Ok(state) => state
                        .page
                        .as_ref()
                        .map(|p| format!("https://github.com/{}", p.repository))
                        .unwrap_or_else(|| normalized(&ui.get_repository())),
                    Err(_) => {
                        ui.set_message("附件状态无法访问，请重新启动应用。".into());
                        return;
                    }
                }
            } else {
                normalized(&ui.get_repository())
            };
            let (catalog, prerelease, state) =
                (api.catalog.clone(), ui.get_prerelease(), state.clone());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Sources,
                inputs.epoch.ticket(),
                async move {
                    catalog
                        .browse(&source, page_number as u32, prerelease)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |window, page| match state.lock() {
                    Ok(mut state) => {
                        state.repository_input =
                            window.global::<Sources>().get_repository().to_string();
                        state.release_id = page
                            .releases
                            .iter()
                            .find(|r| {
                                r.assets
                                    .iter()
                                    .any(|a| Some(&a.url) == page.selected_url.as_ref())
                            })
                            .or_else(|| page.releases.first())
                            .map(|r| r.id);
                        state.selected = page.selected_url.iter().cloned().collect();
                        state.preview = None;
                        state.page = Some(page);
                        render(window, &state);
                    }
                    Err(_) => window
                        .global::<Sources>()
                        .set_message("附件状态无法访问，请重新启动应用。".into()),
                },
            );
        });
    }
    {
        let (weak, state) = (window.as_weak(), state.clone());
        window.global::<Sources>().on_select_release(move |id| {
            if let Some(window) = weak.upgrade() {
                if window.global::<Sources>().get_busy() {
                    return;
                }
                match state.lock() {
                    Ok(mut state) => {
                        let id = id.parse::<u64>().ok();
                        if state
                            .page
                            .as_ref()
                            .is_some_and(|p| p.releases.iter().any(|r| Some(r.id) == id))
                        {
                            state.release_id = id;
                            state.preview = None;
                            render(&window, &state);
                        }
                    }
                    Err(_) => window.set_message("附件状态无法访问".into()),
                }
            }
        });
    }
    {
        let (weak, state) = (window.as_weak(), state.clone());
        window
            .global::<Sources>()
            .on_select_asset(move |url, selected| {
                if let Some(window) = weak.upgrade() {
                    if window.global::<Sources>().get_busy() {
                        return;
                    }
                    match state.lock() {
                        Ok(mut state) => {
                            let valid = state.page.as_ref().is_some_and(|p| {
                                p.releases
                                    .iter()
                                    .flat_map(|r| &r.assets)
                                    .any(|a| a.url == url.as_str() && a.unavailable.is_none())
                            });
                            if selected
                                && valid
                                && state.selected.len() < 100
                                && !state.selected.iter().any(|item| item == url.as_str())
                            {
                                state.selected.push(url.to_string());
                            } else if !selected {
                                state.selected.retain(|item| item != url.as_str());
                            }
                            state.preview = None;
                            render(&window, &state);
                        }
                        Err(_) => window.set_message("附件状态无法访问".into()),
                    }
                }
            });
    }
    {
        let (weak, state, api, runtime, inputs) = (
            window.as_weak(),
            state.clone(),
            api.clone(),
            runtime.clone(),
            inputs.clone(),
        );
        window
            .global::<Sources>()
            .on_preview_batch(move |from_selection| {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                if window.get_busy() {
                    return;
                }
                let urls: Vec<String> = if from_selection {
                    match state.lock() {
                        Ok(state) => state.selected.clone(),
                        Err(_) => {
                            window.set_message("附件状态无法访问".into());
                            return;
                        }
                    }
                } else {
                    window
                        .global::<Sources>()
                        .get_batch_input()
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect()
                };
                let directory = window.get_directory().trim().to_owned();
                let selected = inputs.selected_directory.load(Ordering::Acquire);
                let (ticket, api1, state) = (inputs.epoch.ticket(), api.clone(), state.clone());
                bridge::run(
                    &window,
                    &api,
                    &runtime,
                    Area::Sources,
                    ticket.clone(),
                    async move {
                        let Some(directory) =
                            bridge::prepare_directory(directory, selected, &ticket, &api1).await?
                        else {
                            return Ok(None);
                        };
                        let preview =
                            intake::preview(&api1.manager, &api1.catalog, urls, &directory)
                                .await
                                .map_err(|e| e.to_string())?;
                        Ok(Some(preview))
                    },
                    move |window, preview| {
                        if let Some(preview) = preview {
                            match state.lock() {
                                Ok(mut state) => {
                                    state.preview = Some(preview);
                                    render(window, &state);
                                }
                                Err(_) => window.set_message("附件状态无法访问".into()),
                            }
                        }
                    },
                );
            });
    }
    {
        let (weak, state, api, runtime, inputs) = (
            window.as_weak(),
            state.clone(),
            api.clone(),
            runtime.clone(),
            inputs.clone(),
        );
        window.global::<Sources>().on_submit(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if window.get_busy() {
                return;
            }
            let from_selection = window.global::<Workspace>().get_creation_mode() == 1;
            let (urls, expected, invalid) = match state.lock() {
                Ok(state) => {
                    let Some(preview) = &state.preview else {
                        return;
                    };
                    (
                        preview
                            .items
                            .iter()
                            .filter(|r| r.status == "valid")
                            .filter_map(|r| r.url.clone())
                            .collect::<Vec<_>>(),
                        preview.preflight.directory.clone(),
                        preview
                            .items
                            .iter()
                            .filter(|r| r.status == "invalid")
                            .map(|r| r.input.clone())
                            .collect::<Vec<_>>(),
                    )
                }
                Err(_) => {
                    window.set_message("附件状态无法访问".into());
                    return;
                }
            };
            let directory = window.get_directory().trim().to_owned();
            let selected = inputs.selected_directory.load(Ordering::Acquire);
            let route = presentation::ROUTES
                .get(window.global::<Sources>().get_route() as usize)
                .and_then(|r| {
                    if r.0.is_empty() {
                        None
                    } else {
                        Some(r.0.to_owned())
                    }
                });
            let (ticket, api1, state) = (inputs.epoch.ticket(), api.clone(), state.clone());
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Sources,
                ticket.clone(),
                async move {
                    let Some(directory) =
                        bridge::prepare_directory(directory, selected, &ticket, &api1).await?
                    else {
                        return Ok(None);
                    };
                    if directory != expected {
                        return Err("保存目录已变化，请重新预览后再创建任务".into());
                    }
                    ticket.ensure_current(&api1)?;
                    intake::create_batch(&api1.manager, &api1.catalog, urls, &directory, route)
                        .await
                        .map(Some)
                        .map_err(|e| e.to_string())
                },
                move |window, result| {
                    if let Some(result) = result {
                        let remaining =
                            complete_submission(window, &result, invalid, from_selection);
                        match state.lock() {
                            Ok(mut state) => {
                                state.preview = None;
                                if from_selection {
                                    state.selected.retain(|url| remaining.contains(url));
                                }
                                render(window, &state);
                            }
                            Err(_) => window.set_message("附件状态无法访问".into()),
                        }
                    }
                },
            );
        });
    }
    {
        let (weak, state) = (window.as_weak(), state.clone());
        window.global::<Sources>().on_dismiss_preview(move || {
            if let Some(window) = weak.upgrade() {
                match state.lock() {
                    Ok(mut state) => {
                        state.preview = None;
                        render(&window, &state);
                    }
                    Err(_) => window.set_message("附件状态无法访问".into()),
                }
            }
        });
    }
    let (weak, api, runtime) = (window.as_weak(), api.clone(), runtime.clone());
    window.global::<Sources>().on_favorite(move || {
        if let Some(window) = weak.upgrade() {
            let repo = match state.lock() {
                Ok(state) => state.page.as_ref().map(|p| p.repository.clone()),
                Err(_) => {
                    window.set_message("附件状态无法访问，请重新启动应用。".into());
                    return;
                }
            };
            let Some(repo) = repo else {
                return;
            };
            let library = api.library.clone();
            bridge::run(
                &window,
                &api,
                &runtime,
                Area::Commands,
                Epoch::default().ticket(),
                async move {
                    library
                        .add(format!("https://github.com/{repo}"))
                        .await
                        .map_err(|e| e.to_string())
                },
                |window, _| window.global::<Sources>().set_message("已添加收藏".into()),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_shorthand_only_normalizes_public_owner_and_repo() {
        assert_eq!(normalized("owner/repo"), "https://github.com/owner/repo");
        assert_eq!(
            normalized("https://example.com/a/b"),
            "https://example.com/a/b"
        );
        assert_eq!(normalized("owner/repo/extra"), "owner/repo/extra");
    }
}

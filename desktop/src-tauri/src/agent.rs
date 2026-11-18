//! 本机 Agent 接口由桌面进程持有；插件断开不改变已接受任务的生命周期。
use crate::service::Service;
use githubsp_lib::{
    catalog::Release,
    manager::Action,
    model::{now_ms, AgentSettings, CreateOptions, Task, TaskStatus, Verification, WatchRule},
    selection::AssetPreference,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::{Mutex, Semaphore},
};

const MAX_REQUEST: u64 = 1_048_576;
const MAX_RESPONSE: usize = 2 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    id: String,
    request: Request,
}

#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Request {
    Software {
        #[serde(default)]
        query: String,
        #[serde(default = "first_page")]
        page: u32,
    },
    Watchlist {
        action: String,
        manifest: Option<String>,
        directory: Option<PathBuf>,
        policy: Option<String>,
        fingerprint: Option<String>,
    },
    Updates {
        cursor: Option<String>,
        limit: u32,
        repository: Option<String>,
    },
    Update {
        id: u64,
        action: String,
    },
    ProjectLabels {
        repository: String,
        group: String,
        tags: Vec<String>,
    },
    Status,
    Resolve {
        input: String,
        preference: Option<AssetPreference>,
        pattern: Option<String>,
        #[serde(default)]
        include_prerelease: bool,
    },
    Create {
        resolution_id: String,
        asset_id: u64,
        directory: PathBuf,
        request_key: String,
    },
    Tasks {
        #[serde(default = "first_page")]
        page: u32,
        #[serde(default)]
        query: String,
    },
    Task {
        id: Option<String>,
        request_key: Option<String>,
    },
    Wait {
        id: String,
        after_revision: u64,
        timeout_ms: u64,
    },
    Control {
        id: String,
        action: String,
    },
    Watch {
        action: String,
        repository: Option<String>,
        rule: Option<WatchRule>,
    },
}
fn first_page() -> u32 {
    1
}
fn github_input(input: &str) -> String {
    let input = input.trim();
    if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://github.com/{input}")
    }
}
struct Resolution {
    release: Release,
    expires_at: u64,
}
struct Bridge {
    service: Arc<Service>,
    resolutions: Mutex<BTreeMap<String, Resolution>>,
}

pub fn pipe_name(directory: &Path) -> Result<String, String> {
    let directory = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    // 与 Node 客户端统一去掉 Windows 扩展路径前缀，并只折叠 ASCII 大小写。
    let text = directory
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_ascii_lowercase();
    Ok(format!(
        r"\\.\pipe\githubsp-{:x}",
        Sha256::digest(text.as_bytes())
    ))
}

fn pipe(name: &str, first: bool) -> std::io::Result<NamedPipeServer> {
    use std::{
        mem::{size_of, zeroed},
        ptr::null_mut,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            },
            GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    // 命名管道仅允许当前 Windows 用户，且拒绝远程连接；描述符只在创建时借用。
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut needed = 0;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut length = 0;
        while *sid.add(length) != 0 {
            length += 1;
        }
        let sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
        LocalFree(sid.cast());
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid_text})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let mut attributes: SECURITY_ATTRIBUTES = zeroed();
        attributes.nLength = size_of::<SECURITY_ATTRIBUTES>() as u32;
        attributes.lpSecurityDescriptor = descriptor;
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor.cast());
        result
    }
}

pub fn start(service: Arc<Service>) -> Result<(), String> {
    let name = pipe_name(&service.directory)?;
    let mut listener = pipe(&name, true).map_err(|e| format!("无法启动本机 Agent 接口：{e}"))?;
    let mut stop = service.agent_stop.subscribe();
    let bridge = Arc::new(Bridge {
        service,
        resolutions: Mutex::new(BTreeMap::new()),
    });
    tokio::spawn(async move {
        let slots = Arc::new(Semaphore::new(16));
        loop {
            tokio::select! { _ = stop.changed() => break, result = listener.connect() => if result.is_err() { break; } }
            let next = match pipe(&name, false) {
                Ok(next) => next,
                Err(error) => {
                    eprintln!("Agent 接口停止监听：{error}");
                    break;
                }
            };
            let client = std::mem::replace(&mut listener, next);
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                continue;
            };
            let bridge = bridge.clone();
            let mut stop = bridge.service.agent_stop.subscribe();
            tokio::spawn(async move {
                let _permit = permit;
                tokio::select! { _ = stop.changed() => (), _ = connection(client, bridge) => () }
            });
        }
    });
    Ok(())
}

async fn connection(client: NamedPipeServer, bridge: Arc<Bridge>) {
    let (read, mut write) = tokio::io::split(client);
    let mut input = String::new();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        BufReader::new(read.take(MAX_REQUEST + 1)).read_line(&mut input),
    )
    .await;
    if !matches!(result, Ok(Ok(_))) || input.len() as u64 > MAX_REQUEST || !input.ends_with('\n') {
        return;
    }
    let response = match serde_json::from_str::<Envelope>(&input) {
        Ok(request)
            if request.version == 1 && !request.id.is_empty() && request.id.len() <= 128 =>
        {
            let result =
                tokio::time::timeout(Duration::from_secs(90), bridge.dispatch(request.request))
                    .await;
            match result {
                Ok(Ok(value)) => json!({"version":1,"id":request.id,"ok":true,"result":value}),
                Ok(Err(error)) => json!({"version":1,"id":request.id,"ok":false,"error":error}),
                Err(_) => {
                    json!({"version":1,"id":request.id,"ok":false,"error":"操作超时；创建请求可能已接受，请使用原请求键查询，勿盲目重新建任务"})
                }
            }
        }
        _ => json!({"version":1,"ok":false,"error":"无效的 Agent 请求或协议版本"}),
    };
    let Ok(mut bytes) = serde_json::to_vec(&response) else {
        return;
    };
    if bytes.len() > MAX_RESPONSE {
        return;
    }
    bytes.push(b'\n');
    let _ = tokio::time::timeout(Duration::from_secs(5), write.write_all(&bytes)).await;
}

fn allowed(directory: &Path, settings: &AgentSettings) -> Result<PathBuf, String> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("请选择已存在的绝对下载目录".into());
    }
    let directory = githubsp_lib::preflight::inspect_directory(directory)
        .map_err(|e| e.to_string())?
        .directory;
    if settings.allowed_directories.iter().any(|root| {
        githubsp_lib::preflight::inspect_directory(root)
            .is_ok_and(|root| directory.starts_with(root.directory))
    }) {
        Ok(directory)
    } else {
        Err("下载目录超出 Agent 允许范围；请在 GitHubSP 设置中确认允许目录".into())
    }
}

fn task_result(task: &Task) -> Value {
    let verification = match task.verification {
        Verification::Verified => "official_match",
        Verification::Failed => "failed",
        Verification::Unverified if task.details.sha256.is_some() => "local_hash_only",
        Verification::Unverified => "no_available_digest",
        Verification::Pending => "pending",
    };
    json!({"id":task.id,"revision":task.revision,"requestKey":task.details.agent_request_key,"repository":task.details.repository,"tag":task.details.tag,"assetId":task.details.asset_id,"filename":task.filename,"directory":task.directory,"path":task.final_path,"status":task.status,"downloaded":task.downloaded,"total":task.total,"sha256":task.details.sha256,"officialSha256":task.details.official_sha256,"verification":verification,"failure":task.details.failure,"error":task.error})
}
fn public_watch_entry(
    entry: &githubsp_lib::model::WatchEntry,
    global_auto: bool,
) -> Result<Value, String> {
    let mut value = serde_json::to_value(entry).map_err(|e| e.to_string())?;
    value["rule"] = json!({"autoDownload":entry.rule.automatic(global_auto),"includePrerelease":entry.rule.include_prerelease,"assetPattern":entry.rule.asset_pattern,"preference":entry.rule.preference,"downloadDirectory":entry.rule.download_directory});
    Ok(value)
}

impl Bridge {
    async fn dispatch(&self, request: Request) -> Result<Value, String> {
        let service = &self.service;
        let _guard = service.gate.read().await;
        if service.closing.load(Ordering::Acquire) {
            return Err("GitHubSP 正在退出，请稍后重新查询".into());
        }
        let snapshot = service
            .manager
            .snapshot()
            .await
            .map_err(|e| e.to_string())?;
        let settings = &snapshot.settings.agent;
        if !settings.enabled {
            return Err("GitHubSP 的 Agent 接入已关闭，请在设置中启用并重启".into());
        }
        let result = match request {
            Request::Software { query, page } => serde_json::to_value(
                service
                    .manager
                    .software(query, page)
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
            Request::Watchlist {
                action,
                manifest,
                directory,
                policy,
                fingerprint,
            } => match action.as_str() {
                "export" => serde_json::to_value(
                    service
                        .manager
                        .watchlist_export()
                        .await
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
                "preview" | "import" => {
                    let directory =
                        allowed(&directory.ok_or("导入必须重新选择本机允许目录")?, settings)?;
                    let input = manifest.ok_or("缺少 JSON 关注清单")?;
                    let policy = policy.ok_or("缺少 keep/replace 导入策略")?;
                    if action == "preview" {
                        serde_json::to_value(
                            service
                                .manager
                                .watchlist_preview(input, directory, policy)
                                .await
                                .map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?
                    } else {
                        service
                            .manager
                            .import_watchlist(
                                input,
                                directory,
                                policy,
                                fingerprint.ok_or("请先预览并确认导入，再提供 fingerprint")?,
                            )
                            .await
                            .map_err(|e| e.to_string())?;
                        json!({"imported":true,"manifest":service.manager.watchlist_export().await.map_err(|e| e.to_string())?})
                    }
                }
                _ => return Err("关注清单操作只支持 export/preview/import".into()),
            },
            Request::Updates {
                cursor,
                limit,
                repository,
            } => serde_json::to_value(
                service
                    .manager
                    .update_page(cursor, limit, repository, false)
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
            Request::Update { id, action } => {
                if action != "detail" {
                    service
                        .manager
                        .update_action(id, action)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                serde_json::to_value(
                    service
                        .manager
                        .update_event(id)
                        .await
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?
            }
            Request::ProjectLabels {
                repository,
                group,
                tags,
            } => {
                service
                    .manager
                    .project_labels(repository.clone(), group, tags)
                    .await
                    .map_err(|e| e.to_string())?;
                let state = service
                    .manager
                    .snapshot()
                    .await
                    .map_err(|e| e.to_string())?;
                json!({"repository":repository,"entry":state.watch_state.entry(&repository)})
            }
            Request::Status => {
                json!({"version":env!("CARGO_PKG_VERSION"),"protocolVersion":1,"queueRevision":snapshot.queue_revision,"allowedDirectories":settings.allowed_directories,"runtime":"desktop","running":true})
            }
            Request::Resolve {
                input,
                preference,
                pattern,
                include_prerelease,
            } => {
                let page = service
                    .catalog
                    .browse(&github_input(&input), 1, include_prerelease)
                    .await
                    .map_err(|e| e.to_string())?;
                let release = page
                    .releases
                    .into_iter()
                    .next()
                    .ok_or("未找到可用 Release")?;
                if release.assets.len() > 200 {
                    return Err("此版本超过 200 个附件，请在桌面端选择目标".into());
                }
                let saved = snapshot
                    .watch_state
                    .entry(&page.repository)
                    .map(|entry| &entry.rule);
                let preference =
                    preference.or_else(|| saved.and_then(|rule| rule.preference.clone()));
                let pattern = pattern.or_else(|| saved.and_then(|rule| rule.asset_pattern.clone()));
                let recommendation = githubsp_lib::selection::recommend(
                    &release,
                    preference.as_ref(),
                    pattern.as_deref(),
                )
                .map_err(|e| e.to_string())?;
                let id = format!("{}-{}", now_ms(), SEQUENCE.fetch_add(1, Ordering::Relaxed));
                let expires_at = now_ms() + 600_000;
                let mut visible = release.clone();
                visible.notes = visible.notes.chars().take(16_000).collect();
                let response = json!({"resolutionId":id,"expiresAt":expires_at,"repository":page.repository,"release":visible,"notesTruncated":visible.notes.len() < release.notes.len(),"recommendation":recommendation});
                let mut cache = self.resolutions.lock().await;
                cache.retain(|_, item| item.expires_at > now_ms());
                if cache.len() >= 128 {
                    return Err("解析缓存已满，请稍后重试".into());
                }
                cache.insert(
                    id,
                    Resolution {
                        release,
                        expires_at,
                    },
                );
                response
            }
            Request::Create {
                resolution_id,
                asset_id,
                directory,
                request_key,
            } => {
                let directory = allowed(&directory, settings)?;
                let release = self
                    .resolutions
                    .lock()
                    .await
                    .get(&resolution_id)
                    .filter(|item| item.expires_at > now_ms())
                    .map(|item| item.release.clone())
                    .ok_or("解析结果已过期或服务已重启；先用原请求键查询任务，未创建时重新解析")?;
                let asset = release
                    .assets
                    .iter()
                    .find(|a| a.id == asset_id && a.unavailable.is_none())
                    .ok_or("附件不属于已解析的固定版本或不可用")?;
                service
                    .manager
                    .create_with(
                        asset.url.clone(),
                        directory,
                        CreateOptions {
                            request_key: Some(request_key.clone()),
                            asset_id: Some(asset.id),
                            size: Some(asset.size),
                            official_sha256: asset.sha256.clone(),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                task_result(
                    &service
                        .manager
                        .agent_task(request_key)
                        .await
                        .map_err(|e| e.to_string())?,
                )
            }
            Request::Tasks { page, query } => {
                let history = service
                    .manager
                    .history(query, None, page)
                    .await
                    .map_err(|e| e.to_string())?;
                json!({"items":history.items.iter().map(|item| task_result(&item.task)).collect::<Vec<_>>(),"page":history.page,"pageSize":history.page_size,"total":history.total,"revision":history.revision})
            }
            Request::Task { id, request_key } => {
                let task = match (id, request_key) {
                    (Some(id), None) => service.manager.task(id).await,
                    (None, Some(key)) => service.manager.agent_task(key).await,
                    _ => return Err("请仅提供任务 ID 或请求键之一".into()),
                }
                .map_err(|e| e.to_string())?;
                task_result(&task)
            }
            Request::Wait {
                id,
                after_revision,
                timeout_ms,
            } => {
                if timeout_ms > 30_000 {
                    return Err("等待时长上限为 30000 毫秒".into());
                }
                let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
                let mut changed = service.state.clone();
                loop {
                    let task = service
                        .manager
                        .task(id.clone())
                        .await
                        .map_err(|e| e.to_string())?;
                    if task.revision > after_revision
                        || matches!(
                            task.status,
                            TaskStatus::Completed
                                | TaskStatus::Failed
                                | TaskStatus::Cancelled
                                | TaskStatus::Paused
                        )
                        || tokio::time::Instant::now() >= deadline
                    {
                        break task_result(&task);
                    }
                    if tokio::time::timeout_at(deadline, changed.changed())
                        .await
                        .is_err()
                    {
                        break task_result(&task);
                    }
                }
            }
            Request::Control { id, action } => {
                let task = service
                    .manager
                    .task(id.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                allowed(&task.directory, settings)?;
                let action = match action.as_str() {
                    "pause" => Action::Pause,
                    "resume" => Action::Resume,
                    "cancel" => Action::Cancel,
                    _ => return Err("只支持 pause、resume 或 cancel".into()),
                };
                service
                    .manager
                    .action(id.clone(), action)
                    .await
                    .map_err(|e| e.to_string())?;
                task_result(&service.manager.task(id).await.map_err(|e| e.to_string())?)
            }
            Request::Watch {
                action,
                repository,
                rule,
            } => {
                let repository = repository
                    .map(|input| {
                        githubsp_lib::source::parse_resource(&github_input(&input))
                            .and_then(|resource| resource.repository())
                            .map_err(|e| e.to_string())
                    })
                    .transpose()?;
                if ["add", "rule"].contains(&action.as_str()) && repository.is_none() {
                    return Err("缺少仓库".into());
                }
                if action == "rule" && rule.is_none() {
                    return Err("缺少关注规则".into());
                }
                if action == "rule" {
                    if let Some(directory) = rule
                        .as_ref()
                        .and_then(|rule| rule.download_directory.as_deref())
                    {
                        allowed(directory, settings)?;
                    }
                }
                if ["add", "rule", "check"].contains(&action.as_str()) {
                    let targets: Vec<&str> = repository
                        .as_deref()
                        .map(|repository| vec![repository])
                        .unwrap_or_else(|| {
                            snapshot
                                .favorites
                                .iter()
                                .map(|favorite| favorite.repository.as_str())
                                .collect()
                        });
                    for repository in targets {
                        let current = snapshot
                            .watch_state
                            .entry(repository)
                            .map(|entry| entry.rule.clone())
                            .unwrap_or_default();
                        let target = if action == "rule" {
                            rule.as_ref().unwrap_or(&current)
                        } else {
                            &current
                        };
                        let automatic = if action == "rule" {
                            target.auto_download
                        } else {
                            target.automatic(snapshot.settings.watch_auto_download)
                        };
                        if automatic {
                            allowed(
                                target
                                    .download_directory
                                    .as_deref()
                                    .or_else(|| snapshot.last_directory.as_deref().map(Path::new))
                                    .ok_or("自动下载尚未选择保存目录")?,
                                settings,
                            )?;
                        }
                    }
                }
                match action.as_str() {
                    "list" => (),
                    "add" => {
                        service
                            .library
                            .add(github_input(&repository.ok_or("缺少仓库")?))
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    "rule" => {
                        service
                            .manager
                            .watch_rule(repository.ok_or("缺少仓库")?, rule.ok_or("缺少关注规则")?)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    "check" => {
                        service
                            .library
                            .check(repository)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    "dismiss" => {
                        service
                            .library
                            .dismiss(repository)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    _ => return Err("关注操作只支持 list/add/rule/check/dismiss".into()),
                }
                let snapshot = service
                    .manager
                    .snapshot()
                    .await
                    .map_err(|e| e.to_string())?;
                let entries = snapshot
                    .watch_state
                    .repositories
                    .iter()
                    .map(|(repository, entry)| {
                        Ok((
                            repository.clone(),
                            public_watch_entry(entry, snapshot.settings.watch_auto_download)?,
                        ))
                    })
                    .collect::<Result<serde_json::Map<String, Value>, String>>()?;
                json!({"favorites":snapshot.favorites,"watchState":{"schema":snapshot.watch_state.schema,"repositories":entries},"checking":snapshot.checking,"apiStatus":snapshot.api_status})
            }
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allowed_roots_use_components_and_reject_siblings() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("downloads");
        let sibling = temp.path().join("downloads-other");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        let settings = AgentSettings {
            enabled: true,
            allowed_directories: vec![root.clone()],
        };
        assert!(allowed(&root, &settings).is_ok());
        assert!(allowed(&sibling, &settings).is_err());
        assert!(allowed(Path::new("downloads"), &settings).is_err());
    }
    #[tokio::test]
    async fn private_named_pipe_can_bind_and_reject_second_owner() {
        let temp = tempfile::tempdir().unwrap();
        let name = pipe_name(temp.path()).unwrap();
        let _first = pipe(&name, true).unwrap();
        assert!(pipe(&name, true).is_err());
    }
}

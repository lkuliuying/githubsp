use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::now_ms,
    network::Network,
    source::{parse_release_url, parse_resource, Resource},
};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub url: String,
    pub sha256: Option<String>,
    pub hints: Vec<String>,
    pub unavailable: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: u64,
    pub tag: String,
    pub name: String,
    pub prerelease: bool,
    pub url: String,
    pub notes: String,
    pub assets: Vec<Asset>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub repository: String,
    pub releases: Vec<Release>,
    pub page: u32,
    pub has_more: bool,
    pub selected_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiAsset {
    id: u64,
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct ApiRelease {
    id: u64,
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    html_url: String,
    body: Option<String>,
    assets: Vec<ApiAsset>,
}
struct Cached {
    body: serde_json::Value,
    etag: Option<String>,
    checked: u64,
}
#[derive(Default)]
struct Cache {
    entries: HashMap<String, Cached>,
    next_allowed: u64,
}

#[derive(Clone)]
pub struct Catalog {
    network: Network,
    cache: Arc<Mutex<Cache>>,
    token: CancellationToken,
}

impl Catalog {
    pub fn new(network: Network) -> Self {
        Self {
            network,
            cache: Arc::new(Mutex::new(Cache::default())),
            token: CancellationToken::new(),
        }
    }
    pub fn stop(&self) {
        self.token.cancel();
    }
    #[cfg(test)]
    pub(crate) async fn expire_cache(&self) {
        for value in self.cache.lock().await.entries.values_mut() {
            value.checked = 0;
        }
    }
    pub async fn next_allowed(&self) -> u64 {
        self.cache.lock().await.next_allowed
    }

    async fn get(&self, path: &[&str], query: &[(&str, String)]) -> Result<serde_json::Value> {
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let mut url = Url::parse(&self.network.api_base)
            .map_err(|_| DownloadError::new(ErrorKind::InvalidInput, "官方接口配置错误"))?;
        url.path_segments_mut()
            .map_err(|_| DownloadError::new(ErrorKind::InvalidInput, "官方接口地址无效"))?
            .extend(path);
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v)));
        }
        let key = url.to_string();
        // 持锁执行请求，使目录、收藏和自身更新共享串行调度与限流窗口。
        let mut cache = tokio::select! { _ = self.token.cancelled() => return Err(DownloadError::cancelled()), cache = self.cache.lock() => cache };
        if self.token.is_cancelled() {
            return Err(DownloadError::cancelled());
        }
        let now = now_ms();
        if let Some(cached) = cache.entries.get(&key) {
            if now.saturating_sub(cached.checked) < 60_000 {
                return Ok(cached.body.clone());
            }
        }
        if cache.next_allowed > now {
            return Err(rate_error(cache.next_allowed - now));
        }
        let mut request = self
            .network
            .client
            .get(url.clone())
            .header(header::ACCEPT, "application/vnd.github+json");
        if let Some(etag) = cache.entries.get(&key).and_then(|v| v.etag.as_ref()) {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
        let operation = async {
            let mut response = request.send().await?;
            if response.url().origin() != url.origin() {
                return Err(DownloadError::new(
                    ErrorKind::Protocol,
                    "官方接口发生非官方重定向",
                ));
            }
            let status = response.status();
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .map(retry_delay_ms);
            let exhausted = response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|v| v == "0");
            if matches!(
                status,
                StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN
            ) || retry_after.is_some()
                || exhausted
            {
                let reset = response
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|v| v.saturating_mul(1000).saturating_sub(now_ms()));
                let delay = retry_after.or(reset).unwrap_or(60_000).max(1_000);
                cache.next_allowed = now_ms().saturating_add(delay);
                if !status.is_success() && status != StatusCode::NOT_MODIFIED {
                    return Err(rate_error(delay));
                }
            }
            if status == StatusCode::NOT_MODIFIED {
                if let Some(cached) = cache.entries.get_mut(&key) {
                    cached.checked = now_ms();
                    return Ok(cached.body.clone());
                }
            }
            if status == StatusCode::NOT_FOUND {
                return Err(DownloadError::new(
                    ErrorKind::NotFound,
                    "未找到公开仓库、正式版本或附件，请核对地址",
                ));
            }
            if !status.is_success() {
                return Err(DownloadError::new(
                    ErrorKind::Network,
                    format!(
                        "GitHub 官方接口返回 HTTP {}，标准附件直链仍可尝试下载",
                        status.as_u16()
                    ),
                ));
            }
            let etag = response
                .headers()
                .get(header::ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                    return Err(DownloadError::new(ErrorKind::Protocol, "官方接口响应过大"));
                }
                bytes.extend_from_slice(&chunk);
            }
            let body: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|_| DownloadError::new(ErrorKind::Protocol, "官方接口返回了无效数据"))?;
            if cache.entries.len() >= 128 {
                cache
                    .entries
                    .retain(|_, v| now_ms().saturating_sub(v.checked) < 60_000);
                if cache.entries.len() >= 128 {
                    cache.entries.clear();
                }
            }
            cache.entries.insert(
                key,
                Cached {
                    body: body.clone(),
                    etag,
                    checked: now_ms(),
                },
            );
            Ok(body)
        };
        tokio::select! { _ = self.token.cancelled() => Err(DownloadError::cancelled()), result = tokio::time::timeout(Duration::from_secs(15), operation) => result.map_err(|_| DownloadError::new(ErrorKind::Network, "GitHub 官方接口请求超时"))? }
    }

    pub async fn browse(
        &self,
        input: &str,
        page: u32,
        include_prerelease: bool,
    ) -> Result<CatalogPage> {
        if page > 1000 {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "版本页码超出范围",
            ));
        }
        let resource = parse_resource(input)?;
        let repository = resource.repository()?;
        let (owner, repo) = repository
            .split_once('/')
            .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "仓库地址无效"))?;
        let (value, list) = match &resource {
            Resource::Asset(url) => {
                let source = parse_release_url(url)?;
                (
                    self.get(
                        &["repos", owner, repo, "releases", "tags", &source.tag],
                        &[],
                    )
                    .await?,
                    false,
                )
            }
            Resource::Release(_, tag) => (
                self.get(&["repos", owner, repo, "releases", "tags", tag], &[])
                    .await?,
                false,
            ),
            Resource::Repository(_) if page > 0 => (
                self.get(
                    &["repos", owner, repo, "releases"],
                    &[("per_page", "10".into()), ("page", page.to_string())],
                )
                .await?,
                true,
            ),
            _ => (
                self.get(&["repos", owner, repo, "releases", "latest"], &[])
                    .await?,
                false,
            ),
        };
        let parse_error = |_| DownloadError::new(ErrorKind::Protocol, "官方版本数据不完整");
        let raw: Vec<ApiRelease> = if list {
            serde_json::from_value(value).map_err(parse_error)?
        } else {
            vec![serde_json::from_value(value).map_err(parse_error)?]
        };
        let has_more = list && raw.len() == 10;
        let mut releases = Vec::new();
        for mut release in raw {
            if release.draft || (list && release.prerelease && !include_prerelease) {
                continue;
            }
            if release.assets.len() >= 100 {
                let id = release.id.to_string();
                let mut all = Vec::new();
                for asset_page in 1..=100 {
                    let values = self
                        .get(
                            &["repos", owner, repo, "releases", &id, "assets"],
                            &[("per_page", "100".into()), ("page", asset_page.to_string())],
                        )
                        .await?;
                    let values: Vec<ApiAsset> =
                        serde_json::from_value(values).map_err(parse_error)?;
                    let last = values.len() < 100;
                    all.extend(values);
                    if last {
                        break;
                    }
                    if asset_page == 100 {
                        return Err(DownloadError::new(
                            ErrorKind::Protocol,
                            "附件数量过多，请使用附件直链",
                        ));
                    }
                }
                release.assets = all;
            }
            let mut assets = Vec::new();
            for asset in release.assets {
                let parsed = crate::source::parse_asset_location(&asset.browser_download_url)?;
                if !format!("{}/{}", parsed.owner, parsed.repo).eq_ignore_ascii_case(&repository)
                    || parsed.tag != release.tag_name
                    || parsed.filename != asset.name
                {
                    return Err(DownloadError::new(
                        ErrorKind::Protocol,
                        "官方附件与来源仓库或版本不一致",
                    ));
                }
                assets.push(Asset {
                    unavailable: crate::source::validate_filename(&asset.name)
                        .err()
                        .map(|error| error.message),
                    id: asset.id,
                    hints: hints(&asset.name),
                    name: asset.name,
                    size: asset.size,
                    url: parsed.url.to_string(),
                    sha256: digest(asset.digest),
                });
            }
            if !matches!(parse_resource(&release.html_url), Ok(Resource::Release(ref actual, ref tag)) if actual.eq_ignore_ascii_case(&repository) && *tag == release.tag_name)
            {
                return Err(DownloadError::new(
                    ErrorKind::Protocol,
                    "官方版本页面地址不一致",
                ));
            }
            releases.push(Release {
                id: release.id,
                tag: release.tag_name,
                name: release.name.unwrap_or_default(),
                prerelease: release.prerelease,
                url: release.html_url,
                notes: release.body.unwrap_or_default(),
                assets,
            });
        }
        let selected_url = if let Resource::Latest(_, Some(name)) = resource {
            Some(
                releases
                    .iter()
                    .flat_map(|r| &r.assets)
                    .find(|asset| asset.name == name)
                    .ok_or_else(|| {
                        DownloadError::new(
                            ErrorKind::NotFound,
                            "最新正式版中没有此附件，请重新选择",
                        )
                    })?
                    .url
                    .clone(),
            )
        } else {
            None
        };
        Ok(CatalogPage {
            repository,
            releases,
            page,
            has_more,
            selected_url,
        })
    }

    pub async fn resolve(&self, input: &str) -> Result<(String, Option<Asset>)> {
        match parse_resource(input)? {
            Resource::Asset(url) => {
                // 标准直链不依赖目录 API 成功，下载核心仍会重新获取官方摘要。
                let asset = self.browse(&url, 0, true).await.ok().and_then(|page| {
                    page.releases
                        .into_iter()
                        .flat_map(|r| r.assets)
                        .find(|a| a.url == url)
                });
                Ok((url, asset))
            }
            Resource::Latest(_, Some(_)) => {
                let page = self.browse(input, 0, false).await?;
                let url = page
                    .selected_url
                    .ok_or_else(|| DownloadError::new(ErrorKind::NotFound, "未找到附件"))?;
                let asset = page
                    .releases
                    .into_iter()
                    .flat_map(|r| r.assets)
                    .find(|a| a.url == url);
                Ok((url, asset))
            }
            _ => Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "请先从版本列表选择附件；批量输入只接受附件直链",
            )),
        }
    }
}

fn digest(value: Option<String>) -> Option<String> {
    value
        .and_then(|v| v.strip_prefix("sha256:").map(str::to_ascii_lowercase))
        .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn hints(name: &str) -> Vec<String> {
    let name = name.to_ascii_lowercase();
    let mut result = Vec::new();
    for (matched, label) in [
        (
            name.contains("win") || name.ends_with(".exe") || name.ends_with(".msi"),
            "Windows",
        ),
        (
            name.contains("x64") || name.contains("amd64") || name.contains("x86_64"),
            "x64",
        ),
        (name.ends_with(".exe") || name.ends_with(".msi"), "安装包"),
        (
            [".zip", ".7z", ".tar.gz", ".rar"]
                .iter()
                .any(|s| name.ends_with(s)),
            "压缩包",
        ),
    ] {
        if matched {
            result.push(label.into());
        }
    }
    result
}
fn rate_error(delay: u64) -> DownloadError {
    let mut error = DownloadError::new(
        ErrorKind::Network,
        format!("GitHub 接口限流，请在约 {} 秒后重试", delay.div_ceil(1000)),
    );
    error.retry_after = Some(Duration::from_millis(delay));
    error
}
fn retry_delay_ms(value: &str) -> u64 {
    value
        .parse::<u64>()
        .map(|v| v.saturating_mul(1000))
        .unwrap_or_else(|_| {
            httpdate::parse_http_date(value)
                .ok()
                .and_then(|v| v.duration_since(std::time::SystemTime::now()).ok())
                .map(|v| v.as_millis() as u64)
                .unwrap_or(60_000)
        })
}

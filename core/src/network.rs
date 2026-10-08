use crate::{
    error::{DownloadError, ErrorKind, Result},
    source::ReleaseSource,
};
use reqwest::{header, Client, Response, StatusCode};
use serde::Deserialize;
use std::time::{Duration, Instant, SystemTime};
use tokio_util::sync::CancellationToken;
use url::Url;

pub const PROBE_BYTES: u64 = 512 * 1024;

pub fn report(route: &Route, result: &Result<Probe>) -> crate::model::RouteReport {
    crate::model::RouteReport {
        id: route.id.clone(),
        name: route.name.clone(),
        checked_at: crate::model::now_ms(),
        bytes_per_second: result.as_ref().map(|p| p.throughput()).unwrap_or(0.0),
        available: result.is_ok(),
        error: result.as_ref().err().map(|e| e.message.clone()),
    }
}

pub(crate) fn redirect_allowed(url: &Url, previous: usize) -> bool {
    const HOSTS: &[&str] = &[
        "github.com",
        "api.github.com",
        "release-assets.githubusercontent.com",
        "objects.githubusercontent.com",
        "github-releases.githubusercontent.com",
        "gh-proxy.org",
        "gh-proxy.com",
        "ghproxy.net",
    ];
    previous < 5
        && url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|host| HOSTS.contains(&host))
}

pub(crate) fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if redirect_allowed(attempt.url(), attempt.previous().len()) {
            attempt.follow()
        } else {
            attempt.error("重定向目标不在允许列表或重定向次数超限")
        }
    })
}

#[derive(Debug, Clone)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub prefix: Option<String>,
}

impl Route {
    pub fn url(&self, source: &ReleaseSource) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}{}", source.url),
            None => source.url.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Network {
    pub client: Client,
    pub routes: Vec<Route>,
    pub api_base: String,
    pub probe_timeout: Duration,
    pub idle_timeout: Duration,
    pub retry_delay: Duration,
}

impl Network {
    pub fn production() -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("GitHubSP/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .redirect(redirect_policy())
            .https_only(true)
            .no_proxy()
            .build()?;
        Ok(Self {
            client,
            routes: vec![
                Route {
                    id: "github".into(),
                    name: "GitHub 直连".into(),
                    prefix: None,
                },
                Route {
                    id: "gh-proxy".into(),
                    name: "GH-Proxy".into(),
                    prefix: Some("https://gh-proxy.org/".into()),
                },
                Route {
                    id: "ghproxy-net".into(),
                    name: "ghproxy.net".into(),
                    prefix: Some("https://ghproxy.net/".into()),
                },
            ],
            api_base: "https://api.github.com".into(),
            probe_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            retry_delay: Duration::from_secs(1),
        })
    }

    pub async fn metadata(
        &self,
        source: &ReleaseSource,
        token: &CancellationToken,
    ) -> Option<Metadata> {
        let lookup = async {
            let mut url = Url::parse(&self.api_base).ok()?;
            url.path_segments_mut().ok()?.extend([
                "repos",
                &source.owner,
                &source.repo,
                "releases",
                "tags",
                &source.tag,
            ]);
            let mut response = self
                .client
                .get(url)
                .header(header::ACCEPT, "application/vnd.github+json")
                .send()
                .await
                .ok()?;
            if !response.status().is_success()
                || response.url().host_str() != Url::parse(&self.api_base).ok()?.host_str()
            {
                return None;
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.ok()? {
                if body.len() + chunk.len() > 4 * 1024 * 1024 {
                    return None;
                }
                body.extend_from_slice(&chunk);
            }
            let release: ReleaseMetadata = serde_json::from_slice(&body).ok()?;
            let asset = release.assets.into_iter().find(|asset| {
                crate::source::parse_release_url(&asset.browser_download_url)
                    .is_ok_and(|parsed| parsed.url == source.url)
            })?;
            let sha256 = asset
                .digest
                .and_then(|digest| digest.strip_prefix("sha256:").map(str::to_owned))
                .filter(|digest| {
                    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map(|digest| digest.to_ascii_lowercase());
            Some(Metadata {
                asset_id: asset.id,
                size: asset.size,
                sha256,
            })
        };
        tokio::select! {
            _ = token.cancelled() => None,
            result = tokio::time::timeout(self.probe_timeout, lookup) => result.ok().flatten(),
        }
    }

    pub async fn probe(
        &self,
        route: Route,
        source: &ReleaseSource,
        metadata: Option<&Metadata>,
        token: &CancellationToken,
    ) -> Result<Probe> {
        let started = Instant::now();
        let work = async {
            let mut response = self
                .client
                .get(route.url(source))
                .header(header::ACCEPT_ENCODING, "identity")
                .header(header::RANGE, format!("bytes=0-{}", PROBE_BYTES - 1))
                .send()
                .await?;
            if response.status() == StatusCode::RANGE_NOT_SATISFIABLE
                && metadata.is_some_and(|value| value.size == 0)
            {
                response = self
                    .client
                    .get(route.url(source))
                    .header(header::ACCEPT_ENCODING, "identity")
                    .send()
                    .await?;
            }
            check_status(&response)?;
            check_encoding(&response)?;
            let range = if response.status() == StatusCode::PARTIAL_CONTENT {
                let parsed = content_range(&response)?;
                if parsed.start != 0 || parsed.end != (PROBE_BYTES - 1).min(parsed.total - 1) {
                    return Err(protocol("线路返回了错误的探测范围"));
                }
                if response
                    .content_length()
                    .is_some_and(|length| length != parsed.end + 1)
                {
                    return Err(protocol("线路返回的分段长度与范围不一致"));
                }
                Some(parsed)
            } else {
                None
            };
            let mut total = range
                .as_ref()
                .map(|range| range.total)
                .or(response.content_length());
            if let Some(metadata) = metadata {
                if total.is_some_and(|total| total != metadata.size) {
                    return Err(protocol("线路文件大小与 GitHub 官方记录不一致"));
                }
                total = Some(metadata.size);
            }
            let etag = strong_etag(&response);
            let html_type = is_html_type(&response);
            let mut sample = Vec::new();
            while sample.len() < PROBE_BYTES as usize {
                let Some(chunk) = response.chunk().await? else {
                    break;
                };
                let count = chunk.len().min(PROBE_BYTES as usize - sample.len());
                sample.extend_from_slice(&chunk[..count]);
            }
            reject_error_page(&sample, html_type, &source.filename)?;
            if let Some(total) = total {
                if sample.len() as u64 != total.min(PROBE_BYTES) {
                    return Err(protocol("线路探测内容被截断"));
                }
            }
            Ok(Probe {
                route,
                total,
                etag,
                range_supported: range.is_some(),
                bytes: sample.len() as u64,
                seconds: started.elapsed().as_secs_f64().max(0.001),
            })
        };
        tokio::select! {
            _ = token.cancelled() => Err(DownloadError::cancelled()),
            result = tokio::time::timeout(self.probe_timeout, work) => result.map_err(|_| DownloadError::new(ErrorKind::Network, "线路探测超时"))?,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub asset_id: Option<u64>,
    pub size: u64,
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseMetadata {
    assets: Vec<AssetMetadata>,
}

#[derive(Deserialize)]
struct AssetMetadata {
    id: Option<u64>,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Probe {
    pub route: Route,
    pub total: Option<u64>,
    pub etag: Option<String>,
    pub range_supported: bool,
    pub bytes: u64,
    pub seconds: f64,
}

impl Probe {
    pub fn throughput(&self) -> f64 {
        self.bytes as f64 / self.seconds
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    pub end: u64,
    pub total: u64,
}

pub fn parse_content_range(value: &str) -> Result<ContentRange> {
    let invalid = || protocol("服务返回了无效的 Content-Range");
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or_else(invalid)?;
    let (start, end) = range.split_once('-').ok_or_else(invalid)?;
    let start: u64 = start.parse().map_err(|_| invalid())?;
    let end: u64 = end.parse().map_err(|_| invalid())?;
    let total: u64 = total.parse().map_err(|_| invalid())?;
    if start > end || end >= total {
        return Err(invalid());
    }
    Ok(ContentRange { start, end, total })
}

pub fn content_range(response: &Response) -> Result<ContentRange> {
    parse_content_range(
        response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| protocol("分段响应缺少 Content-Range"))?,
    )
}

pub fn strong_etag(response: &Response) -> Option<String> {
    response
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
        .map(str::to_owned)
}

pub fn check_encoding(response: &Response) -> Result<()> {
    if response
        .headers()
        .get(header::CONTENT_ENCODING)
        .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(protocol("线路压缩了下载内容，无法保证字节范围正确"));
    }
    Ok(())
}

pub fn check_status(response: &Response) -> Result<()> {
    let status = response.status();
    if matches!(status, StatusCode::OK | StatusCode::PARTIAL_CONTENT) {
        return Ok(());
    }
    let retryable = status.is_server_error()
        || matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::REQUEST_TIMEOUT
        );
    let mut error = DownloadError::new(
        if retryable {
            ErrorKind::Network
        } else if status == StatusCode::NOT_FOUND {
            ErrorKind::NotFound
        } else {
            ErrorKind::Protocol
        },
        format!(
            "下载服务返回 HTTP {}{}",
            status.as_u16(),
            if status == StatusCode::NOT_FOUND {
                "，附件不存在或不可公开访问"
            } else {
                ""
            }
        ),
    );
    error.retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .parse::<u64>()
                .ok()
                .map(Duration::from_secs)
                .or_else(|| {
                    httpdate::parse_http_date(value)
                        .ok()?
                        .duration_since(SystemTime::now())
                        .ok()
                })
        });
    Err(error)
}

pub fn is_html_type(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
}

pub fn reject_error_page(bytes: &[u8], html_type: bool, filename: &str) -> Result<()> {
    let filename = filename.to_ascii_lowercase();
    if filename.ends_with(".html") || filename.ends_with(".htm") {
        return Ok(());
    }
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(512)])
        .trim_start_matches('\u{feff}')
        .trim_start()
        .to_ascii_lowercase();
    if html_type || prefix.starts_with("<!doctype html") || prefix.starts_with("<html") {
        return Err(protocol("线路返回了网页而非附件，已拒绝保存"));
    }
    Ok(())
}

pub fn protocol(message: impl Into<String>) -> DownloadError {
    DownloadError::new(ErrorKind::Protocol, message)
}

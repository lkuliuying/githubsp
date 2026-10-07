use crate::{
    catalog::Catalog,
    source::{parse_resource, Resource},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub status: String,
    pub current: String,
    pub latest: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub message: String,
    pub next_check: Option<u64>,
}
pub fn configured_repository() -> &'static str {
    option_env!("GITHUBSP_RELEASE_REPOSITORY").unwrap_or("")
}
pub fn compare(current: &str, latest: &str) -> std::result::Result<bool, semver::Error> {
    let current = semver::Version::parse(current.strip_prefix('v').unwrap_or(current))?;
    let latest = semver::Version::parse(latest.strip_prefix('v').unwrap_or(latest))?;
    Ok(latest > current)
}

/// 更新正文只允许通过系统浏览器打开 HTTPS GitHub 地址，不复用附件解析规则。
pub fn validate_update_link(input: &str) -> Result<url::Url, String> {
    let invalid = || "只允许打开有效的 HTTPS GitHub 更新说明链接".to_string();
    if !input
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        || input
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '\\')
    {
        return Err(invalid());
    }
    let authority = input[8..].split(['/', '?', '#']).next().unwrap_or_default();
    let bytes = input.as_bytes();
    if authority.contains('@')
        || bytes.iter().enumerate().any(|(index, byte)| {
            *byte == b'%'
                && (bytes
                    .get(index + 1)
                    .is_none_or(|byte| !byte.is_ascii_hexdigit())
                    || bytes
                        .get(index + 2)
                        .is_none_or(|byte| !byte.is_ascii_hexdigit()))
        })
    {
        return Err(invalid());
    }
    let decoded = percent_encoding::percent_decode_str(input)
        .decode_utf8()
        .map_err(|_| invalid())?;
    if decoded.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = url::Url::parse(input).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}

pub async fn check(catalog: &Catalog, repository: &str, current: &str) -> UpdateResult {
    let mut result = UpdateResult {
        status: "not_configured".into(),
        current: current.into(),
        latest: None,
        notes: None,
        url: None,
        message: "尚未配置官方发布源".into(),
        next_check: None,
    };
    if repository.trim().is_empty() {
        return result;
    }
    let input = format!("https://github.com/{}", repository.trim());
    if !matches!(parse_resource(&input), Ok(Resource::Repository(_))) {
        result.status = "invalid_source".into();
        result.message = "官方发布源配置无效，请使用 owner/repo".into();
        return result;
    }
    match catalog.browse(&input, 0, false).await {
        Ok(page) => {
            if let Some(release) = page.releases.into_iter().find(|r| !r.prerelease) {
                result.latest = Some(release.tag.clone());
                result.notes = Some(release.notes);
                result.url = Some(release.url);
                match compare(current, &release.tag) {
                    Ok(true) => {
                        result.status = "available".into();
                        result.message = "有新版本，可前往官方发布页获取".into();
                    }
                    Ok(false) => {
                        result.status = "current".into();
                        result.message = "当前版本无需更新".into();
                    }
                    Err(_) => {
                        result.status = "incomparable".into();
                        result.message = "版本号不符合语义化版本格式，请查看官方发布页".into();
                    }
                }
            } else {
                result.status = "failed".into();
                result.message = "官方仓库没有正式版本".into();
            }
        }
        Err(error) => {
            result.status = if error.retry_after.is_some() {
                "rate_limited"
            } else {
                "failed"
            }
            .into();
            result.message = error.message;
            if error.retry_after.is_some() {
                result.next_check = Some(catalog.next_allowed().await);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::validate_update_link;

    #[test]
    fn update_links_accept_github_pages() {
        for input in [
            "https://github.com/lkuliuying/githubsp/compare/v0.2.1...v0.2.2",
            "https://github.com/test/repo/releases/tag/v1",
            "https://github.com/test/repo/issues/1#issuecomment-1",
            "https://github.com/test/repo/pull/1?diff=split",
            "https://GITHUB.COM:443/test/repo",
            "https://github.com/test/repo/blob/main/%E4%B8%AD%E6%96%87%20test.md",
        ] {
            let url =
                validate_update_link(input).unwrap_or_else(|error| panic!("{input}: {error}"));
            assert_eq!(url.host_str(), Some("github.com"));
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.port_or_known_default(), Some(443));
        }
    }

    #[test]
    fn update_links_reject_invalid_or_external_targets() {
        for input in [
            "",
            " ",
            "not a url",
            "/test/repo",
            "//github.com/test/repo",
            "https:github.com/test/repo",
            "http://github.com/test/repo",
            "javascript:alert(1)",
            "file:///C:/test.exe",
            "https://github.com.evil/test/repo",
            "https://evil.github.com/test/repo",
            "https://github.com./test/repo",
            "https://user:pass@github.com/test/repo",
            "https://@github.com/test/repo",
            "https://github.com:8443/test/repo",
            "https://github.com\\@evil.test",
            "https://git\nhub.com/test/repo",
            " https://github.com/test/repo",
            "https://github.com/test/repo\0",
            "https://github.com/%0A",
            "https://github.com/%",
            "https://github.com/%FF",
        ] {
            assert!(validate_update_link(input).is_err(), "{input:?}");
        }
    }
}

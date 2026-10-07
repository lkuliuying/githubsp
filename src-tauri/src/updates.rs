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

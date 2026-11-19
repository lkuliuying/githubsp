//! 从剪贴板或粘贴的文字中识别 GitHub 下载来源。
//!
//! 只认 github.com 上的仓库、版本页和 Release 附件链接；其余内容一律忽略，
//! 调用方拿到的只是识别结果，不需要接触原始文字。

use crate::source::{parse_resource, Resource};
use serde::Serialize;

/// 自动识别时最多检查的字符数；更长的内容通常不是刚复制的链接。
pub const AUTO_LIMIT: usize = 8 * 1024;
/// 一次最多带入的附件链接数量。
const MAX_LINKS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionKind {
    /// 仓库页、版本页或最新版本页，需要读取版本列表。
    Repository,
    /// 一个或多个附件直链，可直接预览下载。
    Links,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkSuggestion {
    pub kind: SuggestionKind,
    /// 填入新建下载输入框的内容；附件链接按行分隔。
    pub source: String,
    /// 第一个链接所属的 `owner/repo`。
    pub repository: String,
    /// 版本页对应的版本号。
    pub version: Option<String>,
    /// 是否指向最新版本页。
    pub latest: bool,
    /// 附件链接数量；仓库类为 0。
    pub count: usize,
    /// 只有一个附件时的文件名。
    pub filename: Option<String>,
}

/// GitHub 站内功能页，不是仓库。
const RESERVED: &[&str] = &[
    "about",
    "apps",
    "collections",
    "customer-stories",
    "enterprise",
    "events",
    "explore",
    "features",
    "github-copilot",
    "issues",
    "login",
    "marketplace",
    "new",
    "notifications",
    "orgs",
    "organizations",
    "pricing",
    "pulls",
    "search",
    "security",
    "sessions",
    "settings",
    "signup",
    "sponsors",
    "team",
    "topics",
    "trending",
    "users",
];

/// 识别文字中的 GitHub 链接。附件链接优先；没有附件时取第一个仓库或版本页。
pub fn recognize(text: &str) -> Option<LinkSuggestion> {
    let mut assets: Vec<(String, Resource)> = Vec::new();
    let mut first_page: Option<(String, Resource)> = None;
    for candidate in candidates(text) {
        let Some((url, resource)) = resolve(&candidate) else {
            continue;
        };
        match resource {
            Resource::Asset(_) | Resource::Latest(_, Some(_)) => {
                if assets.len() < MAX_LINKS
                    && !assets.iter().any(|(u, _)| u.eq_ignore_ascii_case(&url))
                {
                    assets.push((url, resource));
                }
            }
            _ => {
                if first_page.is_none() {
                    first_page = Some((url, resource));
                }
            }
        }
    }
    if let Some((url, first)) = assets.first() {
        // 用原始链接解析，保留仓库名的大小写。
        let release = crate::source::parse_release_url(url).ok();
        let repository = match &release {
            Some(r) => format!("{}/{}", r.owner, r.repo),
            None => first.repository().ok()?,
        };
        let filename = match (assets.len(), first) {
            (1, Resource::Latest(_, name)) => name.clone(),
            (1, _) => release.as_ref().map(|r| r.filename.clone()),
            _ => None,
        };
        let version = release.map(|r| r.tag);
        return Some(LinkSuggestion {
            kind: SuggestionKind::Links,
            source: assets
                .iter()
                .map(|(u, _)| u.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            repository,
            version,
            latest: matches!(first, Resource::Latest(..)),
            count: assets.len(),
            filename,
        });
    }
    let (url, resource) = first_page?;
    let (repository, version, latest) = match resource {
        Resource::Repository(repo) => (repo, None, false),
        Resource::Release(repo, tag) => (repo, Some(tag), false),
        Resource::Latest(repo, _) => (repo, None, true),
        Resource::Asset(_) => return None,
    };
    Some(LinkSuggestion {
        kind: SuggestionKind::Repository,
        source: url,
        repository,
        version,
        latest,
        count: 0,
        filename: None,
    })
}

/// 把一段 github.com 地址规范成可下载的来源；功能页、子目录等退回到仓库首页。
fn resolve(candidate: &str) -> Option<(String, Resource)> {
    let base = candidate
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    // 克隆地址 owner/repo.git 按仓库处理。
    let base = base.strip_suffix(".git").unwrap_or(base);
    if let Ok(resource) = parse_resource(base) {
        let url = match &resource {
            Resource::Repository(repo) => format!("https://github.com/{repo}"),
            _ => base.to_string(),
        };
        return (!reserved_owner(&resource)).then_some((url, resource));
    }
    // 例如 /owner/repo/tree/main、/owner/repo/issues/1、/owner/repo.git。
    let mut parts = base.strip_prefix("https://github.com/")?.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let url = format!("https://github.com/{owner}/{repo}");
    let resource = parse_resource(&url).ok()?;
    (!reserved_owner(&resource)).then_some((url, resource))
}

fn reserved_owner(resource: &Resource) -> bool {
    resource.repository().is_ok_and(|repo| {
        let owner = repo.split('/').next().unwrap_or_default();
        RESERVED.iter().any(|r| r.eq_ignore_ascii_case(owner))
    })
}

/// 找出文字中所有 `github.com/...` 片段，统一成 `https://github.com/...`。
fn candidates(text: &str) -> Vec<String> {
    const HOST: &str = "github.com/";
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(HOST) {
        let host = from + offset;
        from = host + HOST.len();
        let mut start = host;
        if lower[..start].ends_with("www.") {
            start -= 4;
        }
        for scheme in ["https://", "http://"] {
            if lower[..start].ends_with(scheme) {
                start -= scheme.len();
                break;
            }
        }
        // api.github.com、gist.github.com、xgithub.com 或其他网址参数里的地址都不算。
        if lower[..start].chars().next_back().is_some_and(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '.' | '-' | '/' | '@' | '_' | '=' | '&' | '?' | '#' | '%' | '+' | '~'
                )
        }) {
            continue;
        }
        let rest = &text[from..];
        let end = rest.find(is_stop).unwrap_or(rest.len());
        let path = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '"']);
        if !path.is_empty() {
            found.push(format!("https://github.com/{path}"));
        }
        from += end;
    }
    found
}

fn is_stop(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(
            c,
            '<' | '>' | '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '^'
        )
        || "，。、；：！？“”‘’（）【】《》「」".contains(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_page_with_noise() {
        let s = recognize("看看这个 https://github.com/Owner/Repo 挺好用").unwrap();
        assert_eq!(s.kind, SuggestionKind::Repository);
        assert_eq!(s.source, "https://github.com/Owner/Repo");
        assert_eq!(s.repository, "Owner/Repo");
        assert_eq!(s.version, None);
    }

    #[test]
    fn browser_urls_fall_back_to_repository() {
        for text in [
            "https://github.com/owner/repo?tab=readme-ov-file",
            "https://github.com/owner/repo#install",
            "https://github.com/owner/repo/tree/main/src",
            "https://github.com/owner/repo/issues/12",
            "https://github.com/owner/repo.git",
            "github.com/owner/repo",
            "http://www.github.com/owner/repo/",
            "（https://github.com/owner/repo）",
        ] {
            let s = recognize(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(s.source, "https://github.com/owner/repo", "{text}");
            assert_eq!(s.kind, SuggestionKind::Repository);
        }
    }

    #[test]
    fn release_and_latest_pages() {
        let s = recognize("https://github.com/o/r/releases/tag/v1.2.0").unwrap();
        assert_eq!(s.version.as_deref(), Some("v1.2.0"));
        assert_eq!(s.source, "https://github.com/o/r/releases/tag/v1.2.0");
        let s = recognize("https://github.com/o/r/releases/latest").unwrap();
        assert!(s.latest);
        assert_eq!(s.kind, SuggestionKind::Repository);
    }

    #[test]
    fn asset_links_win_and_are_deduplicated() {
        let text = "仓库 https://github.com/o/r\n\
            https://github.com/o/r/releases/download/v1/app-x64.exe\n\
            https://github.com/o/r/releases/download/v1/app-x64.exe\n\
            https://github.com/o/r/releases/latest/download/app.zip";
        let s = recognize(text).unwrap();
        assert_eq!(s.kind, SuggestionKind::Links);
        assert_eq!(s.count, 2);
        assert_eq!(s.repository, "o/r");
        assert_eq!(s.version.as_deref(), Some("v1"));
        assert_eq!(s.source.lines().count(), 2);
        assert_eq!(s.filename, None);
        let one = recognize("https://github.com/o/r/releases/download/v1/app.exe。").unwrap();
        assert_eq!(one.filename.as_deref(), Some("app.exe"));
        let cased =
            recognize("https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/rg.zip")
                .unwrap();
        assert_eq!(cased.repository, "BurntSushi/ripgrep");
        assert_eq!(cased.version.as_deref(), Some("14.1.1"));
        assert_eq!(one.count, 1);
    }

    #[test]
    fn ignores_non_repository_pages() {
        for text in [
            "",
            "hello world",
            "password123",
            "https://github.com/",
            "https://github.com/owner",
            "https://github.com/settings/profile",
            "https://github.com/orgs/acme/repositories",
            "https://gist.github.com/owner/abc",
            "https://api.github.com/repos/o/r",
            "https://notgithub.com/o/r",
            "https://example.com/?u=github.com/o/r",
        ] {
            assert_eq!(recognize(text), None, "{text}");
        }
    }
}

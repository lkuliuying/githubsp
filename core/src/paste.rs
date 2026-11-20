//! 从剪贴板或粘贴的文字中识别 GitHub 下载来源。
//!
//! 只认 github.com 上的仓库、版本页和 Release 附件链接；其余内容一律忽略，
//! 调用方拿到的只是识别结果，不需要接触原始文字。
//!
//! 视频简介、帖子里的地址经常被写乱：全角字符、「github 点 com」、论坛的跳转链接、
//! 加速站前缀、只写 owner/repo 等。识别前先把这些写法还原成普通的 github.com 地址，
//! 下载时仍然只走 GitHub 官方地址。

use crate::source::{parse_resource, Resource};
use serde::Serialize;
use url::Url;

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
    recognize_text(text, false)
}

/// 用户主动粘贴或输入的内容：除了 [`recognize`] 能认的，整段只有 `owner/repo` 时也认。
pub fn recognize_input(text: &str) -> Option<LinkSuggestion> {
    recognize_text(text, true)
}

/// 把输入框里的单个来源规范成标准地址；认不出时原样返回，交给后续校验给出错误提示。
pub fn normalize_input(input: &str) -> String {
    let trimmed = input.trim();
    if parse_resource(trimmed).is_ok() {
        return trimmed.to_string();
    }
    match recognize_input(trimmed) {
        Some(s) if s.count <= 1 => s.source,
        _ => trimmed.to_string(),
    }
}

fn recognize_text(text: &str, bare: bool) -> Option<LinkSuggestion> {
    let text = normalize(text);
    let mut found = candidates(&text);
    found.extend(keyword_candidates(&text));
    if bare && found.is_empty() {
        let only = text.trim().trim_start_matches('/');
        if !only.is_empty() && !only.contains(char::is_whitespace) && !only.contains(':') {
            found.push(format!("https://github.com/{only}"));
        }
    }
    let mut assets: Vec<(String, Resource)> = Vec::new();
    let mut first_page: Option<(String, Resource)> = None;
    for candidate in found {
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
    // 仓库名后面紧跟的中文或标点（「owner/repo挺好用」）不算仓库名。
    let segment = parts.next()?;
    let repo = &segment[..segment
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(segment.len())];
    let repo = repo.trim_end_matches('.');
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
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
        // 加速站前缀：https://加速站/https://github.com/...，取出里面的 GitHub 地址。
        let proxied = behind_proxy(&lower[..start]);
        // api.github.com、gist.github.com、xgithub.com 或其他网址参数里的地址都不算。
        if !proxied
            && lower[..start].chars().next_back().is_some_and(|c| {
                c.is_ascii_alphanumeric()
                    || matches!(
                        c,
                        '.' | '-' | '/' | '@' | '_' | '=' | '&' | '?' | '#' | '%' | '+' | '~'
                    )
            })
        {
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

/// 前面紧挨着的是不是「scheme://主机名/」，即加速站或镜像站的前缀。
fn behind_proxy(prefix: &str) -> bool {
    prefix
        .strip_suffix('/')
        .and_then(|p| p.rsplit_once("://"))
        .is_some_and(|(_, host)| {
            host.contains('.')
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        })
}

/// 把常见的变形写法还原成普通的 github.com 地址。
fn normalize(text: &str) -> String {
    let text: String = text.chars().map(halfwidth).collect();
    let text = deobfuscate(&text);
    let text = unwrap_redirects(&text);
    let text = replace_ascii_ci(&text, "git@github.com:", "https://github.com/");
    let text = replace_ascii_ci(&text, "raw.githubusercontent.com/", "github.com/");
    replace_ascii_ci(&text, "codeload.github.com/", "github.com/")
}

/// 全角字母、数字和网址里会用到的符号转成半角；中文标点保持不变。
fn halfwidth(c: char) -> char {
    match c {
        '\u{FF10}'..='\u{FF19}'
        | '\u{FF21}'..='\u{FF3A}'
        | '\u{FF41}'..='\u{FF5A}'
        | '．'
        | '／'
        | '－'
        | '＿' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        _ => c,
    }
}

/// 只替换 ASCII 子串，忽略大小写。
fn replace_ascii_ci(text: &str, needle: &str, replacement: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (at, _) in lower.match_indices(needle) {
        out.push_str(&text[copied..at]);
        out.push_str(replacement);
        copied = at + needle.len();
    }
    out.push_str(&text[copied..]);
    out
}

fn is_inline_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{3000}')
}

/// 「github 点 com」「github[.]com」「github . com / owner/repo」等写法还原成 github.com/。
fn deobfuscate(text: &str) -> String {
    const SEPARATORS: &[&str] = &[
        "[.]",
        "(.)",
        "（.）",
        "[dot]",
        "(dot)",
        "（dot）",
        "dot",
        "点",
        "。",
        ".",
    ];
    let lower = text.to_ascii_lowercase();
    let skip = |i: usize| lower.len() - lower[i..].trim_start_matches(is_inline_space).len();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(offset) = lower[from..].find("github") {
        let start = from + offset;
        from = start + "github".len();
        let i = skip(from);
        let Some(separator) = SEPARATORS.iter().find(|s| lower[i..].starts_with(*s)) else {
            continue;
        };
        let i = skip(i + separator.len());
        if !lower[i..].starts_with("com") {
            continue;
        }
        let mut end = i + 3;
        if lower[end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            continue;
        }
        let j = skip(end);
        let slash = lower[j..].starts_with('/');
        if slash {
            end = skip(j + 1);
        }
        out.push_str(&text[copied..start]);
        out.push_str(if slash { "github.com/" } else { "github.com" });
        copied = end;
        from = end;
    }
    out.push_str(&text[copied..]);
    out
}

/// 论坛、博客把外链包了一层跳转，参数里才是真正的地址。
const REDIRECTS: &[(&str, &str)] = &[
    ("link.zhihu.com/", "target"),
    ("link.juejin.cn/", "target"),
    ("link.csdn.net/", "target"),
    ("www.jianshu.com/go-wild", "url"),
    ("gitee.com/link", "target"),
];

/// 把白名单里的跳转链接换成参数里的 GitHub 地址。
fn unwrap_redirects(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    for (host, param) in REDIRECTS {
        for (at, _) in lower.match_indices(*host) {
            let mut start = at;
            for scheme in ["https://", "http://"] {
                if lower[..start].ends_with(scheme) {
                    start -= scheme.len();
                    break;
                }
            }
            let end = at + lower[at..].find(is_stop).unwrap_or(lower.len() - at);
            let Ok(url) = Url::parse(&format!("https://{}", &text[at..end])) else {
                continue;
            };
            let target = url
                .query_pairs()
                .find(|(key, _)| key == *param)
                .map(|(_, value)| value.into_owned());
            if let Some(target) = target.filter(|t| t.to_ascii_lowercase().contains("github.com")) {
                spans.push((start, end, target));
            }
        }
    }
    spans.sort_by_key(|(start, ..)| *start);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, end, target) in spans {
        if start < copied {
            continue;
        }
        out.push_str(&text[copied..start]);
        out.push(' ');
        out.push_str(&target);
        out.push(' ');
        copied = end;
    }
    out.push_str(&text[copied..]);
    out
}

/// 没有完整网址、只在关键词后面写了 owner/repo 的情况，例如「GitHub：owner/repo」
/// 「GitHub 仓库地址 owner/repo」「gh repo clone owner/repo」。
fn keyword_candidates(text: &str) -> Vec<String> {
    /// 关键词和仓库名之间必须出现其中之一，避免把「GitHub 上 iOS/Android」当成仓库。
    const STRONG: &[&str] = &[
        ":",
        "：",
        "仓库",
        "地址",
        "项目",
        "搜索",
        "搜",
        "链接",
        "开源",
        "repository",
        "repo",
    ];
    /// 可以夹在中间、但单独出现不算数的字词。
    const WEAK: &[&str] = &["上", "的", "-", "—", ">", "→", "|", "是", "为"];
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    for keyword in ["github", "gh repo clone"] {
        for (at, _) in lower.match_indices(keyword) {
            let from = at + keyword.len();
            if lower[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric())
                || lower[from..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '@'))
            {
                continue;
            }
            let mut i = from;
            let mut hinted = keyword != "github";
            let mut steps = 0;
            while steps < 8 && i < lower.len() {
                steps += 1;
                let rest = &lower[i..];
                let spaced = rest.trim_start_matches(is_inline_space);
                if spaced.len() != rest.len() {
                    i = lower.len() - spaced.len();
                    continue;
                }
                if let Some(word) = STRONG.iter().find(|w| rest.starts_with(*w)) {
                    hinted = true;
                    i += word.len();
                } else if let Some(word) = WEAK.iter().find(|w| rest.starts_with(*w)) {
                    i += word.len();
                } else {
                    break;
                }
            }
            // 「repo」这类提示词本身也是字母，碰到时 i 已经越过它。
            if !hinted || !lower[i..].starts_with(|c: char| c.is_ascii_alphanumeric()) {
                continue;
            }
            let rest = &text[i..];
            let end = rest.find(is_stop).unwrap_or(rest.len());
            let path = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '"']);
            let owner = path.split('/').next().unwrap_or_default();
            // GitHub 用户名只有字母、数字和连字符。
            if path.contains('/') && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                found.push(format!("https://github.com/{path}"));
            }
        }
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

    fn repo(text: &str) -> Option<String> {
        recognize(text).map(|s| s.source)
    }

    #[test]
    fn obfuscated_and_fullwidth_hosts() {
        for text in [
            "github 点 com/owner/repo",
            "地址：github点com/owner/repo 自取",
            "GitHub[.]com/owner/repo",
            "github(.)com/owner/repo",
            "github . com / owner/repo",
            "github dot com/owner/repo",
            "github。com/owner/repo",
            "ｇｉｔｈｕｂ．ｃｏｍ／ｏｗｎｅｒ／ｒｅｐｏ",
            "https://github.com/owner/repo挺好用",
            "https://github.com/owner/repo,下载",
        ] {
            assert_eq!(
                repo(text).as_deref(),
                Some("https://github.com/owner/repo"),
                "{text}"
            );
        }
        // 正常英文句子不受影响。
        assert_eq!(repo("visit github.community/topic for help"), None);
        assert_eq!(repo("Fork me on GitHub. Community rocks"), None);
    }

    #[test]
    fn redirect_wrappers_are_unwrapped() {
        for text in [
            "https://link.zhihu.com/?target=https%3A//github.com/owner/repo",
            "链接 https://link.juejin.cn/?target=https%3A%2F%2Fgithub.com%2Fowner%2Frepo 好用",
            "https://link.csdn.net/?target=https%3A%2F%2Fgithub.com%2Fowner%2Frepo",
            "https://www.jianshu.com/go-wild?ac=2&url=https%3A%2F%2Fgithub.com%2Fowner%2Frepo",
            "https://gitee.com/link?target=https%3A%2F%2Fgithub.com%2Fowner%2Frepo",
        ] {
            assert_eq!(
                repo(text).as_deref(),
                Some("https://github.com/owner/repo"),
                "{text}"
            );
        }
        let s = recognize(
            "https://link.zhihu.com/?target=https%3A//github.com/o/r/releases/download/v1/app.exe",
        )
        .unwrap();
        assert_eq!(s.kind, SuggestionKind::Links);
        assert_eq!(
            s.source,
            "https://github.com/o/r/releases/download/v1/app.exe"
        );
        // 不在白名单里的网址参数仍然不认。
        assert_eq!(
            repo("https://example.com/?target=https%3A//github.com/o/r"),
            None
        );
    }

    #[test]
    fn proxy_prefixes_and_other_github_hosts() {
        let s = recognize("https://ghproxy.net/https://github.com/o/r/releases/download/v1/a.zip")
            .unwrap();
        assert_eq!(
            s.source,
            "https://github.com/o/r/releases/download/v1/a.zip"
        );
        assert_eq!(
            repo("https://gh-proxy.com/github.com/owner/repo").as_deref(),
            Some("https://github.com/owner/repo")
        );
        for text in [
            "https://raw.githubusercontent.com/owner/repo/main/install.sh",
            "https://codeload.github.com/owner/repo/zip/refs/heads/main",
            "git@github.com:owner/repo.git",
            "https://github.com/owner/repo/blob/main/README.md",
            "https://github.com/owner/repo/archive/refs/tags/v1.0.zip",
        ] {
            assert_eq!(
                repo(text).as_deref(),
                Some("https://github.com/owner/repo"),
                "{text}"
            );
        }
        // 路径中间出现的地址不算加速站前缀。
        assert_eq!(repo("https://example.com/a/b/https://github.com/o/r"), None);
    }

    #[test]
    fn keyword_followed_by_owner_repo() {
        for text in [
            "GitHub：owner/repo",
            "github: owner/repo 欢迎 star",
            "项目开源在 GitHub 仓库地址：owner/repo。",
            "GitHub 搜 owner/repo",
            "GitHub上搜索owner/repo",
            "gh repo clone owner/repo",
            "GitHub repo owner/repo",
        ] {
            assert_eq!(
                repo(text).as_deref(),
                Some("https://github.com/owner/repo"),
                "{text}"
            );
        }
        for text in [
            "在 GitHub 上看到 iOS/Android 都支持",
            "GitHub Actions/CI 配置",
            "GitHub上star/fork一下",
            "GitHubSP: owner/repo",
            "owner/repo",
            "1/2",
        ] {
            assert_eq!(repo(text), None, "{text}");
        }
    }

    #[test]
    fn input_accepts_bare_owner_repo() {
        let s = recognize_input(" Owner/Repo ").unwrap();
        assert_eq!(s.source, "https://github.com/Owner/Repo");
        assert_eq!(s.repository, "Owner/Repo");
        assert_eq!(
            recognize_input("owner/repo/releases/tag/v1")
                .unwrap()
                .version
                .as_deref(),
            Some("v1")
        );
        assert_eq!(recognize_input("and or"), None);
        assert_eq!(recognize_input("settings/profile"), None);
        assert_eq!(
            normalize_input("owner/repo"),
            "https://github.com/owner/repo"
        );
        assert_eq!(
            normalize_input("看这个 https://github.com/owner/repo?tab=readme 不错"),
            "https://github.com/owner/repo"
        );
        assert_eq!(
            normalize_input("https://github.com/o/r/releases/tag/v1"),
            "https://github.com/o/r/releases/tag/v1"
        );
        assert_eq!(normalize_input("hello"), "hello");
    }
}

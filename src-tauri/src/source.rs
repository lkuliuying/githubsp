use crate::error::{DownloadError, ErrorKind, Result};
use percent_encoding::percent_decode_str;
use url::Url;

#[derive(Debug, Clone)]
pub struct ReleaseSource {
    pub url: Url,
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource {
    Asset(String),
    Repository(String),
    Release(String, String),
    Latest(String, Option<String>),
}

impl Resource {
    pub fn repository(&self) -> Result<String> {
        match self {
            Self::Asset(url) => {
                let source = parse_release_url(url)?;
                Ok(format!("{}/{}", source.owner, source.repo))
            }
            Self::Repository(repo) | Self::Release(repo, _) | Self::Latest(repo, _) => {
                Ok(repo.clone())
            }
        }
    }
}

pub fn parse_resource(input: &str) -> Result<Resource> {
    if let Ok(source) = parse_release_url(input) {
        return Ok(Resource::Asset(source.url.to_string()));
    }
    let invalid = || {
        DownloadError::new(
            ErrorKind::InvalidInput,
            "请输入公开 GitHub 仓库、版本页面或 Release 附件链接",
        )
    };
    let raw = input.trim().trim_end_matches('/');
    if raw.len() > 8192 || raw.chars().any(char::is_control) || raw.contains('\\') {
        return Err(invalid());
    }
    let url = Url::parse(raw).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    let parts: Vec<String> = url
        .path_segments()
        .ok_or_else(invalid)?
        .map(|s| {
            percent_decode_str(s)
                .decode_utf8()
                .map(|v| v.into_owned())
                .map_err(|_| invalid())
        })
        .collect::<Result<_>>()?;
    if parts.len() < 2 {
        return Err(invalid());
    }
    for part in &parts[..2] {
        if part.is_empty()
            || part == "."
            || part == ".."
            || !part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(invalid());
        }
    }
    let repo = format!("{}/{}", parts[0], parts[1]);
    match parts.get(2..).unwrap_or_default() {
        [] => Ok(Resource::Repository(repo)),
        [releases] if releases == "releases" => Ok(Resource::Repository(repo)),
        [releases, latest] if releases == "releases" && latest == "latest" => {
            Ok(Resource::Latest(repo, None))
        }
        [releases, tag, version]
            if releases == "releases"
                && tag == "tag"
                && !version.is_empty()
                && !version.chars().any(char::is_control) =>
        {
            Ok(Resource::Release(repo, version.clone()))
        }
        [releases, latest, download, name]
            if releases == "releases" && latest == "latest" && download == "download" =>
        {
            validate_filename(name)?;
            Ok(Resource::Latest(repo, Some(name.clone())))
        }
        _ => Err(invalid()),
    }
}

pub fn parse_release_url(input: &str) -> Result<ReleaseSource> {
    let source = parse_asset_location(input)?;
    validate_filename(&source.filename)?;
    Ok(source)
}

pub(crate) fn parse_asset_location(input: &str) -> Result<ReleaseSource> {
    let invalid = || {
        DownloadError::new(
            ErrorKind::InvalidInput,
            "请输入公开 GitHub Release 附件链接（releases/download/版本/文件名）",
        )
    };
    let raw = input.trim();
    if raw.len() > 8192 || raw.chars().any(char::is_control) || raw.contains('\\') {
        return Err(invalid());
    }
    let mut url = Url::parse(raw).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    let segments: Vec<&str> = url.path_segments().ok_or_else(invalid)?.collect();
    if segments.len() != 6 || segments[2] != "releases" || segments[3] != "download" {
        return Err(invalid());
    }
    let decode = |value: &str| -> Result<String> {
        percent_decode_str(value)
            .decode_utf8()
            .map(|value| value.into_owned())
            .map_err(|_| invalid())
    };
    let owner = decode(segments[0])?;
    let repo = decode(segments[1])?;
    let tag = decode(segments[4])?;
    let filename = decode(segments[5])?;
    for component in [&owner, &repo] {
        if component.is_empty()
            || component == "."
            || component == ".."
            || !component
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(invalid());
        }
    }
    if tag.is_empty() || tag.chars().any(char::is_control) {
        return Err(invalid());
    }
    url.path_segments_mut()
        .map_err(|_| invalid())?
        .clear()
        .extend([
            &owner.to_ascii_lowercase(),
            &repo.to_ascii_lowercase(),
            "releases",
            "download",
            &tag,
            &filename,
        ]);
    Ok(ReleaseSource {
        url,
        owner,
        repo,
        tag,
        filename,
    })
}

pub fn validate_filename(filename: &str) -> Result<()> {
    let stem = filename
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if filename.is_empty()
        || filename.encode_utf16().count() > 200
        || filename.ends_with(['.', ' '])
        || filename
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || reserved
        || filename == "."
        || filename == ".."
    {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "附件文件名不能安全保存到 Windows，请使用有效的附件链接",
        ));
    }
    Ok(())
}

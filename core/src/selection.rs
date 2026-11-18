use crate::{
    catalog::Release,
    error::{DownloadError, ErrorKind, Result},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AssetPreference {
    pub platform: String,
    pub architecture: String,
    pub package: String,
}

impl Default for AssetPreference {
    fn default() -> Self {
        Self {
            platform: "windows".into(),
            architecture: "x64".into(),
            package: "installer".into(),
        }
    }
}

impl AssetPreference {
    pub fn validate(&self) -> Result<()> {
        if !matches!(
            self.platform.as_str(),
            "windows" | "macos" | "linux" | "any"
        ) || !matches!(self.architecture.as_str(), "x64" | "arm64" | "x86" | "any")
            || !matches!(self.package.as_str(), "installer" | "portable" | "any")
        {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "附件平台、架构或分发类型无效",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub url: String,
    pub name: String,
    pub eligible: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub state: String,
    pub message: String,
    pub selected_urls: Vec<String>,
    pub candidates: Vec<Candidate>,
}

/// 只在规则唯一命中时推荐；文件名不能确认的平台和架构交给用户选择。
pub fn recommend(
    release: &Release,
    preference: Option<&AssetPreference>,
    pattern: Option<&str>,
) -> Result<Recommendation> {
    let defaults = AssetPreference::default();
    let target = preference.unwrap_or(&defaults);
    target.validate()?;
    let pattern = pattern.filter(|value| !value.trim().is_empty());
    if pattern.is_some_and(|value| value.len() > 200) {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "附件匹配规则过长，请控制在 200 个字符内",
        ));
    }
    let mut candidates = Vec::new();
    for asset in &release.assets {
        let reason = if let Some(error) = &asset.unavailable {
            Some(error.clone())
        } else if release.atom {
            Some("降级发现的附件信息尚未由官方接口确认".into())
        } else if let Some(pattern) = pattern {
            let with_tag = pattern.replace("{version}", &release.tag);
            let without_prefix =
                pattern.replace("{version}", release.tag.trim_start_matches(['v', 'V']));
            if !matches_pattern(&with_tag, &asset.name)
                && !matches_pattern(&without_prefix, &asset.name)
            {
                Some("附件名不符合自定义规则".into())
            } else if preference.is_some() {
                mismatch(&asset.name, target)
            } else {
                // 既有明确规则保持原语义；新保存的偏好则与规则共同约束。
                None
            }
        } else {
            mismatch(&asset.name, target)
        };
        candidates.push(Candidate {
            url: asset.url.clone(),
            name: asset.name.clone(),
            eligible: reason.is_none(),
            reason: reason.unwrap_or_else(|| {
                if pattern.is_some() {
                    "命中附件名称规则".into()
                } else {
                    format!(
                        "符合 {} / {} / {} 偏好",
                        target.platform, target.architecture, target.package
                    )
                }
            }),
        });
    }
    let matches: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.eligible)
        .collect();
    let (state, message) = if release.atom {
        ("incomplete", "附件信息不完整，请恢复官方接口后重新读取")
    } else if matches.len() == 1 {
        ("selected", "唯一附件符合规则；请在下载前确认")
    } else if matches.is_empty() {
        ("unmatched", "没有可确认的匹配附件，请调整偏好或手动选择")
    } else {
        ("ambiguous", "有多个附件符合规则，请收紧规则或手动选择")
    };
    let selected_urls = if matches.len() == 1 && !release.atom {
        vec![matches[0].url.clone()]
    } else {
        Vec::new()
    };
    Ok(Recommendation {
        state: state.into(),
        message: message.into(),
        selected_urls,
        candidates,
    })
}

fn mismatch(name: &str, preference: &AssetPreference) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let tokens: Vec<_> = n.split(['-', '_', '.', ' ']).collect();
    if tokens.iter().any(|token| {
        matches!(
            *token,
            "source" | "sources" | "symbols" | "debug" | "checksums" | "sha256" | "sha512"
        )
    }) {
        return Some("辅助文件或源码包，请手动确认用途".into());
    }
    let windows = n.contains("windows")
        || n.contains("win32")
        || n.contains("win64")
        || tokens.contains(&"win")
        || [".exe", ".msi", ".msix"]
            .iter()
            .any(|suffix| n.ends_with(suffix));
    let macos = n.contains("darwin")
        || n.contains("macos")
        || tokens.contains(&"osx")
        || n.ends_with(".dmg")
        || n.ends_with(".pkg");
    let linux = n.contains("linux")
        || [".appimage", ".deb", ".rpm"]
            .iter()
            .any(|suffix| n.ends_with(suffix));
    let platforms: Vec<_> = [(windows, "windows"), (macos, "macos"), (linux, "linux")]
        .into_iter()
        .filter_map(|(matched, value)| matched.then_some(value))
        .collect();
    if preference.platform != "any" && (platforms.len() != 1 || platforms[0] != preference.platform)
    {
        return Some(
            if platforms.is_empty() {
                "无法从文件名确认目标平台"
            } else {
                "平台不匹配或包含多个平台标记"
            }
            .into(),
        );
    }
    let x64 = n.contains("x86_64")
        || n.contains("amd64")
        || tokens.contains(&"x64")
        || n.contains("win64");
    let arm64 = n.contains("aarch64") || n.contains("arm64");
    let x86 = n.contains("i386")
        || n.contains("i686")
        || n.contains("win32")
        || (tokens.contains(&"x86") && !n.contains("x86_64"));
    let architectures: Vec<_> = [(x64, "x64"), (arm64, "arm64"), (x86, "x86")]
        .into_iter()
        .filter_map(|(matched, value)| matched.then_some(value))
        .collect();
    let universal = tokens.contains(&"universal") && architectures.is_empty();
    if preference.architecture != "any"
        && !universal
        && (architectures.len() != 1 || architectures[0] != preference.architecture)
    {
        return Some(
            if architectures.is_empty() {
                "无法从文件名确认目标架构"
            } else {
                "架构不匹配或包含多个架构标记"
            }
            .into(),
        );
    }
    let archive = [
        ".zip", ".7z", ".rar", ".tar.gz", ".tgz", ".tar.xz", ".tar.zst", ".tar.bz2",
    ]
    .iter()
    .any(|suffix| n.ends_with(suffix));
    let installer = [
        ".exe",
        ".msi",
        ".msix",
        ".dmg",
        ".pkg",
        ".appimage",
        ".deb",
        ".rpm",
    ]
    .iter()
    .any(|suffix| n.ends_with(suffix));
    let portable = archive || tokens.contains(&"portable") || tokens.contains(&"standalone");
    if !archive && !installer {
        return Some("不是可识别的软件分发文件，请手动确认".into());
    }
    if (preference.package == "installer" && (!installer || portable))
        || (preference.package == "portable" && !portable)
    {
        return Some("安装版/便携版偏好不匹配".into());
    }
    None
}

pub fn matches_pattern(pattern: &str, name: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    let (pattern, name) = (pattern.as_bytes(), name.as_bytes());
    let (mut p, mut n) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = n;
        } else if let Some(star) = star {
            p = star + 1;
            retry += 1;
            n = retry;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Asset;

    fn release(names: &[&str]) -> Release {
        Release {
            id: 1,
            tag: "v1.2.0".into(),
            name: String::new(),
            prerelease: false,
            url: "https://github.com/o/r/releases/tag/v1.2.0".into(),
            notes: String::new(),
            published_at: None,
            atom: false,
            assets: names
                .iter()
                .enumerate()
                .map(|(i, name)| Asset {
                    id: i as u64,
                    name: (*name).into(),
                    size: 1,
                    url: format!("https://github.com/o/r/releases/download/v1.2.0/{name}"),
                    sha256: None,
                    hints: vec![],
                    unavailable: None,
                })
                .collect(),
        }
    }

    #[test]
    fn selects_architecture_and_distribution_without_guessing_unknowns() {
        let r = release(&[
            "Tool-windows-x64-Setup.exe",
            "Tool-windows-arm64-Setup.exe",
            "Tool-windows-x64-Portable.zip",
            "Tool.dmg",
        ]);
        let mut target = AssetPreference::default();
        assert!(
            recommend(&r, Some(&target), None).unwrap().selected_urls[0].ends_with("x64-Setup.exe")
        );
        target.package = "portable".into();
        assert!(
            recommend(&r, Some(&target), None).unwrap().selected_urls[0].ends_with("Portable.zip")
        );
        target.architecture = "arm64".into();
        assert_eq!(
            recommend(&r, Some(&target), None).unwrap().state,
            "unmatched"
        );
        assert_eq!(
            recommend(&release(&["Tool-Setup.exe"]), None, None)
                .unwrap()
                .state,
            "unmatched"
        );
    }

    #[test]
    fn ambiguity_and_fallback_metadata_never_auto_select() {
        let mut r = release(&["Tool-windows-x64.exe", "Tool-windows-x64.msi"]);
        assert_eq!(recommend(&r, None, None).unwrap().state, "ambiguous");
        assert!(recommend(&r, None, None).unwrap().selected_urls.is_empty());
        r.atom = true;
        assert_eq!(
            recommend(&r, None, Some("*.exe")).unwrap().state,
            "incomplete"
        );
    }

    #[test]
    fn explicit_legacy_patterns_and_version_placeholders_keep_their_meaning() {
        let r = release(&["Tool-1.2.0.zip", "Tool-v1.2.0.zip"]);
        assert_eq!(
            recommend(&r, None, Some("Tool-{version}.zip"))
                .unwrap()
                .state,
            "ambiguous"
        );
        assert_eq!(
            recommend(
                &release(&["Tool-1.2.0.zip"]),
                None,
                Some("Tool-{version}.zip")
            )
            .unwrap()
            .state,
            "selected"
        );
        assert_eq!(
            recommend(
                &release(&["Tool-1.2.0.zip"]),
                Some(&AssetPreference::default()),
                Some("*.zip")
            )
            .unwrap()
            .state,
            "unmatched"
        );
    }

    #[test]
    fn universal_packages_and_platform_markers_are_not_confused() {
        let target = AssetPreference {
            platform: "macos".into(),
            architecture: "arm64".into(),
            package: "installer".into(),
        };
        assert_eq!(
            recommend(
                &release(&["Tool-darwin-universal.dmg"]),
                Some(&target),
                None
            )
            .unwrap()
            .state,
            "selected"
        );
        assert_eq!(
            recommend(&release(&["Tool-darwin-x64.zip"]), None, None)
                .unwrap()
                .state,
            "unmatched"
        );
        assert_eq!(
            recommend(&release(&["Tool-windows-x64-source.zip"]), None, None)
                .unwrap()
                .state,
            "unmatched"
        );
        assert!(AssetPreference {
            architecture: "invalid".into(),
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}

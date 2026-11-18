use crate::{
    error::{DownloadError, ErrorKind, Result},
    model::{Favorite, WatchRule, WatchState},
    selection::AssetPreference,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableRule {
    pub include_prerelease: bool,
    pub auto_download: bool,
    pub asset_pattern: Option<String>,
    pub preference: Option<AssetPreference>,
}
impl From<&WatchRule> for PortableRule {
    fn from(rule: &WatchRule) -> Self {
        Self {
            include_prerelease: rule.include_prerelease,
            auto_download: rule.auto_download,
            asset_pattern: rule.asset_pattern.clone(),
            preference: rule.preference.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub repository: String,
    pub group: String,
    pub tags: Vec<String>,
    pub rule: PortableRule,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub projects: Vec<Project>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub repository: String,
    pub state: String,
    pub action: String,
    pub auto_download: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub fingerprint: String,
    pub directory: std::path::PathBuf,
    pub policy: String,
    pub items: Vec<ImportItem>,
}

pub fn labels(group: String, tags: Vec<String>) -> Result<(String, Vec<String>)> {
    if group.chars().count() > 40
        || group.chars().any(char::is_control)
        || tags.len() > 12
        || tags.iter().any(|tag| {
            tag.trim().is_empty() || tag.chars().count() > 40 || tag.chars().any(char::is_control)
        })
    {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "分组最多 40 字，标签最多 12 个且每个 1–40 字，不接受控制字符",
        ));
    }
    let mut tags: Vec<_> = tags.into_iter().map(|tag| tag.trim().to_owned()).collect();
    tags.sort();
    tags.dedup();
    Ok((group.trim().to_owned(), tags))
}
pub fn validate_rule(rule: &PortableRule) -> Result<()> {
    if let Some(preference) = &rule.preference {
        preference.validate()?;
    }
    if rule
        .asset_pattern
        .as_ref()
        .is_some_and(|pattern| pattern.len() > 200)
    {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "附件匹配规则过长，请控制在 200 个字符内",
        ));
    }
    Ok(())
}
pub fn parse(input: &str) -> Result<Manifest> {
    if input.len() > 1_048_576 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "关注清单超过 1 MiB",
        ));
    }
    let mut manifest: Manifest = serde_json::from_str(input)?;
    if manifest.schema != 1 || manifest.projects.len() > 200 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "仅支持 schema=1、最多 200 个项目的关注清单",
        ));
    }
    let mut repositories = BTreeSet::new();
    for project in &mut manifest.projects {
        let canonical =
            crate::source::parse_resource(&format!("https://github.com/{}", project.repository))?
                .repository()?;
        if canonical != project.repository || !repositories.insert(canonical.to_ascii_lowercase()) {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "关注清单的仓库名称无效或重复",
            ));
        }
        (project.group, project.tags) = labels(project.group.clone(), project.tags.clone())?;
        validate_rule(&project.rule)?;
    }
    Ok(manifest)
}
pub fn export(favorites: &[Favorite], state: &WatchState, global_auto: bool) -> Manifest {
    Manifest {
        schema: 1,
        projects: favorites
            .iter()
            .map(|favorite| {
                let entry = state
                    .entry(&favorite.repository)
                    .cloned()
                    .unwrap_or_default();
                Project {
                    repository: favorite.repository.clone(),
                    group: entry.group,
                    tags: entry.tags,
                    rule: PortableRule {
                        auto_download: entry.rule.automatic(global_auto),
                        ..PortableRule::from(&entry.rule)
                    },
                }
            })
            .collect(),
    }
}
pub fn write_file(manifest: &Manifest, target: &Path) -> Result<()> {
    use std::io::Write;
    if !target.is_absolute() {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "清单导出位置必须是绝对路径",
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| DownloadError::new(ErrorKind::InvalidInput, "清单导出目录无效"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(target)
        .map_err(|error| DownloadError::from(error.error))?;
    Ok(())
}
pub fn preview(
    manifest: &Manifest,
    directory: &Path,
    policy: &str,
    favorites: &[Favorite],
    state: &WatchState,
    global_auto: bool,
) -> Result<Preview> {
    if !["keep", "replace"].contains(&policy) {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "导入策略只支持 keep 或 replace",
        ));
    }
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "导入时需重新选择已存在的绝对下载根目录",
        ));
    }
    let directory = crate::preflight::inspect_directory(directory)?.directory;
    let current = export(favorites, state, global_auto);
    let mut relevant = Vec::new();
    let mut added = 0;
    let items = manifest
        .projects
        .iter()
        .map(|project| {
            let old = current
                .projects
                .iter()
                .find(|old| old.repository.eq_ignore_ascii_case(&project.repository));
            let favorite = favorites.iter().find(|favorite| {
                favorite
                    .repository
                    .eq_ignore_ascii_case(&project.repository)
            });
            relevant.push((
                &project.repository,
                favorite.map(|favorite| &favorite.id),
                old,
                state.entry(&project.repository).map(|entry| &entry.rule),
            ));
            if old.is_none() {
                added += 1;
            }
            let status = match old {
                None => "new",
                Some(old)
                    if old.rule == project.rule
                        && old.group == project.group
                        && old.tags == project.tags =>
                {
                    "duplicate"
                }
                _ => "conflict",
            };
            ImportItem {
                repository: project.repository.clone(),
                state: status.into(),
                action: if old.is_none() {
                    "add"
                } else if policy == "replace" {
                    "replace"
                } else {
                    "keep"
                }
                .into(),
                auto_download: if old.is_some() && policy == "keep" {
                    old.map(|project| project.rule.auto_download)
                        .unwrap_or(false)
                } else {
                    project.rule.auto_download
                },
            }
        })
        .collect();
    if favorites.len() + added > 200 {
        return Err(DownloadError::new(
            ErrorKind::InvalidInput,
            "合并后的收藏超过 200 个项目",
        ));
    }
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            manifest, &directory, policy, relevant
        ))?)
    );
    Ok(Preview {
        fingerprint,
        directory,
        policy: policy.into(),
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_manifest_rejects_paths_credentials_and_duplicate_repositories() {
        let legacy: WatchRule = serde_json::from_str(r#"{"autoDownload":false}"#).unwrap();
        assert!(legacy.automatic(true));
        assert!(parse(r#"{"schema":1,"projects":[],"token":"secret"}"#).is_err());
        assert!(parse(r#"{"schema":1,"projects":[{"repository":"o/r","group":"","tags":[],"rule":{"downloadDirectory":"C:/private"}}]}"#).is_err());
        assert!(parse(r#"{"schema":1,"projects":[{"repository":"o/r","group":"","tags":[],"rule":{}},{"repository":"O/R","group":"","tags":[],"rule":{}}]}"#).is_err());
        assert!(parse(r#"{"schema":2,"projects":[]}"#).is_err());
    }
}

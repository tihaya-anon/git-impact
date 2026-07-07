use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::git;

const DEFAULT_NODE_NAME: &str = "node";

#[derive(Debug, Eq, PartialEq)]
pub struct InitResult {
    pub path: PathBuf,
    pub has_patterns: bool,
}

#[derive(Serialize)]
struct GeneratedConfig {
    nodes: std::collections::BTreeMap<String, GeneratedNode>,
}

#[derive(Serialize)]
struct GeneratedNode {
    paths: Vec<String>,
    command: Vec<String>,
}

pub fn init_config(config_path: &Path, force: bool) -> Result<InitResult> {
    let repo_root = git::repo_root_in(std::env::current_dir()?)?;
    init_config_at_repo_root(&repo_root, config_path, force)
}

#[cfg(test)]
fn init_config_in(
    working_dir: impl AsRef<Path>,
    config_path: &Path,
    force: bool,
) -> Result<PathBuf> {
    let repo_root = git::repo_root_in(working_dir)?;
    Ok(init_config_at_repo_root(&repo_root, config_path, force)?.path)
}

fn init_config_at_repo_root(
    repo_root: &Path,
    config_path: &Path,
    force: bool,
) -> Result<InitResult> {
    let target_path = target_config_path(repo_root, config_path);

    if target_path.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite it",
            target_path.display()
        );
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let files = repo_files_without_target(repo_root, &target_path)?;
    let patterns = default_patterns(&files);
    let has_patterns = !patterns.is_empty();
    let content = generate_config(patterns)?;

    fs::write(&target_path, content)
        .with_context(|| format!("failed to write config {}", target_path.display()))?;

    Ok(InitResult {
        path: target_path,
        has_patterns,
    })
}

fn target_config_path(repo_root: &Path, config_path: &Path) -> PathBuf {
    if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        repo_root.join(config_path)
    }
}

fn repo_files_without_target(repo_root: &Path, target_path: &Path) -> Result<Vec<String>> {
    let target = target_path
        .strip_prefix(repo_root)
        .ok()
        .map(path_to_git_style);

    Ok(git::repo_files_in(repo_root)?
        .into_iter()
        .filter(|file| Some(file.as_str()) != target.as_deref())
        .collect())
}

fn path_to_git_style(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn default_patterns(files: &[String]) -> Vec<String> {
    let mut patterns = std::collections::BTreeSet::new();

    for file in files {
        if file.trim().is_empty() {
            continue;
        }

        if let Some((top_level, _)) = file.split_once('/') {
            patterns.insert(format!("{top_level}/**"));
        } else {
            patterns.insert(file.clone());
        }
    }

    patterns.into_iter().collect()
}

fn generate_config(patterns: Vec<String>) -> Result<String> {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        DEFAULT_NODE_NAME.to_owned(),
        GeneratedNode {
            paths: patterns,
            command: vec![
                "echo".to_owned(),
                "git-impact: node was impacted".to_owned(),
            ],
        },
    );

    serde_yaml::to_string(&GeneratedConfig { nodes })
        .with_context(|| "failed to generate YAML config")
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn creates_config_under_nearest_git_root() {
        let repo = temp_test_dir();
        let nested = repo.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        git(&repo, ["init"]);
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

        let target = init_config_in(&nested, Path::new("git-impact.yaml"), false).unwrap();

        assert_eq!(target, repo.join("git-impact.yaml"));
        let content = fs::read_to_string(target).unwrap();
        assert!(content.contains("Cargo.toml"));
        assert!(content.contains("src/**"));
        assert!(content.contains("echo"));

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn creates_placeholder_config_when_repo_has_no_files() {
        let repo = temp_test_dir();
        fs::create_dir_all(&repo).unwrap();
        git(&repo, ["init"]);

        let result = init_config_at_repo_root(&repo, Path::new("git-impact.yaml"), false).unwrap();

        assert_eq!(
            result,
            InitResult {
                path: repo.join("git-impact.yaml"),
                has_patterns: false
            }
        );
        let content = fs::read_to_string(repo.join("git-impact.yaml")).unwrap();
        assert!(content.contains("paths: []"));
        crate::config::Config::from_yaml(&content).unwrap();

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_existing_config_without_force() {
        let repo = temp_test_dir();
        fs::create_dir_all(&repo).unwrap();
        git(&repo, ["init"]);
        fs::write(repo.join("git-impact.yaml"), "existing\n").unwrap();

        let error = init_config_in(&repo, Path::new("git-impact.yaml"), false).unwrap_err();

        assert!(error.to_string().contains("already exists"));
        assert_eq!(
            fs::read_to_string(repo.join("git-impact.yaml")).unwrap(),
            "existing\n"
        );

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn force_overwrites_existing_config() {
        let repo = temp_test_dir();
        fs::create_dir_all(&repo).unwrap();
        git(&repo, ["init"]);
        fs::write(repo.join("git-impact.yaml"), "existing\n").unwrap();

        let target = init_config_in(&repo, Path::new("git-impact.yaml"), true).unwrap();

        assert_eq!(target, repo.join("git-impact.yaml"));
        assert!(
            fs::read_to_string(repo.join("git-impact.yaml"))
                .unwrap()
                .contains("paths: []")
        );

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn collapses_nested_files_to_top_level_patterns() {
        let patterns = default_patterns(&[
            "Cargo.toml".to_owned(),
            "src/main.rs".to_owned(),
            "src/lib.rs".to_owned(),
            "tests/cli.rs".to_owned(),
        ]);

        assert_eq!(
            patterns,
            vec![
                "Cargo.toml".to_owned(),
                "src/**".to_owned(),
                "tests/**".to_owned()
            ]
        );
    }

    fn temp_test_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("git-impact-init-test-{id}"))
    }

    fn git<const N: usize>(working_dir: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(working_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

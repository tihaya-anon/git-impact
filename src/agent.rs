use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::git;

const SKILL_NAME: &str = "git-impact-quality-hooks";
const SKILL_MD: &str = include_str!("../skills/git-impact-quality-hooks/SKILL.md");
const OPENAI_YAML: &str = include_str!("../skills/git-impact-quality-hooks/agents/openai.yaml");

pub fn render_prompt(config_path: &Path, base: &str, head: &str) -> String {
    format!(
        r#"You are an AI coding agent working in a Git repository. Use git-impact as the changed-file quality gate.

Setup:
1. Check whether `{config}` exists. If it does not, run `git-impact --config {config} init`.
2. Inspect `{config}` and replace placeholder `echo` commands with deterministic quality hooks for this repo.
3. Prefer small nodes tied to file patterns, for example Python files -> `ruff format .`, Rust files -> `cargo fmt`, shared libraries -> tests that cover dependents.
4. Use `depends_on` when one check must run before another check, or when downstream nodes should be impacted by upstream changes.
5. Keep commands as argv arrays. Do not rely on shell pipes, redirection, aliases, or interactive prompts.

Before finishing changes:
1. Run `git-impact --config {config} validate`.
2. Run `git-impact --config {config} tree --base {base} --head {head}` and inspect the impacted graph.
3. Run `git-impact --config {config} run --base {base} --head {head}` when the tools are available in this environment.
4. If a command cannot run because a tool is missing, report the missing tool and the skipped git-impact node.

For reusable agent instructions, run `git-impact skills` to create a `{skill}` skill folder in this repo.
"#,
        config = config_path.display(),
        base = base,
        head = head,
        skill = SKILL_NAME,
    )
}

pub fn create_skill(output_dir: &Path, force: bool) -> Result<PathBuf> {
    let repo_root = git::repo_root_in(std::env::current_dir()?)?;
    create_skill_at_repo_root(&repo_root, output_dir, force)
}

fn create_skill_at_repo_root(repo_root: &Path, output_dir: &Path, force: bool) -> Result<PathBuf> {
    let skill_dir = target_output_dir(repo_root, output_dir).join(SKILL_NAME);
    let skill_file = skill_dir.join("SKILL.md");
    let agents_dir = skill_dir.join("agents");
    let openai_file = agents_dir.join("openai.yaml");

    if skill_dir.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite generated skill files",
            skill_dir.display()
        );
    }

    fs::create_dir_all(&agents_dir)
        .with_context(|| format!("failed to create {}", agents_dir.display()))?;
    fs::write(&skill_file, SKILL_MD)
        .with_context(|| format!("failed to write {}", skill_file.display()))?;
    fs::write(&openai_file, OPENAI_YAML)
        .with_context(|| format!("failed to write {}", openai_file.display()))?;

    Ok(skill_dir)
}

fn target_output_dir(repo_root: &Path, output_dir: &Path) -> PathBuf {
    if output_dir.is_absolute() {
        output_dir.to_path_buf()
    } else {
        repo_root.join(output_dir)
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn prompt_contains_setup_commands() {
        let prompt = render_prompt(Path::new("git-impact.yaml"), "main", "HEAD");

        assert!(prompt.contains("git-impact --config git-impact.yaml init"));
        assert!(prompt.contains("git-impact --config git-impact.yaml validate"));
        assert!(prompt.contains("git-impact skills"));
        assert!(prompt.contains("ruff format"));
    }

    #[test]
    fn creates_skill_under_repo_root() {
        let repo = temp_test_dir();
        let nested = repo.join("a/b");
        fs::create_dir_all(&nested).unwrap();
        git(&repo, ["init"]);

        let skill_dir = create_skill_at_repo_root(&repo, Path::new("skills"), false).unwrap();

        assert_eq!(skill_dir, repo.join("skills").join(SKILL_NAME));
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("agents/openai.yaml").exists());

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_skill_without_force() {
        let repo = temp_test_dir();
        fs::create_dir_all(repo.join("skills").join(SKILL_NAME)).unwrap();
        git(&repo, ["init"]);

        let error = create_skill_at_repo_root(&repo, Path::new("skills"), false).unwrap_err();

        assert!(error.to_string().contains("already exists"));

        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn force_overwrites_generated_skill_files() {
        let repo = temp_test_dir();
        let skill_dir = repo.join("skills").join(SKILL_NAME);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "old\n").unwrap();
        git(&repo, ["init"]);

        let written_dir = create_skill_at_repo_root(&repo, Path::new("skills"), true).unwrap();

        assert_eq!(written_dir, skill_dir);
        assert!(
            fs::read_to_string(skill_dir.join("SKILL.md"))
                .unwrap()
                .contains("git-impact-quality-hooks")
        );

        fs::remove_dir_all(repo).unwrap();
    }

    fn temp_test_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("git-impact-agent-test-{id}"))
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

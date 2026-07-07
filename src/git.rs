use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug)]
pub enum DiffRange {
    TwoDot,
    ThreeDot,
}

pub fn changed_files(base: &str, head: &str, range: DiffRange) -> Result<Vec<String>> {
    changed_files_in(std::env::current_dir()?, base, head, range)
}

pub(crate) fn repo_root_in(working_dir: impl AsRef<Path>) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(working_dir)
        .output()
        .with_context(|| "failed to execute git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("not inside a Git repository: {}", stderr.trim());
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        bail!("git rev-parse returned an empty repository root");
    }

    Ok(PathBuf::from(root))
}

pub(crate) fn repo_files_in(working_dir: impl AsRef<Path>) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(working_dir)
        .output()
        .with_context(|| "failed to execute git ls-files")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git ls-files failed: {}", stderr.trim());
    }

    let mut files: Vec<String> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect();
    files.sort();
    files.dedup();

    Ok(files)
}

fn changed_files_in(
    working_dir: impl AsRef<Path>,
    base: &str,
    head: &str,
    range: DiffRange,
) -> Result<Vec<String>> {
    let range = match range {
        DiffRange::TwoDot => format!("{base}..{head}"),
        DiffRange::ThreeDot => format!("{base}...{head}"),
    };

    let output = Command::new("git")
        .args(["diff", "--name-only", &range])
        .current_dir(working_dir)
        .output()
        .with_context(|| "failed to execute git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed for range {range}: {}", stderr.trim());
    }

    let files = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn reads_changed_files_between_commits() {
        let test_dir = temp_test_dir();
        fs::create_dir_all(&test_dir).unwrap();

        git(&test_dir, ["init"]);
        fs::create_dir_all(test_dir.join("modules/datalake/catalog")).unwrap();
        fs::write(
            test_dir.join("modules/datalake/catalog/schema.sql"),
            "create table catalog(id int);\n",
        )
        .unwrap();
        git(&test_dir, ["add", "."]);
        git(
            &test_dir,
            [
                "-c",
                "user.name=git-impact",
                "-c",
                "user.email=git-impact@example.test",
                "commit",
                "-m",
                "initial",
            ],
        );
        let base = git_output(&test_dir, ["rev-parse", "HEAD"]);

        fs::create_dir_all(test_dir.join("dagster")).unwrap();
        fs::write(test_dir.join("dagster/job.py"), "print('deploy')\n").unwrap();
        fs::write(
            test_dir.join("modules/datalake/catalog/schema.sql"),
            "create table catalog(id bigint);\n",
        )
        .unwrap();
        git(&test_dir, ["add", "."]);
        git(
            &test_dir,
            [
                "-c",
                "user.name=git-impact",
                "-c",
                "user.email=git-impact@example.test",
                "commit",
                "-m",
                "change catalog and dagster",
            ],
        );
        let head = git_output(&test_dir, ["rev-parse", "HEAD"]);

        let files = changed_files_in(&test_dir, &base, &head, DiffRange::TwoDot).unwrap();

        assert_eq!(
            files,
            vec![
                "dagster/job.py".to_owned(),
                "modules/datalake/catalog/schema.sql".to_owned()
            ]
        );

        fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn lists_tracked_and_untracked_repo_files() {
        let test_dir = temp_test_dir();
        fs::create_dir_all(&test_dir).unwrap();

        git(&test_dir, ["init"]);
        fs::write(test_dir.join("tracked.txt"), "tracked\n").unwrap();
        git(&test_dir, ["add", "tracked.txt"]);
        fs::write(test_dir.join("untracked.txt"), "untracked\n").unwrap();
        fs::write(test_dir.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(test_dir.join("ignored.txt"), "ignored\n").unwrap();

        let files = repo_files_in(&test_dir).unwrap();

        assert_eq!(
            files,
            vec![
                ".gitignore".to_owned(),
                "tracked.txt".to_owned(),
                "untracked.txt".to_owned()
            ]
        );

        fs::remove_dir_all(test_dir).unwrap();
    }

    fn temp_test_dir() -> std::path::PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("git-impact-test-{id}"))
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

    fn git_output<const N: usize>(working_dir: &Path, args: [&str; N]) -> String {
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

        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
}

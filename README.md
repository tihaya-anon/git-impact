# git-impact

`git-impact` answers: "Tell me what changed in Git, let me expand a dependency graph, and then I'll call whatever command you want."

It is designed for CI pipelines, but also includes a `tree`-style terminal view for humans.

## Config

Create `git-impact.yaml`:

```yaml
nodes:
  node:
    paths:
      - "**"
    command:
      - echo
      - "git-impact: node was impacted"
```

Use `depends_on` when one node should be impacted by another node's changes. Commands run in dependency order.

## Commands

```bash
git-impact init
git-impact prompt
git-impact skills
git-impact validate
git-impact list
git-impact plan --base origin/main --head HEAD
git-impact tree --base origin/main --head HEAD
git-impact run --base origin/main --head HEAD
```

By default, diffing uses Git's three-dot range: `base...head`.

Use `--range two-dot` for `base..head`.

`git-impact init` writes a ready-to-run config at the nearest Git repository root. It creates a safe `echo` command and path patterns from the repository's current files. If the repo has no files yet, it writes a placeholder node with `paths: []` and reminds you to add patterns after adding files.

## AI Agent Automation

`git-impact prompt` prints setup instructions that can be pasted into an AI agent:

```bash
git-impact prompt
```

`git-impact skills` creates a reusable skill folder at `skills/git-impact-automation` under the nearest Git repo root:

```bash
git-impact skills
```

This repo includes a `git-impact.yaml` that agents can use to route Rust source changes to Cargo commands:

```bash
git-impact tree --base origin/main --head HEAD
git-impact run --base origin/main --head HEAD
```

For adding similar automation to other repositories, use the bundled skill at `skills/git-impact-automation/SKILL.md`.

See `examples/git-impact.aiagent.yaml` for Python Ruff, Rust Cargo, and JavaScript package-script patterns. The same graph model can drive Makefile targets, Docker build checks, generated-code/spec tests, deploy commands, and other repo-specific automation.

## CI Install

Install from GitHub Releases on Linux:

```bash
curl -L https://github.com/<owner>/git-impact/releases/download/v0.1.0/git-impact-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz
sudo mv git-impact-x86_64-unknown-linux-gnu /usr/local/bin/git-impact
```

Or compile from a Git tag:

```bash
cargo install --git https://github.com/<owner>/git-impact --tag v0.1.0 --locked
```

Release binaries are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

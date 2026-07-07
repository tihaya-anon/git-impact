---
name: git-impact-quality-hooks
description: Configure and run git-impact as changed-file quality hooks for AI agent workflows. Use when Codex needs to add or maintain git-impact.yaml rules, run only relevant format/lint/test commands after edits, set up Python ruff hooks, Rust cargo hooks, JavaScript package-script hooks, or explain how agents should use git-impact before committing.
---

# Git Impact Quality Hooks

Use `git-impact` to turn Git changes into targeted quality commands. Prefer it when a repo has mixed stacks or expensive checks and the agent should run only commands affected by the files it touched.

## Workflow

1. Check whether `git-impact` is installed:

```bash
git-impact --version
```

2. Ensure the repo has a config:

```bash
git-impact init
```

3. If the repo needs reusable agent instructions, create the skill:

```bash
git-impact skills
```

4. To hand instructions to another agent, print the bootstrap prompt:

```bash
git-impact prompt
```

5. Replace placeholder `echo` commands with real quality hooks. Keep commands deterministic and noninteractive.

6. Validate and inspect the impact plan before running:

```bash
git-impact validate
git-impact tree --base origin/main --head HEAD
git-impact plan --base origin/main --head HEAD
```

7. Run the impacted checks:

```bash
git-impact run --base origin/main --head HEAD
```

Use `--range two-dot` when the workflow wants `base..head` instead of merge-base semantics.

## Hook Patterns

Python with Ruff:

```yaml
nodes:
  python-format:
    paths:
      - "*.py"
      - "**/*.py"
      - pyproject.toml
      - uv.lock
      - requirements*.txt
    command:
      - ruff
      - format
      - .

  python-lint-fix:
    paths:
      - "*.py"
      - "**/*.py"
      - pyproject.toml
      - uv.lock
      - requirements*.txt
    depends_on:
      - python-format
    command:
      - ruff
      - check
      - --fix
      - .
```

Rust:

```yaml
nodes:
  rust-format:
    paths:
      - Cargo.toml
      - Cargo.lock
      - src/**
      - tests/**
    command:
      - cargo
      - fmt

  rust-test:
    paths:
      - Cargo.toml
      - Cargo.lock
      - src/**
      - tests/**
    depends_on:
      - rust-format
    command:
      - cargo
      - test
```

## Agent Rules

- Before committing, run `git-impact plan` or `git-impact tree` to show what will execute.
- Run `git-impact run` when the impacted commands are safe in the current environment.
- If a command is missing locally, report the missing tool and leave the config intact.
- Prefer small nodes with explicit `depends_on` edges over one shell command that chains unrelated checks.
- Use argv arrays only; do not rely on shell features like pipes or redirection in `command`.

---
name: git-impact-automation
description: Configure and run git-impact as changed-file automation for AI agent workflows. Use when Codex needs to add or maintain git-impact.yaml rules; route file changes to Makefile targets, Docker build or layer checks, generated-code/spec tests, format/lint/test commands, deploy commands, or explain how agents should use git-impact before handing work back.
---

# Git Impact Automation

Use `git-impact` to turn Git changes into targeted commands. Prefer it when a repo has mixed stacks, expensive commands, generated artifacts, Docker images, Makefile targets, or downstream systems that should only run when affected files changed.

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

5. Replace placeholder `echo` commands with real repo automation. Keep commands deterministic and noninteractive.

6. Validate and inspect the impact plan before running:

```bash
git-impact validate
git-impact tree --base origin/main --head HEAD
git-impact plan --base origin/main --head HEAD
```

7. Run the impacted commands:

```bash
git-impact run --base origin/main --head HEAD
```

Use `--range two-dot` when the workflow wants `base..head` instead of merge-base semantics.

## Automation Patterns

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

Makefile target:

```yaml
nodes:
  make-tests:
    paths:
      - Makefile
      - src/**
      - tests/**
    command:
      - make
      - test
```

Docker context:

```yaml
nodes:
  docker-build-check:
    paths:
      - Dockerfile
      - docker/**
      - services/api/**
    command:
      - make
      - docker-build-check
```

Spec or generated-code tests:

```yaml
nodes:
  api-contract:
    paths:
      - openapi/**
      - proto/**
      - schemas/**
    command:
      - make
      - contract-test
```

## Agent Rules

- Before finishing, run `git-impact plan` or `git-impact tree` to show what will execute.
- Run `git-impact run` when the impacted commands are appropriate in the current environment.
- If a command is missing locally, report the missing tool and leave the config intact.
- Prefer small nodes with explicit `depends_on` edges over one shell command that chains unrelated automation.
- Use argv arrays only; do not rely on shell features like pipes or redirection in `command`.

# AGENTS.md

Use `git-impact` to route Git changes to the repo commands they impact.

- Inspect impacted commands with `git-impact tree --base origin/main --head HEAD`.
- Run impacted commands with `git-impact run --base origin/main --head HEAD` when they are appropriate and available.
- Use `--range two-dot` for workflows that compare `base..head`.
- Keep `git-impact.yaml` commands noninteractive and expressed as argv arrays.
- For adding automation to another repo, use the bundled skill at `skills/git-impact-automation/SKILL.md`.
- Use `git-impact prompt` to print agent bootstrap instructions.
- Use `git-impact skills` to create a reusable `git-impact-automation` skill in another repo.

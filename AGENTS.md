# AGENTS.md

Use `git-impact` as the repo quality gate after code edits.

- Inspect impacted hooks with `git-impact tree --base origin/main --head HEAD`.
- Run impacted hooks with `git-impact run --base origin/main --head HEAD` when the commands are available.
- Use `--range two-dot` for workflows that compare `base..head`.
- Keep `git-impact.yaml` commands noninteractive and expressed as argv arrays.
- For adding hooks to another repo, use the bundled skill at `skills/git-impact-quality-hooks/SKILL.md`.

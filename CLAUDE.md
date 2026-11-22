@AGENTS.md

## Claude Code only

- Use the test-runner subagent (`.claude/agents/test-runner.md`) for the `full` and `milestone`
  tiers, multi-backend, MPI and remote runs, so logs stay out of the main context.
- `.claude/settings.json` sets `worktree.baseRef` to `head`, so subagent worktrees start from the
  current commit; create other worktrees with `git worktree add … main`.

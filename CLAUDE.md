@AGENTS.md

## Claude Code only

- Use the test-runner subagent (`.claude/agents/test-runner.md`) for the `full` and `milestone`
  tiers, multi-backend, MPI and remote runs, so logs stay out of the main context.
- Reviewers of the review skill: start each brief as its own delegated task in T3 Code
  (`delegate_task`, provider `claudeAgent`, model `claude-fable-5-2`, `effort: high`,
  isolation `worktree`), one task per reviewer and round, with the brief, the commit, the base
  branch and the pull request description as a file. If `delegate_task` is unavailable, use a
  fresh read-only Claude subagent (Agent tool, model `fable`, effort `high`).
- `.claude/settings.json` sets `worktree.baseRef` to `head`, so subagent worktrees start from the
  current commit; create other worktrees with `git worktree add … main`.

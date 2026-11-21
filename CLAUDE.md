@AGENTS.md

## Claude Code only

- Use the test-runner subagent (`.claude/agents/test-runner.md`) for the `full` and `milestone`
  tiers, multi-backend, MPI and remote runs, so logs stay out of the main context.
- Reviewers of the review skill: start each brief as its own delegated task in T3 Code
  (`delegate_task`, provider `antigravity`, model `gemini-4-neon`, `thought_level: high`,
  isolation `worktree`), one task per reviewer and round, with the brief, the commit, the base
  branch and the pull request description as a file. If Antigravity is unavailable, use a fresh
  read-only Claude subagent and say in the review comment that the review was not independent.
- Delegated work (subagents): include the task text, the design sections it depends on and
  this rule: "If the design is ambiguous or an API must change, stop and return an ADR draft
  instead of deviating." Subagents cannot ask the maintainer questions.
- `.claude/settings.json` sets `worktree.baseRef` to `head`, so subagent worktrees start from the
  current commit; create other worktrees with `git worktree add … main`.

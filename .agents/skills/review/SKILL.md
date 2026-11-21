---
name: review
description: Review one pull request with independent read-only reviewers (task mode), or run the gate for a finished milestone (gate mode). Use when asked for a review or a gate, or from implement-task.
---

# Review

Two modes. Say which one you are running.

## Rules for both

- A reviewer gets only its brief from `references/`, the repository, the
  commit to review, the base branch, and the pull request description as a
  file. Never your reasoning or another reviewer's report.
- Reviewers are read-only and need no network. They read the change with
  `git diff <base>...<commit>` and `git show <commit>:<path>`.
- Start each reviewer in a fresh context: a read-only subagent if you have
  one, otherwise ask the maintainer to run the brief in a new session and
  paste the report.
- Use the strongest model you can start, from another vendor than the one
  that wrote the code if you can. Say which model reviewed, and say so if
  it was not independent.
- Every report ends with what the reviewer read in full. If that is not
  every changed file, run it again.
- **A report is a claim, not a result.** Check each finding against the
  code or the mathematics before you record it. A finding that does not
  hold is recorded as withdrawn, with the reason.

## Task mode

For one pull request, after the draft is open.

1. Run `references/adversarial.md` and `references/fidelity.md` at the pull
   request's current commit. Give the fidelity reviewer the path of the
   task's plan section.
2. Check each finding. Post one review comment
   (`gh pr review <n> --comment --body-file <file>`) headed with the round,
   the reviewer model and the commit, and one line per finding: ID,
   severity (blocking, should-fix, note), location, what it says, status
   (confirmed, withdrawn with reason, fixed in <commit>).
3. Fix confirmed blocking and should-fix findings, push, and post the
   updated statuses.
4. If anything blocking was fixed, run one second round at the new commit.
5. If a blocking finding is still open after the second round, say so and
   stop. There is no third round.

## Gate mode

For a milestone whose tasks are all closed, from a clean checkout of the
main branch.

1. **Check** that every task issue of the milestone is closed and the main
   branch's checks pass.
2. **Evidence.** Run the `milestone` check from `AGENTS.md` once. Record
   what ran and where.
3. **Acceptance.** For every row of the plan's acceptance table, name the
   passing test. No test may still be ignored with a `pending` marker of
   this milestone.
4. **Two independent passes** over the milestone's merged changes:
   `references/gate-record.md` and `references/gate-architecture.md`.
5. **Check** every finding of both passes.
6. **Record** `docs/reviews/<milestone>.md`: the findings with IDs, severity
   and status; the evidence; what each pass read in full, and what nobody
   read. Open a pull request with it.
7. **Triage with the maintainer.** For each confirmed finding ask: fix now,
   open an issue, draft an ADR, or accept with a reason. Carry out the
   answers and record them in the maintainer's words.
8. The maintainer closes the milestone issue. You never do.

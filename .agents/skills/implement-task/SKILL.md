---
name: implement-task
description: Take one task issue to a reviewed pull request, tests first. Use when the maintainer says to do the next task, a named task, or to continue.
---

# Implement one task

## 1. Find the task

- **A task was named:** use it.
- **"Continue":** if this checkout is on a `task/` branch, that is the task.
  Read the comments on its issue first (`gh issue view <n> --comments`):
  an earlier session may have left notes on what is done and what failed.
  Then read its pull request (`gh pr view --json state,isDraft,comments,reviews`)
  and carry on from there. If the pull request was closed without merging,
  stop and ask what the closure means.
- **"The next task":** list the open tasks,
  `gh issue list --label task --state open --json number,title,labels,blockedBy`,
  and take the first, in milestone and task order, that is not labelled
  `in-progress` and whose blocking issues are all closed. Say which one and
  why. If none qualifies, say what blocks each and stop. If every task of a
  milestone is closed, say that its gate can start.

## 2. Start

1. Check again that the issue is open, not `in-progress`, and unblocked.
2. `gh issue edit <n> --add-label in-progress --add-assignee @me`.
3. Update the main branch and create `task/<n>-<short-name>` from it.
4. Read the task's section in `docs/plans/<milestone>.md`, the design
   sections the plan names, the Conventions, the Lessons in `AGENTS.md`,
   the files under `Touches`, and any comments on the issue.

## 3. Tests first

- Task T0: add every acceptance test of the plan as an ignored test marked
  `pending <milestone>.T<k>`, against stubs.
- Any other task: remove the ignore markers that name this task. Run the
  tests: they must fail, and for the right reason.

## 4. Implement

Small commits; the `quick` check from `AGENTS.md` before each. Follow the
Conventions. Rules 3, 4 and 5 of `AGENTS.md` apply here above all: do not
change your own checks, do not settle what the design leaves open, and stop
for a decision when the code cannot do what the design states.

## 5. Pull request

1. Run the `full` check. Fix until it passes.
2. Push, and open a draft pull request, `gh pr create --draft`, titled
   `<task id>: <what changed>`, with these sections in its description:
   `Closes #<issue>`; acceptance rows covered; checks run; deviations from
   the plan; anything the maintainer must look at.
3. Unless the plan marks the task trivial, run the review skill in task
   mode. At most two rounds.
4. When checks and review are complete: `gh pr ready <n>`, and tell the
   maintainer it is ready, with what to look at first.

The maintainer merges, or tells you to (see Merging in `AGENTS.md`). The
merge closes the issue.

## Stop and ask when

| Situation | Do |
| --- | --- |
| The code cannot follow the design, or the design leaves open what you need | Decision skill; leave the pull request a draft |
| A protected path would change behaviour | Stop and ask before changing it |
| The task grows past about 600 changed lines | Propose a split |
| A check tier in `AGENTS.md` is not filled in, and the plan does not give this task that job | Stop and ask |
| Two review rounds leave a blocking finding | Say so, and stop |
| The pull request was closed without merging | Ask: rework, start over, or drop the task |

## When you stop before the task is finished

Whatever the reason, a question for the maintainer, the end of the session,
something you cannot solve, leave a comment on the task's issue before you
stop:

- what is done, and up to which commit;
- what is left;
- what you tried that did not work, and why;
- what you would do next.

A few lines. No logs, no host names, no paths outside the repository. If
you learned something that every later session needs, propose it as a line
under Lessons in `AGENTS.md`, in a small pull request of its own.

## Starting over, or dropping a task

Only when the maintainer says so.

- **Start over:** close the task's pull request if it is open, remove the
  `in-progress` label and the assignee, and leave the old branch in place.
  The task is then free to be done again, on a new branch.
- **Drop:** do the same, then change the plan in a pull request: remove the
  task, and move its acceptance rows and whatever needed it to other tasks.
  After the merge, bring the issues in line with the plan as the
  plan-milestone skill describes, and ask the maintainer to close the
  dropped task's issue. Never close it yourself, and never just close an
  issue to unblock others.

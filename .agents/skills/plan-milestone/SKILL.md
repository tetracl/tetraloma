---
name: plan-milestone
description: Turn one roadmap milestone into a plan with tasks and an acceptance table, and create its GitHub issues. Use only when the maintainer asks to plan a named milestone.
---

# Plan a milestone

The milestone is the one the maintainer named; if none, ask.

## Preconditions

- The milestone is in `docs/roadmap.md` and has no plan in `docs/plans/`.
- Every milestone it depends on is closed on GitHub:
  `gh issue list --label milestone --state all --json number,title,state`.

If one fails, say which and stop.

## Write the plan

1. Read the roadmap entry, the design sections it lists, the Conventions
   and Protected paths, and the ADRs that touch them.
2. On a branch `plan/<milestone>`, write `docs/plans/<milestone>.md` from
   `references/plan-template.md`. `references/example-plan.md` shows one.
   - **Acceptance:** one row per roadmap bullet, each with exactly one test
     name and the task that enables it.
   - **T0:** all acceptance tests as ignored tests against stubs of the
     public interface, each marked `pending <milestone>.T<k>`. If the
     repository has no build setup yet, say in T0 that it also adds the
     minimal setup and proposes the check commands for `AGENTS.md`.
     Otherwise no task may touch the checks.
   - **Tasks:** in dependency order, each about 600 changed lines at most.
     Split anything larger. Give each `Needs`, `Touches`, `Size`,
     `Protected paths`, `Sign-off needed`.
   - **Open questions:** only what the maintainer must answer.
3. No status fields and no checkboxes: state lives on GitHub.
4. Open a pull request titled `Plan <milestone>`. Tell the maintainer it is
   ready to read, and list the open questions. The merge is the approval.

## Create the issues

When the maintainer asks for it, after the plan is merged. If the plan is
not on the main branch yet, say so and stop. First list what exists, so that nothing is created
twice: `gh issue list --state all --search "<milestone> in:title" --json number,title`.

1. Labels, once per repository:
   `gh label create milestone --force`, and the same for `task`,
   `in-progress` and `decision`.
2. The milestone issue:
   `gh issue create --title "<milestone>: <title>" --label milestone --body-file <file>`
   with the plan's path, the design sections and the number of acceptance
   criteria.
3. One issue per task, in plan order, as sub-issues:
   `gh issue create --title "<milestone>.T<k>: <title>" --label task --parent <milestone issue> --body-file <file>`
   with the goal, the acceptance rows it enables, and a link to its plan
   section.
4. Dependencies, from each task's `Needs`:
   `gh issue edit <task> --add-blocked-by <issue>,<issue>`.
5. Read them back and compare with the plan:
   `gh issue list --label task --state open --json number,title,parent,blockedBy`.
   Report the list to the maintainer.

If the plan changes later, change the issues to match in the same way: add
what is new, edit titles, bodies and links. Never close or reopen an issue
to make the list fit.

## Stop and ask when

- a design section is missing, or contradicts an ADR;
- an acceptance bullet cannot be decided by one automated test;
- the milestone needs more than about eight tasks: propose a split.

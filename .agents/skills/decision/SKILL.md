---
name: decision
description: Put a question to the maintainer and record the answer, as a short decision for a choice the design allows, or as an ADR when the design itself must change. Use when a task must stop for a decision, or when the maintainer asks for an ADR.
---

# Decision

You never decide these yourself, and you never edit the design outside an
ADR pull request.

| The question | Use |
| --- | --- |
| A choice among options the design already allows (a tolerance, a machine, an ordering) | Short decision |
| Anything that changes the design, a convention, a public interface or an acceptance criterion; or a feature the design does not describe | ADR |

## Short decision

1. Ask the maintainer directly: what is undecided, the options, what waits
   for it, and your recommendation with one reason.
2. If the maintainer is not there to answer, create an issue with the same
   text (`gh issue create --label decision --title "Decision: <question>"`),
   mark the task as blocked by it (`gh issue edit <task> --add-blocked-by <n>`),
   and stop.
3. Quote the answer word for word, with its date, into the Decisions table
   of the plan, in the task's pull request.

## ADR

1. Finish or commit what you have on the task branch, and leave its pull
   request a draft.
2. Take the next free number in `docs/adr/`. On a branch
   `adr/<NNNN>-<short-name>` from the main branch, write
   `docs/adr/<NNNN>-<short-name>.md` from `references/adr-template.md`,
   status Proposed. It must have: the evidence that the design does not
   fit; at least two options, one of them "keep the design as written"; and
   under "Changes if accepted" the exact design sections, interfaces, tests,
   tasks and milestones.
3. Open a pull request titled `ADR-<NNNN>: <title>`. Mark every affected
   task as blocked by it: create a `decision` issue that the pull request
   closes, and `gh issue edit <task> --add-blocked-by <decision issue>`.
4. Tell the maintainer what the options are and what you recommend, and
   wait.
5. **Record the answer.** Copy the maintainer's sentence word for word
   under Decision, with the date and where it was given: "comment on pull
   request #<n>" or "said to the agent in a session". Record exactly as
   much as was said: a conditional answer stays conditional. Set the status
   to Accepted or Rejected. Never post the answer in the maintainer's name.
6. **Apply, if accepted,** in a second commit on the same pull request:
   exactly the design edits listed, marked `(ADR-<NNNN>)`, the roadmap and
   the plans affected, and a line in `docs/adr/README.md`. Code and tests
   stay in the task's pull request.
7. The maintainer merges it, or tells you to; a rejected ADR is merged too,
   for the record. The merge is what makes the decision binding. Then
   remove the blocking links and carry on with the task.

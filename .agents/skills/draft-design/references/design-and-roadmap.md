# Structure rules for design.md and roadmap.md

Agents read the design to learn what to build and the roadmap to learn in
what order. The workflow's skills rely on the properties below. Everything
else about the two files is free.

## docs/design.md

| Rule | Why the workflow needs it |
| --- | --- |
| Sections are headed `## <number>. <title>`. A number never changes and is never reused. New material gets the next free number. | Plans, ADRs and issues cite "design section 5". Renumbering breaks every citation. |
| Every section that describes something to build has four parts, headed `### Purpose`, `### Interface`, `### Behaviour and invariants`, `### Out of scope`. | The planner turns Interface and Behaviour into tasks and tests, and Out of scope into limits on the worker. |
| One Conventions section holds what applies everywhere: types, storage order, error handling, tolerances, units. | Workers follow it without being told per task. Reviewers check against it. |
| One Protected paths section lists each path, what must not change, and the check that guards it. | The checks include it. A change of behaviour there stops the worker. |
| One Open questions section says what is undecided. | An agent that meets an undecided point stops and asks. It does not guess. |
| A Glossary fixes terms, symbols and reference cells. | Different agents and models use the same names. |
| Text changed by a decision carries an inline marker such as `(ADR-0003)`. | Readers see why the text is as it is. |
| No status words, dates, task lists or checkboxes. | State lives on GitHub. |
| Every limit is a number. | Tests and reviewers need the number. |

## docs/roadmap.md

Each milestone is one entry:

    ## M2: Rules on the triangle

    Depends on: M1
    Design: 3, 5
    Acceptance:
    - For degree = 1 to 30, the rule integrates every monomial x^i y^j with
      i + j <= degree with relative error at most 1e-13.
    - Weights are positive and sum to 1/2 with absolute error at most 1e-14.
    Out of scope: symmetric rules with fewer points (design section 9).
    Runs: none

| Rule | Why the workflow needs it |
| --- | --- |
| The heading is `## M<n>: <title>`. An ID never changes and is never reused. | Plan file names, issue markers and task IDs are built from it. |
| The fields Depends on, Design, Acceptance, Out of scope, Runs appear in that order. | Skills and the linter read them by name. |
| Depends on lists milestone IDs, or `none`. | A milestone is planned only when those it depends on are closed. |
| Design lists the section numbers the milestone implements. | The planner reads exactly those sections. |
| Each acceptance bullet is one sentence that one automated test can decide, with its range and tolerance stated. | The first task turns every bullet into exactly one test. The gate checks every bullet against a passing test. |
| Out of scope names what the milestone leaves out. | It keeps a worker from adding it. |
| Runs names the machines or long runs the milestone needs, or `none`. | They are planned once, at the gate. |
| A milestone is roughly four to eight tasks. A larger one is split. | The gate stays reviewable. |
| No status, checkboxes or dates. | State lives on GitHub. |

## Common mistakes

Acceptance bullets:

| Does not work | Why | Works |
| --- | --- | --- |
| "The triangle rules are accurate." | No test can decide it. | "For degree = 1 to 30, the rule integrates every monomial of total degree <= degree with relative error at most 1e-13." |
| "Implement Golub-Welsch." | It describes work, not a result. Work belongs in a plan's tasks. | "Points and weights agree with the reference tables to relative error at most 1e-13." |
| "Rules are exact and weights are positive and points are interior." | Three criteria, one test, three reasons to fail. | Three bullets. |
| "Performance is good." | No number and no machine. | "triangle(20) takes at most 1.2 times the stored baseline on the benchmark machine." |
| "The interface is easy to use." | It needs human judgement. | Move it to the Conventions, where a reviewer checks it. |
| "Works for all supported inputs." | The range is not stated. | State the range, and add a bullet for what happens outside it. |

Design sections:

| Does not work | Why | Works |
| --- | --- | --- |
| A building section without an Interface part | Workers invent signatures, and two workers invent different ones. | State every public signature, even if provisional. |
| "We will probably use collapsed coordinates." | An agent cannot tell a decision from a thought. | Decide it, or list it under Open questions. |
| "Done", "TODO" or dates in the text | They go stale and disagree with GitHub. | Leave state out. |
| Renumbering after inserting a section | Citations now point at the wrong text. | Give new material the next free number. |
| "Moderately large n" | No number. | "1 <= n <= 30". |
| The same rule repeated in several sections | Copies drift apart. | One Conventions section, cited by number. |

Roadmap entries:

| Does not work | Why | Works |
| --- | --- | --- |
| "Depends on: the recurrence code" | Milestones depend on milestones. Finer dependencies belong to a plan's tasks. | "Depends on: M1" |
| One milestone for the whole project | The gate cannot be reviewed in one sitting. | Several milestones of four to eight tasks. |
| A milestone without an acceptance bullet | There is nothing to test and nothing for the gate to check. | At least one bullet. If none can be stated, the design is not ready. |

## Changing the files later

After the first merge, the design changes only through an ADR pull request.
The roadmap gains a milestone through an ADR, or through a small pull request
when the design already describes the work.

---
name: draft-design
description: Draft docs/design.md and docs/roadmap.md in the structure the workflow needs, by asking the maintainer what cannot be looked up. Use when the maintainer asks to draft, write or restructure the design or the roadmap.
---

# Draft the design and the roadmap

You write the draft. The maintainer decides what it says.

## Before writing

1. Read `references/design-and-roadmap.md`: the structure rules.
2. Read `references/example-design.md` and `references/example-roadmap.md`:
   the rules applied to a small library.
3. Read what the repository already has. Do not ask for what you can find.

## Ask

Ask what you could not find, three to five questions at a time:

- What is the project for, and who uses it?
- Which public interfaces should exist? What do they take and return?
- Which ranges, sizes and accuracies must be supported? Ask for numbers.
- What applies everywhere: types, storage order, error handling, tolerances?
- What must never change once written, and what checks it?
- What is undecided?

Never fill a gap with a guess. What the maintainer leaves open goes under
Open questions, in their words.

## Write

1. `docs/design.md`: numbered sections. Every section that describes
   something to build has the parts Purpose, Interface, Behaviour and
   invariants, Out of scope. Every limit is a number.
2. `docs/roadmap.md`: milestones of roughly four to eight tasks, each with
   the fields Depends on, Design, Acceptance, Out of scope, Runs, in that
   order. Each acceptance bullet is one sentence that one automated test can
   decide, quoting the numbers from the design.
3. Nothing about progress: no status, no dates, no checkboxes.

## Check your draft

- Does every building section have all four parts, with signatures?
- Does every roadmap entry have all five fields, in order?
- Does every section number the roadmap cites exist?
- Could one test decide each acceptance bullet?
- Is everything undecided listed, and nothing stated that was not decided?

## Hand over

Open a pull request with both files, or for a repository without history
leave them uncommitted. Tell the maintainer what you assumed, what is under
Open questions, and which acceptance bullets you are least sure about.
After the first merge, the design changes only through an ADR.

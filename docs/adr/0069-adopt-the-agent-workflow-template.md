# ADR-0069: Adopt the agent workflow template

- **Status:** Accepted
- **Date:** 2026-11-21
- **Raised in:** between milestones (E3 merged; no milestone open), by the maintainer
- **Design sections affected:** the title block and "Summary" (Audience, Status); every `##`
  heading (numbers added); "Implementation roadmap" (moved to `docs/roadmap.md`); "Handing this
  design to Claude Code" with Listing H1 (retired); "Change log from the October 2026 draft"
  (introduction); four new sections: Conventions, Protected paths, Open questions, Glossary

## Context

tetraloma has been built with a Claude Code workflow made of `CLAUDE.md` (155 lines: the working
agreement of Listing H1 plus workflow and test-target rules), six skills in `.claude/skills`, two
subagents in `.claude/agents` and `IMPLEMENTATION.md`: one branch, plan and pull request per
milestone, one commit per task, plan status lines and checkboxes, a design review before the
milestone pull request.

The maintainer wants to move to the workflow of `github.com/tetracl/agent-workflow-template`
(commit 489f843; `WORKFLOW.md` there). It is instructions only: an `AGENTS.md` (rules, check tiers,
protected paths, lessons), five skills in `.agents/skills` (draft-design, plan-milestone,
implement-task, review, decision) with links from `.claude/skills`, and a one-line `CLAUDE.md` that
imports `AGENTS.md`. One task is one GitHub issue, one branch and one pull request, tests first;
milestone and task issues hold all state; every pull request is reviewed by fresh read-only
reviewers; a finished milestone gets a gate review; the maintainer merges.

Its skills rely on structure rules for the design and the roadmap
(`.agents/skills/draft-design/references/design-and-roadmap.md`): numbered `##` sections, a
Conventions section, a Protected paths section, an Open questions section, a Glossary, no status
words or dates, and a separate `docs/roadmap.md` whose entries have the fields Depends on, Design,
Acceptance, Out of scope and Runs.

The maintainer settled the following in the session of 2026-11-21, answering the transition plan
(quoted as given):

1. Milestone ids: "Use only M11 and up from now on."
2. Restructuring the design: "Do a minimal restructure".
3. Where the rules of `CLAUDE.md` go (code conventions into the design, operating rules into
   `AGENTS.md`): "As proposed".
4. The milestone check tier: "Run the Milestone tier only once at the gate".
5. Reviewers: "Use another vendor. Antigravity Neon on high effort is a good reviewer."
6. Checking the migration: "Traceability and independent reviewer is enough".

## Problem

1. **The design does not meet the template's structure rules.** Its sections are cited by name,
   not number; the roadmap is a section of the design (lines 1882–2207 at commit 04064fd), its
   entries have Scope and Acceptance but not the five fields; there is no Conventions, Protected
   paths, Open questions or Glossary section; the title block and Summary carry a date and a status
   ("As of 17 October 2026", "Status: revised design, 17 October 2026").
2. **The code conventions live outside the design, in two copies that have drifted.** The kernel,
   SIMD, precision, crate-boundary, host↔device copy, FFI and user-documentation rules are in
   Listing H1 and in `CLAUDE.md`. ADRs amend them (ADR-0026, 0028, 0039, 0040, 0042, 0049, 0059,
   0067, 0068), but the two copies already differ: `CLAUDE.md` allows the debug-build index checks
   of "gather, scatter, pack and unpack" (M10 review, commit 08a8f31) where Listing H1 still says
   "gather and scatter", and `CLAUDE.md`'s user-documentation bullet names `docs/README.md` where
   Listing H1 does not. Under the template, `CLAUDE.md` becomes one line, so the rules need a home;
   the template's reviewers read the design's Conventions section and `AGENTS.md`.
3. **"Handing this design to Claude Code" describes the old workflow** ("run one milestone per
   session", "add the working agreement below as `CLAUDE.md`") and would contradict `AGENTS.md`.
4. **Section citations must survive.** Sixty-eight ADRs, 22 plans and about 15 source files cite
   design sections by name ("Partitioning", "Listing S3", "Benchmarks", "A.6"). Renaming or
   splitting sections would break them.

## Options

1. **Keep the design as written.** The template's skills then read a design without the sections
   they expect: no Conventions for reviewers to check against, a roadmap whose entries lack the
   fields the planner reads, and Listing H1 stating a different workflow. The code rules would stay
   only in `CLAUDE.md`, which becomes one line, or in a second copy in `AGENTS.md` outside ADR
   control. Not proposed.
2. **Minimal restructure.** Number the existing headings without renaming them; move the roadmap
   verbatim into `docs/roadmap.md`; retire Listing H1; add Conventions (the code rules of
   `CLAUDE.md`, verbatim), Protected paths, Open questions and Glossary. The four-part layout
   (Purpose, Interface, Behaviour and invariants, Out of scope) is not applied now; a section gets
   it through its own ADR when an M11+ roadmap entry first names it. Citations by name keep
   working. The change is mostly moved text and can be checked in one pass.
3. **Full restructure.** In addition to option 2, rewrite about twelve building sections into the
   four parts now. About 2,000 lines change; the Out of scope parts need new design decisions;
   text moves between sections ("Architecture" and "API walkthrough" describe the mesh in several
   places), so citations by name would point at moved text; checking that nothing normative
   changed needs a line-by-line review. The maintainer chose against it.

## Proposed decision

Option 2, as the maintainer chose. The design keeps its text and its section names; it gains
numbers, the four sections the template's skills read, and loses the two parts that describe the
old workflow (the roadmap moves out unchanged; Listing H1 is replaced by the Conventions section
and `AGENTS.md`). The rest of the transition (`AGENTS.md`, the skills, the removal of the old
skills, the design-reviewer agent and `IMPLEMENTATION.md`, the ADR index, the traceability table)
changes no design text and follows in a separate pull request after this ADR is merged.

## Consequences

- **Text to change in docs/design.md:**
    - **Title block:** "As of 17 October 2026. Markdown export of the revised design document; …"
      becomes "The design reference for implementation. It changes only through ADRs;
      `docs/adr/README.md` lists them."
    - **1. Summary:** Audience becomes "library architects and the coding agents that implement
      it"; the Status bullet (revision history) moves to the introduction of section 21.
    - **Numbers** on the remaining `##` headings, in file order, titles unchanged:
      1 Summary, 2 Requirements and decisions, 3 Design rationale, 4 Architecture, 5 API
      walkthrough, 6 Reference cells and conventions, 7 Device model and backends,
      8 Vectorization and CPU execution, 9 Topology construction on the device, 10 Geometry,
      11 Distributed mesh model, 12 Partitioning, 13 Import and export, 14 User documentation,
      15 Tags, regions and submeshes, 16 Testing and validation, 17 Development environment and
      test targets, 18 Workspace layout, 19 Risks and mitigations, 20 Appendix A — Reference cells
      and element tables (A.1–A.11 keep their numbers), 21 Change log from the October 2026 draft.
      `###` headings are not numbered. Numbers are never reused or changed.
    - **"Implementation roadmap"** is removed from the design and moved verbatim to
      `docs/roadmap.md`, under a heading "Milestones before ADR-0069" (E0–E2, M0–M10c, D1; E3 had
      no roadmap entry, its plan is `docs/plans/E3.md`). The roadmap's introduction states the
      template's rules for new entries: heading `## M<n>: <title>` with n ≥ 11; the fields Depends
      on, Design (section numbers), Acceptance (one sentence per bullet, decidable by one automated
      test, with range and tolerance), Out of scope, Runs, in that order; Depends on lists only
      M11+ ids or `none`, since every earlier milestone is merged; no status, checkboxes or dates.
      "the roadmap" in section 16 then means `docs/roadmap.md`; the change-log rows that cite
      "Implementation roadmap" or "Roadmap" (MAJ-20, MIN-19) stay as history.
    - **"Handing this design to Claude Code"** and Listing H1 are removed. Section 21's
      introduction notes that Listing H1 was retired by ADR-0069; the change-log rows that cite it
      (BLK-1, MIN-9) stay as history.
    - **22. Conventions (new):** the code rules of `CLAUDE.md` at commit 04064fd, verbatim apart
      from list layout, with their ADR references: Toolchain; CubeCL (pin, runtime types, runtime
      features); Kernels; SIMD (including "vector sizes come from VectorPolicy … changed only with
      benchmark data" and the four generic sets of ADR-0058); Precision; Host maths; Orientation
      (origin ids); the layout of the MPI tests (crate tetraloma-mpi-tests, `[[test]]` targets with
      `harness = false`, the world-size check of ADR-0068); the ndelement dependency rule (only in
      `conformance/`, never `version =`); the allowed host↔device copies (the full list, with the
      `CLAUDE.md` wording "gather, scatter, pack and unpack"); FFI and unsafe; user documentation
      (ADR-0067). Where Listing H1 and `CLAUDE.md` differ, the `CLAUDE.md` wording is taken.
    - **23. Protected paths (new):** each path, what must not change, and its check:

      | Path | What must not change | Check |
      | --- | --- | --- |
      | `tetraloma-device/vector-policy.toml`, `tetraloma-geometry/degree-policy.toml` | the four generic sets; entries change only with benchmark records in `docs/perf/` (ADR-0058) | `cargo test -p tetraloma-device --test integration policy::`; `cargo test -p tetraloma-bench --test perf_record` |
      | `tetraloma-refcell/tests/fixtures/`, the ndelement pin in `conformance/Cargo.toml` | the reference-cell conventions exported from ndelement | `cargo test -p tetraloma-refcell --test integration fixtures::`; `cargo test --manifest-path conformance/Cargo.toml` |
      | `tetraloma-io/tests/fixtures/` | the committed Gmsh and VTK corpus written by the pinned tools | `cargo test -p tetraloma-io --test integration gmsh_corpus::`; `cargo test -p tetraloma-io --test integration vtk_fixtures::` |
      | `rust-toolchain.toml`, the `cubecl` pin in `Cargo.toml` | Rust 1.99; `cubecl = "=0.11.0"`; a default build free of ndelement, BLAS and MPI | `scripts/check-workspace-deps.sh` |
      | `scripts/mpi-test.sh`, `scripts/run-examples.sh` | every MPI run checks the world size (ADR-0068) | `scripts/check-rank-guard.sh` |
      | names in tracked paths and content | the project name and its allowlist (ADR-0066) | `scripts/check-name.sh` |

      `AGENTS.md`'s Protected paths table points to this section instead of copying it.
    - **24. Open questions (new):** "None." Later ADRs and plans add to it.
    - **25. Glossary (new):** the terms and symbols of the user book's glossary
      (`docs/user/src/glossary.md`) apply; the section adds the terms used only in the design and
      the agent rules, each pointing to the section that defines it: standing matrix (defined here,
      from `CLAUDE.md`'s run-plan rule: the CPU runtime and Metal on every change, Locust after
      kernel tasks, MPI on 1–4 ranks under both flavours, f32 and f64, scalar and widest vector
      sizes, the review targets; ADR-0057), candidate protocol (8, "Benchmarks"), benchmark
      platform (8 and 17), Cpu and Gpu device classes (8; ADR-0058).
- **Public API changes:** none.
- **Acceptance tests affected:** none. Every acceptance bullet of the moved roadmap is unchanged.
- **Later milestones affected:** none exist yet. New milestones start at M11, are planned in
  `docs/plans/M<n>.md` from the template's plan skill, and are tracked as a milestone issue
  (label `milestone`) with one sub-issue per task (label `task`).
- **Outside the design (the follow-up pull request, no ADR needed):** `AGENTS.md` (operating
  rules from `CLAUDE.md` as rule 11; check tiers `quick` = the verify skill, `full` = the CPU
  runtime and Metal plus Locust when kernels change and MPI on 1–4 ranks under both flavours when
  tetraloma-parallel or a later crate changes, `milestone` = the standing matrix, run once at the
  gate; reviewers from another vendor, Antigravity Gemini 4 Neon at high effort; lessons from the
  agents' memory); `.agents/skills` with links in `.claude/skills`; `hpc-test` and `verify` kept as
  project skills; the test-runner agent kept; the old skills, the design-reviewer agent and
  `IMPLEMENTATION.md` removed after their content is moved; `docs/adr/README.md` becomes the
  index table of ADR-0001 onwards; `docs/plans/TEMPLATE.md` and `docs/adr/TEMPLATE.md` are replaced
  by the skills' templates; references to `CLAUDE.md` in code comments, scripts and docs point to
  the rule's new home; `docs/workflow-migration.md` maps every rule of the old files to its new
  place, and an independent reviewer checks it. Existing plans, reviews in plans, ADRs and closed
  issues are kept unchanged as history.

## Decision record

Accepted by the maintainer on 2026-11-21, said to the agent in the session: "Accept ADR 69".

Applied in commit `adr: 0069 apply` as listed above, with three details of wording: the
inline marker (ADR-0069) stands in the first sentence of sections 22–25, not in their headings;
section 24 opens with one sentence saying what it is for; and in the revision history moved into
section 21, "The last section maps every review finding" reads "The table below maps every
review finding". The check commands of section 23 name the `integration` test targets of
tetraloma-device, tetraloma-refcell and tetraloma-io with a module filter (these crates bundle
their tests into one target); all of them pass on the CPU runtime.

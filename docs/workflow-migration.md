# Workflow migration (ADR-0069)

tetraloma moved from its Claude Code workflow (`CLAUDE.md`, six skills, two subagents,
`IMPLEMENTATION.md`) to the agent workflow template (`AGENTS.md`, `.agents/skills`; template
commit 489f843). This table maps every rule of the old files to its new place, or says why it was
dropped. The old files are at the tag `pre-agent-workflow`.

"Superseded" means the template states the same rule, or a stricter one, in the place named.

## CLAUDE.md: working agreement

| Old rule | New place |
| --- | --- |
| Design reference; public types in "Architecture" and "API walkthrough" change only via an ADR | Superseded: `AGENTS.md` rules 4 and 5; decision skill |
| Toolchain: Rust 1.99 | Design 22, Conventions |
| CubeCL pin; kernels may use cubecl without runtime features; runtime types only in tetraloma-device; no Runtime type parameter; runtime features only in testing, examples, benches, binaries | Design 22 |
| Kernels: launch shape from LaunchPlan, checked usize arithmetic, six buffers, no 64-bit or float atomics, no forward progress, plane size 1, bitwise deterministic | Design 22 |
| SIMD only through `Vector<F, N>`; candidate kernels generic over N; independent lanes; VectorPolicy changes only with benchmark data; four generic sets (ADR-0058) | Design 22; the policy files also design 23, Protected paths |
| Precision: every kernel test in f32 and f64; tolerances 1e-12 and 1e-5 | Design 22 |
| Host maths through `widen`/`narrow` or f64 | Design 22 |
| Orientation uses origin ids | Design 22 |
| Tests: `cargo test --workspace` on the CPU runtime; `TETRALOMA_TEST_DEVICE` | `AGENTS.md` rule 11, Tests; Checks |
| MPI tests: crate layout, `scripts/mpi-test.sh 1 2 3 4` under MPICH and Open MPI, world-size check (ADR-0068) | Design 22 (layout and check); `AGENTS.md` Checks (the runs); the rank guard also design 23 |
| Run plans (ADR-0057), standing matrix | `AGENTS.md` rule 11, Runs; the standing matrix defined in design 25, Glossary |
| ndelement only in `conformance/`, never `version =` | Design 22; the pin also design 23 |
| After bumping the ndelement pin: conformance tests and re-export of the fixtures | `AGENTS.md` rule 11, ndelement |
| Allowed host↔device copies (full list) | Design 22, with the newer `CLAUDE.md` wording "gather, scatter, pack and unpack" |
| FFI declarations only in -sys crates; FFI calls only in three crates; SAFETY comments | Design 22 |
| User documentation (ADR-0067) | Design 22 |
| A milestone that adds or changes public items documents them and updates the book in the same task; `check-user-docs.sh` at the review | Design 22 (verbatim); `AGENTS.md` Checks (`full` and `milestone` run `check-user-docs.sh`) |
| Each milestone: acceptance tests first, then implementation, unit tests, property tests, rustdoc examples | Superseded for tests first: `AGENTS.md` rule 2, plan T0; the rest in rule 11, Tests |
| If an API deviation is unavoidable, stop and draft an ADR | Superseded: `AGENTS.md` rule 4; decision skill |

## CLAUDE.md: Workflow

| Old rule | New place |
| --- | --- |
| One milestone per branch, from an up-to-date main | Superseded: one task per branch (`AGENTS.md` rule 1, Conventions on GitHub) |
| A plan per milestone from `docs/plans/TEMPLATE.md` by `/plan-milestone`; the plan is the hand-off | Superseded: plan-milestone skill; the hand-off is the task issue's comments (rule 10) |
| Work only on tasks of an approved plan; tick tasks; Deviations log | Superseded: the merged plan is the approval; state on GitHub; deviations in the pull request (rule 5) |
| Commit messages `<milestone> T<n>: …`, `plan:`, `adr:`, `review:` | Dropped: titles are set by the skills (`<task id>: …`, `Plan M<n>`, `ADR-<NNNN>: …`); squash merges |
| Design not edited during implementation; changes need an accepted ADR | Superseded: `AGENTS.md` rule 4; decision skill |
| CubeCL upstream log (`C<n>` entries) | `AGENTS.md` rule 11, CubeCL upstream log |
| Delegated work: task text, design sections, the ADR-draft rule | `CLAUDE.md`, Claude Code only |
| Test-runner subagent for full-suite, multi-backend and MPI runs | `CLAUDE.md`, Claude Code only |
| Design-reviewer subagent before plan approval and the milestone PR | Superseded: review skill (task mode, gate mode with gate-architecture); its checklist below |
| New worktree: `scripts/setup-worktree.sh` | `AGENTS.md` rule 11, Worktrees |
| Milestone issue `<milestone> <name>` with label `enhancement`, task checklist, tick command | Superseded: milestone and task issues with sub-issues (Conventions on GitHub); old issues keep their label (`AGENTS.md`, How work flows) |
| Claude may push branches and merge into main | Superseded by stricter rules: never push to main (rule 9); merge only a named pull request on the maintainer's instruction (Merging) |
| Never force-push or rewrite history | Superseded: rule 9; `.claude/settings.json` denies |

## CLAUDE.md: Test targets

Every bullet moved to `AGENTS.md` rule 11, Test targets, verbatim except: "milestone review"
reads "gate"; "the user" reads "the maintainer"; the E0 exception for hand-written `qsub` and
`ssh` commands is dropped (E0 is merged); Locust gains the rule from the old review skill that an
unavailable Locust run is recorded, not counted as missing.

## Old skills and agents

| Old step | New place |
| --- | --- |
| plan-milestone: dependencies merged; branch and clean tree; existing plan | Superseded: plan-milestone skill preconditions |
| plan-milestone: read the roadmap entry, design sections, ADRs, the code, earlier plans' deviations | Superseded: plan-milestone skill, Write the plan, step 1 |
| plan-milestone: T0 with stubs and `todo!()`; tasks under ~600 lines; per-task fields | Superseded: plan-milestone skill and plan template |
| plan-milestone: runs planned at the minimum; benchmark tasks name targets, candidates, sizes, runs, wall time | `AGENTS.md` rule 11, Runs and Plans |
| plan-milestone: acceptance table with targets and backends; vector-size invariance test for every candidate kernel | `AGENTS.md` rule 11, Plans |
| plan-milestone: "Parallel with" only without shared files | Dropped: one task per pull request; independent tasks run as separate tasks |
| plan-milestone: design-reviewer critique for M3, M6, M7, M9 or more than ten tasks | Superseded: the template caps a milestone at about eight tasks; plan pull requests can be reviewed with the review skill |
| plan-milestone: issue creation with label `enhancement` | Superseded: plan-milestone skill, Create the issues |
| implement-milestone: plan approved, branch, clean tree, `setup-worktree.sh` | Superseded: implement-task skill, Start; `AGENTS.md` rule 11, Worktrees |
| implement-milestone: T0 tests compile and fail with `not yet implemented` | Superseded: implement-task skill, Tests first |
| implement-milestone: kernel tasks on Metal, then Locust in the background before dependent tasks | `AGENTS.md` Checks, `full` (Locust when kernels change, before the pull request is ready) |
| implement-milestone: commit, tick, issue checkbox; `/clear` advice | Superseded: implement-task skill; fresh session per task (`WORKFLOW.md`) |
| implement-milestone: parallel tasks in worktree subagents, merged with `--no-ff` | Dropped: one task per pull request |
| implement-milestone: final acceptance run through the test-runner | Superseded: the gate runs the `milestone` tier once (review skill, gate mode) |
| review-milestone: design review of `main...HEAD` | Superseded: gate-record and gate-architecture passes |
| review-milestone: full run, standing matrix, once | `AGENTS.md` Checks, `milestone` tier |
| review-milestone: every acceptance row against a passing test; skipped counts as missing; Locust exception | Superseded: review skill, gate step 3; Locust exception in `AGENTS.md` rule 11 |
| review-milestone: fix Blockers and Majors; re-run only what fixes reach, proposed as a table with what each cut loses | Superseded: gate triage with the maintainer; the re-run rule in `AGENTS.md` rule 11, Runs |
| review-milestone: Review section in the plan; PR description; tick acceptance boxes | Superseded: `docs/reviews/M<n>.md` and the milestone issue |
| draft-adr: number, template, options with "keep the design", consequences, stop | Superseded: decision skill, ADR |
| draft-adr: link from the plan's open questions | Superseded: a `decision` issue blocks the affected tasks |
| draft-adr: apply only the named sections, `adr: NNNN apply`; update the milestone issue | Superseded: decision skill steps 6–7 |
| design-reviewer: scope, acceptance coverage, tests first | Superseded: fidelity and gate-architecture briefs |
| design-reviewer: public API against the listings | Superseded: fidelity and gate-architecture briefs (interfaces against the design) |
| design-reviewer: crate rules, kernel rules, precision, conventions (Appendix A, map direction), safety, user documentation, host↔device copies | `AGENTS.md` rule 11, Review focus, citing design 22 and the sections named there |
| design-reviewer: may run `scripts/check-user-docs.sh --no-build` | Dropped for reviewers (template reviewers need no tools beyond git); `check-user-docs.sh` runs in the `full` and `milestone` tiers |
| verify skill | Kept as a project skill (`.agents/skills/verify`): the `quick` tier; its pre-M0 step dropped |
| hpc-test skill | Kept as a project skill (`.agents/skills/hpc-test`); "plan's verification line" reads "task's plan section, Verification set or check tier"; pre-M0 command kept for env-check only |
| test-runner agent | Kept (`.claude/agents/test-runner.md`); pre-M0 and pre-E0 notes dropped |

## Other files

| Old file or part | New place |
| --- | --- |
| `IMPLEMENTATION.md`: archive contents, one-time setup, the loop, parallel milestones, one-off prompts | Superseded: `WORKFLOW.md` |
| `IMPLEMENTATION.md`: tool table | Already in `docs/dev-environment.md`, One-time setup |
| `IMPLEMENTATION.md`: milestone notes (E0–M10) | Dropped: history of finished milestones; the plans hold it |
| `IMPLEMENTATION.md`: troubleshooting | `docs/dev-environment.md`, Troubleshooting |
| `docs/plans/TEMPLATE.md` | Superseded: `.agents/skills/plan-milestone/references/plan-template.md` |
| `docs/adr/TEMPLATE.md`, `docs/adr/README.md` process | Superseded: decision skill and its ADR template; `docs/adr/README.md` is now the index |
| `docs/adr/README.md`: a choice private to one crate needs no ADR, only a Deviations-log row | Superseded: a deviation from the plan is listed in the pull request (rule 5) |
| Design: "Handing this design to Claude Code", Listing H1 | Retired by ADR-0069; rules in design 22 and `AGENTS.md` |
| Design: "Implementation roadmap" | `docs/roadmap.md` |
| `CLAUDE.md` citations in code comments, `scripts/mpi-test.sh`, `tools/env-check` | Now cite design "Conventions" |
| `docs/cubecl-upstream.md` citation of `CLAUDE.md` | Now cites `AGENTS.md` rule 11 |
| `scripts/doc_audit.py` developer-citation pattern | Also catches `AGENTS.md` and `WORKFLOW.md` |
| `.claude/settings.json` | Kept; `git merge --no-ff` no longer pre-approved; read-only `gh` commands pre-approved; pushes to main and `--no-verify` denied |

## Agents' memory (Claude Code, local)

| Memory | New place |
| --- | --- |
| One local cargo test run at a time | `AGENTS.md` Lessons |
| No edits while `remote-test.sh` syncs | `AGENTS.md` Lessons |
| `remote-test.sh` re-quotes inline commands | `AGENTS.md` Lessons |
| `remote-test.sh` deletes its job on SIGTERM | `AGENTS.md` Lessons |
| Pinned worktree for long runs | `AGENTS.md` Lessons |
| Metal feature flag, `HDF5_DIR` for `check-docs.sh`, Metal device-lost rerun | `AGENTS.md` Lessons (two lines) |
| hwloc flags for local MPI commands | `AGENTS.md` Lessons; details already in `docs/dev-environment.md` |
| Review re-runs reduced | `AGENTS.md` rule 11, Runs |
| Locust is an open node | Already ADR-0002 and `AGENTS.md` rule 11, Test targets; removed from memory |
| Worktree setup without asking | `AGENTS.md` rule 11, Worktrees (the permission stays personal) |
| ssh and push outside the sandbox; `.claude` edits outside the sandbox; benchmark sweeps unsandboxed | Personal permissions: stay in memory; the sandbox rule is in `AGENTS.md`, Sandboxes |
| Local partition-library paths | Machine-specific: stays in memory; the procedure is in `docs/dev-environment.md` |
| Rename to tetraloma; CPU-policy follow-up | History: ADR-0066, ADR-0065 and the plans hold it |

## Known gap

Design section 14, "Keeping it current", still says "`/review-milestone` runs
`scripts/check-user-docs.sh`". ADR-0069 did not list that sentence. In the new workflow the gate's
`milestone` tier runs the script (`AGENTS.md`, Checks). The sentence changes with the next ADR that
touches section 14, or with a one-line ADR if the maintainer wants it sooner.

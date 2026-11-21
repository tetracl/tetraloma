# ADR-0070: Corrections to the design text of ADR-0069

Status: Proposed
Date: 2026-11-21
Decision issue: #58
Raised by: independent review of the workflow migration (#57, findings W1, W3 and W5)

## Context

ADR-0069 added section 23, Protected paths, to `docs/design.md` and moved the old workflow's rules
out of the design. The review of the follow-up pull request #57 (Gemini 4 Neon, two rounds) found
four passages that are wrong or stale. They were left as written because the design changes only
through an ADR; `docs/workflow-migration.md`, "Known gaps in the design", lists them.

1. **Section 23, row 1** protects `tetraloma-device/vector-policy.toml` and
   `tetraloma-geometry/degree-policy.toml`. Its checks are
   `cargo test -p tetraloma-device --test integration policy::` and
   `cargo test -p tetraloma-bench --test perf_record`. Neither reads `degree-policy.toml`:
   `perf_record.rs` never names it, and `tetraloma-device/tests/policy.rs` reads only the vector
   policy. The tests that do read it exist and pass on the CPU runtime (2026-11-21):
   - `cargo test -p tetraloma-geometry --test integration strategy::degree_policy_has_four_sets`
     (`tetraloma-geometry/tests/strategy.rs`, the four generic sets of ADR-0058);
   - `cargo test -p tetraloma-bench --test high_order_record` (13 tests; reads
     `tetraloma-geometry/degree-policy.toml` and checks it against the committed records).
2. **Section 23, row 4** names `scripts/check-workspace-deps.sh` as the check for "Rust 1.99;
   `cubecl = "=0.11.0"`; a default build free of ndelement, BLAS and MPI". The script runs
   `cargo tree` and fails only on the forbidden crates (ndelement, BLAS/LAPACK, MPI). Nothing in the
   repository checks the pins: hosted CI installs whatever channel `rust-toolchain.toml` names,
   and `tools/env-check/tests/acceptance.rs` checks `rustc 1.99` only when env-check runs.
   `Cargo.toml` pins both `cubecl` and `cubecl-runtime` at `=0.11.0`; row 4 names only `cubecl`.
3. **Section 9, "Mark and number"** says of `unique_by_key`: "its count read is the one
   `CLAUDE.md` allows (ADR-0012)". Since ADR-0069 the list of allowed copies is section 22,
   Conventions; `CLAUDE.md` no longer holds it.
4. **Section 14, "Keeping it current"** ends: "`/review-milestone` runs
   `scripts/check-user-docs.sh`." That skill was removed in #57; the gate's `milestone` check tier
   runs the script (`AGENTS.md`, Checks), and the `full` tier runs it for every task that adds or
   changes public items or the book.

## Options

1. **Keep the design as written.** Section 23 then claims that `degree-policy.toml` and the
   pins are guarded when they are not: a task could change the degree policy, run row 1's checks,
   see them pass, and not stop. Sections 9 and 14 send readers to a file and a skill that no
   longer hold what they cite.
2. **Correct the text.** Row 1 lists the two degree-policy checks besides the two it has. Row 4
   names both pins and says plainly what is and is not checked: `scripts/check-workspace-deps.sh`
   for the forbidden dependencies; the pins have no automated check, so a change to them is seen
   in the diff and is a design change (section 2, fixed decision 2; section 22). Sections 9 and 14
   cite section 22 and the gate. No code, test or CI change.
3. **Correct the text and add a pin check.** As option 2 for rows 1, sections 9 and 14; row 4
   names a new check, `scripts/check-pins.sh`, which fails unless `rust-toolchain.toml` has
   `channel = "1.99"` and `Cargo.toml`'s `[workspace.dependencies]` pin `cubecl` and
   `cubecl-runtime` at `=0.11.0`, run by a hosted CI step. That is a new script and a CI change,
   so it needs a task of its own (a roadmap milestone, or a task in the next one), and row 4 would
   name a check that does not exist until that task is merged.

## Recommendation

Option 2. It makes section 23 true today with checks that exist and pass. The pins change rarely
and only through an ADR (a CubeCL upgrade is planned as its own milestone, section 19), and every
reviewer sees a change to `rust-toolchain.toml` or `Cargo.toml` in the diff, so an automated pin
check buys little; if you want one, option 3's script can be added later as a task that also
updates row 4.

## Changes if accepted

- Design sections:
    - **9, "Mark and number"** (step 4): "its count read is the one `CLAUDE.md` allows
      (ADR-0012)" becomes "its count read is the one section 22 allows (ADR-0012, ADR-0070)".
    - **14, "Keeping it current"**: "`/review-milestone` runs `scripts/check-user-docs.sh`" becomes
      "`scripts/check-user-docs.sh` runs in the `full` check tier of every such task and in the
      gate's `milestone` tier (`AGENTS.md`, Checks; ADR-0070)".
    - **23, row 1**, Check column: `cargo test -p tetraloma-device --test integration policy::`;
      `cargo test -p tetraloma-geometry --test integration strategy::degree_policy_has_four_sets`;
      `cargo test -p tetraloma-bench --test perf_record`;
      `cargo test -p tetraloma-bench --test high_order_record` (ADR-0070).
    - **23, row 4**: Path column "`rust-toolchain.toml`, the `cubecl` and `cubecl-runtime` pins in
      `Cargo.toml`"; What column "Rust 1.99; `cubecl` and `cubecl-runtime` at `=0.11.0`; a default
      build free of ndelement, BLAS and MPI"; Check column "`scripts/check-workspace-deps.sh` (the
      forbidden dependencies). The pins have no automated check: a change to them shows in the
      diff and is a design change (section 2, fixed decision 2; section 22) (ADR-0070)".
- Interfaces and tests: none.
- Plans and tasks affected: none (no milestone is open).
- New tasks: none with option 2. With option 3: one task, `scripts/check-pins.sh` and its CI step.
- Outside the design: `docs/workflow-migration.md`, "Known gaps in the design", records that
  ADR-0070 resolves the four gaps; `docs/adr/README.md` gains the index row.

## Decision

> <the maintainer's words, verbatim>, <date>

# ADR-0073: Local CPU test concurrency and full-tier scoping for host-only changes

Status: Accepted
Date: 2026-11-22
Decision issue: none (requested and accepted in session; tracked in pull request #78; amends ADR-0057)
Raised by: M11.T0 (pull request #77) local `full`-tier run taking 2 h 05 m on the M6 Mac mini

## Context

Running bare `cargo test --workspace` followed by Metal on the local M6 Mac mini (12 cores: 2 Super, 4 Performance, 6 Efficiency) for M11.T0 (pull request #77, which added `tetraloma-element`, `tetraloma-refcell` tests, and CI/workspace-dep checks) took **2 h 05 m 11 s** (**1 h 53 m** for CPU, **12 m** for Metal).

Two workflow-level causes were identified alongside test-harness inefficiencies (addressed in separate pull requests #79–#82):

1. **CubeCL CPU thread oversubscription (consistent with C14 spin-collapse).** `cubecl-common 0.11.0` uses a single device server thread with a 32-task queue (`docs/cubecl-upstream.md`, C14), and `cubecl-cpu 0.11.0` uses a 12-thread worker pool (C1, C21, C24). Under libtest's default `--test-threads=12`, concurrent device tests and `ThreadComm` rank threads oversubscribe the cores and trigger launch-throughput collapse consistent with C14. Measured directly on the M6 Mac mini (debug build, `main`, 16 thread-heavy integration tests `submesh_threads:: orient_threads:: scatter_threads::`):
   - default `--test-threads=12`: **849.68 s** wall time, **9,797.99 s** user CPU time (14 of 16 tests stalled > 60 s);
   - `--test-threads=1`: **341.15 s** wall time, **3,766.84 s** user CPU time (0 tests > 60 s) — **2.49× faster wall time** and **2.60× less CPU time**.
   In the M11.T0 bare workspace run, `tetraloma`'s integration binary alone took **5,566 s (92 m 46 s)** on CPU versus **379 s (6 m 19 s)** on Metal, and `tetraloma`'s unit tests took **172 s** on CPU versus **6.5 s** on Metal. (A full-workspace timed run of `scripts/test-cpu.sh` has not yet been taken; the speedup on the whole workspace is estimated from the 16-test subset.)
2. **Unconditional full-workspace local runs on host-only changes (reversing ADR-0057 Option 3).** In ADR-0057 (2026-11-16), Option 3 ("**Also thin the standing matrix.** … Not chosen: the maintainer keeps the matrix") was declined and the decision record states "**the standing matrix is kept**" (item 1: "Standing matrix unchanged"). Consequently, `AGENTS.md` required running the full workspace on both CPU and Metal locally on every change, even when a change touches only host-only workspace crates (crates with no `cubecl` or `cubecl-*` in their default-feature dependency graph in `cargo metadata`, such as `tetraloma-element` during M11 before M13 adds downstream CubeCL dependencies, or test/doc-only edits inside host-only crates). Hosted CI already runs the full sharded Linux x86-64 CPU, software Vulkan, MPI, and docs matrices on every push before a pull request is marked ready, and every milestone gate (`milestone` tier) still runs the full workspace locally on both CPU and Metal.

## Options

1. **Keep the design and `AGENTS.md` as written.** Every PR runs bare `cargo test --workspace` (default 12 libtest threads) and full-workspace Metal locally, spending ~2 hours on host-only PRs.
2. **Cap CubeCL CPU test concurrency locally (`scripts/test-cpu.sh`) and keep full-workspace Metal on every change.** Eliminates the measured 2.49× CPU oversubscription penalty without reversing ADR-0057's standing-matrix decision, while keeping full local CPU (estimated ~35–50 min before PRs #79–#82) and Metal (~12 min) on every PR.
3. **Cap CubeCL CPU test concurrency locally (`scripts/test-cpu.sh`) and mechanically scope local `full`-tier CPU and Metal runs on host-only changes (reverses ADR-0057's "standing matrix unchanged" for host-only `full`-tier PRs).**
   - Add `scripts/test-cpu.sh` (for the local Mac; Locust aarch64 hits C17 under `--test-threads=1`), which classifies workspace members via cycle-safe reverse BFS from `cubecl`/`cubecl-*` in `cargo metadata --format-version 1` (default-feature graph, matching today's featureless `cargo test --workspace`), runs host-only crates at default libtest parallelism and CubeCL crates at `--test-threads=${TETRALOMA_CPU_TEST_THREADS:-1}`, and supports `-p CRATE`, `--with-rdeps`, and `--print-cubecl`. (Splitting into two `cargo test` invocations builds shared dependencies without `tetraloma-testing`'s optional features for host-only crates, matching `cargo test -p <host-crate>`.)
   - Use `scripts/test-cpu.sh` for local Mac CPU runs across `quick`, `full`, and `milestone` tiers (and `test-runner`).
   - Define **host-only scoping** for the `full` tier mechanically from `git diff --name-only <base>...HEAD` (conditional on hosted CI passing on the head commit before marking the PR ready):
     - A change is host-only when every modified path lies inside host-only workspace crate(s) (no CubeCL crate, root `Cargo.toml`, `Cargo.lock`, or workspace-wide script modified; note that M11.T0 itself modified `Cargo.toml`, `Cargo.lock`, `ci.yml`, and `scripts/check-workspace-deps.sh`, so PRs touching workspace wiring still run the full suite).
     - **Path-defined test/doc-only edits** inside host-only crate(s): every modified path is under `<crate>/tests/` or `<crate>/*.md`, **excluding** shared fixture/data directories read across crates (such as `tetraloma-refcell/tests/fixtures/`, read by `tetraloma-topology` and `tetraloma` via `CARGO_MANIFEST_DIR`). Local `full` runs `scripts/test-cpu.sh -p <touched>...` (no local Metal run).
     - **Library, manifest, or shared-fixture edits** inside host-only crate(s): local `full` runs `scripts/test-cpu.sh --with-rdeps -p <touched>...` on CPU and runs Metal on the CubeCL crates in that reverse-dependency set via `metal_pkgs=$(scripts/test-cpu.sh --with-rdeps --print-cubecl -p <touched>...); [ -z "$metal_pkgs" ] || TETRALOMA_TEST_DEVICE=metal cargo test $metal_pkgs --features tetraloma-testing/metal`. For a host-only crate with no CubeCL reverse dependency in `cargo metadata` at that commit (e.g. `tetraloma-element` in M11 T4–T7 before M13), `metal_pkgs` is empty and the local run finishes in seconds (estimated under a minute); for `tetraloma-refcell` or `tetraloma-dims` library edits, `--with-rdeps` selects all 11 CubeCL crates and runs them on CPU and Metal.
     - Explicitly note what hosted CI covers (Linux x86-64 CPU, software Vulkan, MPICH/Open MPI, docs) and what remains local-only (Metal and macOS arm64 CPU, still run in full on every CubeCL/workspace change and at every milestone gate).

## Recommendation

Option 3 (or Option 2 if the maintainer prefers keeping ADR-0057's standing matrix unchanged for Metal). Option 3 gives the measured 2.49× wall-time reduction on CPU-heavy CubeCL tests and shrinks local `full`-tier runs to seconds on pure `tetraloma-element` M11 tasks (T4–T7) and host-only test/doc PRs, while hosted CI verifies the complete Linux/Vulkan/MPI matrix on every PR and milestone gates still run the complete local macOS CPU + Metal standing matrix.

## Changes if accepted

Each design edit carries the marker `(ADR-0073)`. Option 3 amends ADR-0057's standing-matrix rule for host-only `full`-tier PR runs.

- **Design sections (`docs/design.md`):**
  - **Section 7, `Supported backends`:** rows `CPU runtime` and `Metal via wgpu (MSL)` note host-only `full`-tier scoping `(ADR-0073)`.
  - **Section 16, `Testing and validation`:** `Local Mac` row notes host-only `full`-tier scoping when hosted CI runs `(ADR-0073)`.
  - **Section 17, `Development environment and test targets`:** intro paragraph and `Local Mac` row note host-only `full`-tier scoping `(ADR-0073)`.
  - **Section 18, `CI matrix`, row `local`:** local CPU runs use `scripts/test-cpu.sh`, and host-only `full`-tier CPU and Metal runs scope mechanically via `cargo metadata` when hosted CI runs `(ADR-0073)`.
  - **Section 25, `Glossary`, `Standing matrix`:** note the host-only `full`-tier scoping amendment to ADR-0057 `(ADR-0073)`.
- **ADRs:**
  - `docs/adr/0057-minimum-test-and-benchmark-runs.md`: append `Amended by ADR-0073` line.
  - `docs/adr/README.md`: add ADR-0073 row.
- **Process documents and scripts:**
  - `scripts/test-cpu.sh`: local Mac CPU test runner with reverse-BFS CubeCL classification, `--with-rdeps`, and `--print-cubecl`.
  - `.github/workflows/ci.yml`: add `scripts/test-cpu.sh` to the `shellcheck` step.
  - `AGENTS.md`: update `Test targets`, `Checks` (`quick`, `full`, `milestone`) and `Lessons`.
  - `.agents/skills/verify/SKILL.md`: allow and reference `scripts/test-cpu.sh -p <crate>` on the local Mac.
  - `.claude/agents/test-runner.md` and `.claude/settings.json`: use and allow `scripts/test-cpu.sh` on the local Mac.
  - `docs/cubecl-upstream.md`: add the 2026-11-22 M6 Mac mini 12-vs-1-thread measurement and `scripts/test-cpu.sh` workaround to C14.
  - `docs/dev-environment.md`: record the 849.68 s → 341.15 s measurement, update `Hosted CI` (shellcheck of three scripts), `Test commands`, and `Change history`.
- **Interfaces and tests:** no library API, test assertion or tolerance changes.
- **Plans and tasks affected:** none (active M11 tasks inherit the updated `AGENTS.md` check tiers; M11's "check commands stay as they are" applies to M11 task PRs, and `docs/plans/M11.md:256` restates the pre-ADR-0073 milestone tier command).
- **New tasks:** none (separate PRs #79–#82 address `ThreadComm` fairness, serial-reference caching, `trybuild` isolation, and `for_each_strategy!` deduplication).

## Decision

Accepted **Option 3** by the maintainer in session on 2026-11-22 (*"Accept option 3"*), capping local Mac CubeCL CPU libtest concurrency via `scripts/test-cpu.sh` and amending ADR-0057 to mechanically scope local Mac `full`-tier CPU and Metal runs on host-only changes when hosted CI runs on the head commit. Applied in pull request #78.

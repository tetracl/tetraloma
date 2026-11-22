# ADR-0073: Local CPU test concurrency and full-tier scoping for host-only changes

Status: Proposed
Date: 2026-11-22
Decision issue: none (requested in session; tracked in pull request #78; amends ADR-0057)
Raised by: M11.T0 (pull request #77) local `full`-tier run taking 2 h 05 m on the M6 Mac mini

## Context

Running bare `cargo test --workspace` followed by Metal on the local M6 Mac mini (12 cores: 2 Super, 4 Performance, 6 Efficiency) for M11.T0 took **2 h 05 m 11 s** (**1 h 53 m** for CPU, **12 m** for Metal), even though M11.T0 touched only host-only code (`tetraloma-element`, `tetraloma-refcell` tests, and `scripts/check-workspace-deps.sh`).

Two workflow-level causes were identified alongside test-harness inefficiencies (addressed in separate pull requests #79–#82):

1. **CubeCL CPU thread oversubscription (C14 spin-collapse).** `cubecl-cpu 0.11.0` uses a single device server thread with a 32-task queue and a 12-thread worker pool (`docs/cubecl-upstream.md`, C1, C14, C21, C24). Under libtest's default `--test-threads=12`, concurrent device tests and `ThreadComm` rank threads oversubscribe the cores and trigger C14's launch-throughput collapse. Measured directly on the M6 Mac mini (debug build, `main`, 16 thread-heavy integration tests `submesh_threads:: orient_threads:: scatter_threads::`):
   - default `--test-threads=12`: **849.68 s** wall time, **9,797.99 s** user CPU time (14 of 16 tests stalled > 60 s);
   - `--test-threads=1`: **341.15 s** wall time, **3,766.84 s** user CPU time (0 tests > 60 s) — **2.49× faster wall time** and **2.60× less CPU time**.
   In the M11.T0 bare workspace run, `tetraloma`'s integration binary alone took **5,566 s (92 m 46 s)** on CPU versus **379 s (6 m 19 s)** on Metal, and `tetraloma`'s unit tests took **172 s** on CPU versus **6.5 s** on Metal.
2. **Unconditional full-workspace local runs on host-only changes.** The `full` tier in `AGENTS.md` and the standing matrix of ADR-0057 required running the full workspace on both CPU and Metal locally on every change, even when a change touches only host-only workspace crates (crates with no `cubecl` or `cubecl-*` in their resolved dependency graph in `cargo metadata`). Hosted CI already runs the full sharded Linux x86-64 CPU, Vulkan, MPI, and docs matrices on every push before a pull request is marked ready, and every milestone gate (`milestone` tier) still runs the full workspace locally on both CPU and Metal.

## Options

1. **Keep the design and `AGENTS.md` as written.** Every PR runs bare `cargo test --workspace` (default 12 libtest threads) and full-workspace Metal locally, spending ~2 hours on host-only M11 PRs.
2. **Cap CubeCL CPU test concurrency locally (`scripts/test-cpu.sh`) and keep full-workspace Metal on every change.** Eliminates the 2.5× CPU oversubscription penalty without narrowing ADR-0057's standing matrix, but host-only PRs still spend ~12 minutes running untouched CubeCL crates on Metal plus ~25–45 minutes on CPU.
3. **Cap CubeCL CPU test concurrency locally (`scripts/test-cpu.sh`) and mechanically scope local `full`-tier CPU and Metal runs on host-only changes (amends ADR-0057).**
   - Add `scripts/test-cpu.sh` (for the local Mac; Locust aarch64 hits C17 under `--test-threads=1`), which classifies workspace crates via reverse BFS from `cubecl`/`cubecl-*` in `cargo metadata --format-version 1`, runs host-only crates at default libtest parallelism and CubeCL crates at `--test-threads=${TETRALOMA_CPU_TEST_THREADS:-1}`, and supports `-p CRATE` and `--with-rdeps` (transitive reverse workspace dependencies from `cargo metadata`).
   - Use `scripts/test-cpu.sh` for local Mac CPU runs across `quick`, `full`, and `milestone` tiers (and `test-runner`).
   - Define **host-only scoping** for the `full` tier mechanically (conditional on hosted CI passing on the head commit before marking the PR ready):
     - A change is host-only when every modified path lies inside host-only workspace crate(s) (no CubeCL crate, root `Cargo.toml`, `Cargo.lock`, or workspace-wide script modified).
     - If only tests or docs inside host-only crate(s) changed, local `full` runs `scripts/test-cpu.sh -p <touched>...` (no local Metal run).
     - If library source or manifests of host-only crate(s) changed, local `full` runs `scripts/test-cpu.sh --with-rdeps -p <touched>...` on CPU and runs Metal on the CubeCL crates in that reverse-dependency set (none if `cargo metadata` reports no CubeCL reverse dependency at that commit).
     - Note explicitly what hosted CI covers (Linux x86-64 CPU, software Vulkan, MPICH/Open MPI, docs) and what remains local-only (Metal and macOS arm64 CPU, still run in full on every CubeCL/workspace change and at every milestone gate).

## Recommendation

Option 3. The 12-vs-1-thread measurement shows a 2.49× wall-time reduction on CPU-heavy CubeCL tests without dropping any test, and mechanical `--with-rdeps` scoping cuts host-only local `full`-tier runs to under a minute while hosted CI verifies the complete Linux/Vulkan/MPI matrix on every PR and milestone gates still run the complete local macOS CPU + Metal standing matrix.

## Changes if accepted

Each design edit carries the marker `(ADR-0073)`. This amends ADR-0057's standing-matrix rule for host-only `full`-tier PR runs.

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
  - `scripts/test-cpu.sh`: local Mac CPU test runner with reverse-BFS CubeCL classification and `--with-rdeps`.
  - `.github/workflows/ci.yml`: add `scripts/test-cpu.sh` to the `shellcheck` step.
  - `AGENTS.md`: update `Test targets`, `Checks` (`quick`, `full`, `milestone`) and `Lessons`.
  - `.agents/skills/verify/SKILL.md`: allow and reference `scripts/test-cpu.sh -p <crate>` on the local Mac.
  - `.claude/agents/test-runner.md` and `.claude/settings.json`: use and allow `scripts/test-cpu.sh` on the local Mac.
  - `docs/dev-environment.md`: record the 849.68 s → 341.15 s measurement, update `Test commands` and `Change history`.
- **Interfaces and tests:** no library API, test assertion or tolerance changes.
- **Plans and tasks affected:** none (active M11 tasks inherit the updated `AGENTS.md` check tiers; M11's "check commands stay as they are" applies to M11 task PRs, not this ADR).
- **New tasks:** none (separate PRs #79–#82 address `ThreadComm` fairness, serial-reference caching, `trybuild` isolation, and `for_each_strategy!` deduplication).

## Decision

Pending maintainer confirmation on PR #78 (in session on 2026-11-22, the maintainer said: *"Do PRs for all recommendations with the exception of a CubeCL upgrade. This will wait until there is an official 0.12 release."* — the edits for Option 3 are included in this pull request for review; please confirm Option 3 including the ADR-0057 standing-matrix amendment, or choose Option 2 to keep full-workspace Metal on every change).

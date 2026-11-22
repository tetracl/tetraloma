# ADR-0073: Local CPU test concurrency and full-tier scoping for host-only changes

Status: Accepted
Date: 2026-11-22
Decision issue: none (decided directly in session; tracked in this pull request)
Raised by: M11.T0 (pull request #77) local `full`-tier run taking 2 h 05 m on the M6 Mac mini

## Context

Running `cargo test --workspace` followed by Metal on the local M6 Mac mini (12 cores: 2 Super, 4 Performance, 6 Efficiency) for M11.T0 took **2 h 05 m 11 s** (**1 h 53 m** for CPU, **12 m** for Metal), even though M11.T0 touched only host-only code (`tetraloma-element`, `tetraloma-refcell` tests, and `scripts/check-workspace-deps.sh`).

Investigation of the run log and workspace test harness found two workflow-level causes alongside test-harness inefficiencies (addressed in separate pull requests):

1. **CubeCL CPU thread oversubscription (C14 spin-collapse).** `cubecl-cpu 0.11.0` uses a single device server thread with a 32-task queue and a 12-thread worker pool (`docs/cubecl-upstream.md`, C1, C14, C21, C24). Under libtest's default `--test-threads=12`, up to 11 device tests run concurrently with `ThreadComm`'s rank threads, creating 25–30 runnable threads on 6 performance cores and triggering C14's 20× launch-throughput collapse. `tetraloma`'s integration binary alone took **5,566 s (92 m 46 s)** on CPU versus **379 s (6 m 19 s)** on Metal, and `tetraloma`'s unit tests took **172 s** on CPU versus **6.5 s** on Metal. Because `cubecl-cpu` compiles kernels serially on one thread and parallelises each kernel across cores internally, capping CubeCL-crate libtest concurrency at `--test-threads=1` avoids the spin-collapse without dropping any test.
2. **Unconditional full-workspace local runs on host-only changes.** `AGENTS.md` rule 11 ("Runs", ADR-0057) requires running only what shows the code works, yet the `full` tier in `AGENTS.md` required `cargo test --workspace` and Metal over every crate even when a change touches only crates with no CubeCL dependency (for example `tetraloma-element`, dependency rule 8, or host-only additions to `tetraloma-refcell` that no downstream crate calls yet). Hosted CI already runs the full sharded CPU, Vulkan, MPI, and docs matrices on every push before a pull request is marked ready.

## Options

1. **Keep the design and `AGENTS.md` as written.** Every PR runs bare `cargo test --workspace` (default 12 libtest threads) and Metal over the whole workspace locally, spending ~2 hours on host-only M11 PRs.
2. **Cap CubeCL CPU test concurrency locally (`scripts/test-cpu.sh`) and scope local `full`-tier runs on host-only changes.**
   - Add `scripts/test-cpu.sh`, which runs host-only workspace crates (no `cubecl` in their dependency tree) at default libtest parallelism and CubeCL-dependent workspace crates at `--test-threads=1` (overrideable via `TETRALOMA_CPU_TEST_THREADS`), and accepts `-p <crate>` filters.
   - Update `AGENTS.md` (`Checks`, `full` tier) and `docs/design.md` (section 18, `CI matrix`) so that:
     - local Mac CPU workspace runs use `scripts/test-cpu.sh`;
     - when a change touches only host-only crates (no CubeCL crate in the workspace depends on the changed code, or only tests/docs/scripts of host-only crates change), the local `full` tier runs `cargo fmt`, workspace `cargo clippy`, `scripts/test-cpu.sh` scoped to the touched crates and their workspace reverse dependencies, and Metal only on reverse-dependent CubeCL crates (none for leaf host crates such as `tetraloma-element`), while hosted CI still runs the complete workspace matrix before the pull request is marked ready;
     - changes touching any CubeCL or mesh crate, and every milestone gate (`milestone` tier), keep the full local CPU and Metal workspace runs.

## Recommendation

Option 2. It preserves 100% of test coverage and tolerances—every test still runs in hosted CI on every pull request and locally at every milestone gate—while cutting M11 host-only local `full`-tier runs from ~2 hours to under a minute and cutting full local CPU runs significantly by eliminating C14 spin-collapse.

## Changes if accepted

Each design edit carries the marker `(ADR-0073)`.

- **Design sections:**
  - **Section 18, `CI matrix`, row `local`:** note that local CPU runs cap CubeCL-crate concurrency (`scripts/test-cpu.sh`) and that `full`-tier local runs on host-only changes scope to touched crates and their reverse workspace dependencies while hosted CI covers the full workspace (`ADR-0073`).
- **Process documents and scripts:**
  - `scripts/test-cpu.sh`: new script splitting host-only and CubeCL workspace crates and passing `--test-threads=${TETRALOMA_CPU_TEST_THREADS:-1}` to CubeCL crates.
  - `AGENTS.md`: update `Checks` (`full` tier) and `Lessons`.
  - `docs/dev-environment.md`: update `Test commands`, `Notes on the local Mac` and `Change history`.
  - `docs/adr/README.md`: add ADR-0073.
- **Interfaces and tests:** no library API, test assertion or tolerance changes.
- **Plans and tasks affected:** none (all M11 tasks benefit automatically through `AGENTS.md`'s `full` tier).
- **New tasks:** none (separate test-harness performance PRs address `Scatterer::reverse` blocking sync, `ThreadComm` fairness, serial-reference caching, and `trybuild` isolation).

## Decision

> Do PRs for all recommendations with the exception of a CubeCL upgrade. This will wait until there is an official 0.12 release. — 2026-11-22, said to the agent in a session.

Option 2 is accepted and applied in this pull request.

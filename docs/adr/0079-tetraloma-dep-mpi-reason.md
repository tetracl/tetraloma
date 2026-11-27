# ADR-0079: Reason for facade `tetraloma`'s optional `mpi` dependency

Status: Accepted
Date: 2026-11-27
Decision issue: #138
Raised by: issue #136 (follow-up from #131 and ADR-0078)

## Context

`docs/design.md` section 18 (crate table, row `tetraloma`) still lists `mpi (optional, for CommDyn::raw of the test communicator; ADR-0028)`, and `tetraloma/Cargo.toml:16` gives the same rationale (`# dep:mpi names mpi::ffi::MPI_Comm in the test communicator's CommDyn::raw.`).

That rationale no longer matches the codebase:

1. **`tetraloma` has no test communicator.** `src/test_comm.rs` in the facade crate was removed in M8 (`ee73a29`). No source file under `tetraloma/src/` or `tetraloma/tests/` implements `CommDyn` or names `mpi::ffi::MPI_Comm`.
2. **`CommDyn::raw` now has a default body.** ADR-0078 (implemented in #137) gave `CommDyn::raw` the default body `{ None }`, removing the need for non-MPI `CommDyn` implementors to depend on `mpi`.
3. **`dep:mpi` is still required by `tetraloma`'s MPI rustdoc examples.** Four `no_run` rustdoc examples in `tetraloma/src/` call `mpi::initialize()` to construct an `Mpi` communicator under `#[cfg(feature = "mpi")]` or `#[cfg(feature = "ptscotch")]`:
   - `tetraloma/src/mesh.rs:65` (`Mesh`)
   - `tetraloma/src/builder.rs:540` (`MeshBuilder::partitioner`)
   - `tetraloma/src/gmsh/options.rs:148` (`GmshOptions::partitioner`)
   - `tetraloma/src/gmsh/write.rs:438` (`GmshWriter::write`)

   Each sits alongside a running serial or `metis` example, and `cargo test --doc` (in `scripts/check-user-docs.sh` and the CI `docs` job) compiles them with `--features tetraloma/mpi,…`. Because Cargo `[dev-dependencies]` cannot be optional and dependency rule 5 (section 18) forbids an unconditional `mpi` dependency (`cargo test --workspace` without features must require neither MPI nor a GPU), `mpi` must remain an optional dependency enabled by `tetraloma`'s `mpi` feature (which already pulls `mpi` transitively via `tetraloma-comm/mpi`).

## Options

1. **Keep the design as written.** Section 18's `tetraloma` row and `tetraloma/Cargo.toml:16` continue to explain `dep:mpi` by a deleted test communicator and a `CommDyn::raw` obligation that ADR-0078 removed. The documented rationale remains factually wrong and misleads readers and future dependency audits.
2. **Keep `dep:mpi` on `tetraloma` and update the stated reason (recommended).** Update section 18's `tetraloma` row to record that `mpi` is optional for `mpi::initialize` in MPI rustdoc examples, and update the matching comment at `tetraloma/Cargo.toml:16` in issue #136's pull request. No public API, Cargo feature or transitive dependency changes, dependency rule 5 stays intact, and all four MPI rustdoc examples remain compile-checked by `cargo test --doc`.
3. **Drop `dep:mpi` from `tetraloma` by changing the four doctests.** Either:
   - (a) re-export `mpi` (or an MPI initialisation helper) from `tetraloma-comm` and `tetraloma` so the doctests call `tetraloma::…` instead of `mpi::initialize()`. Cost: widens the public API of published crates (`tetraloma-comm` and `tetraloma`) to expose or wrap an external crate solely to remove an optional dependency already brought in transitively by `tetraloma-comm/mpi` whenever `mpi` is enabled, and diverges from how `tetraloma-comm/src/mpi.rs` and the book tutorials initialise MPI; or
   - (b) mark the four doctests `ignore` or remove them so rustdoc does not compile `mpi::initialize()`. Cost: loses compile-checking of the facade's MPI examples and runs counter to section 14's doctest discipline against unverified `ignore` blocks.

## Recommendation

Option 2. It fixes the stale rationale in `docs/design.md` and `tetraloma/Cargo.toml` without changing any public API, Cargo feature or transitive dependency, while keeping the four MPI rustdoc examples compile-checked under `cargo test --doc`.

## Changes if accepted

For Option 2.

- **Design sections (`docs/design.md`):**
  - **Section 18, crate table, row `tetraloma`, "Depends on":** replace `mpi (optional, for \`CommDyn::raw\` of the test communicator; ADR-0028)` with `mpi (optional, for \`mpi::initialize\` in MPI rustdoc examples; ADR-0028, ADR-0079)`.
- **Interfaces and tests** (in issue #136's pull request, not in this ADR):
  - Update the comment at `tetraloma/Cargo.toml:16` to explain that `dep:mpi` is kept for `mpi::initialize()` in `tetraloma`'s `no_run` MPI rustdoc examples (Cargo dev-dependencies cannot be optional, and dependency rule 5 forbids an unconditional MPI dependency).
  - No public API changes and no test changes.
- **Process documents:** `docs/adr/README.md` (add the ADR-0079 row in the second commit of this pull request).
- **Plans and tasks affected:** issue #136 (unblocked once this ADR merges; updates `tetraloma/Cargo.toml:16`).
- **New tasks:** none.

## Decision

> Accept AdR 79 as recommended

Said to the agent in a session, 2026-11-27.

The recommendation is Option 2, so the changes listed under "Changes if accepted" apply as written.

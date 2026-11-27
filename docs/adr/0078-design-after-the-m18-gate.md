# ADR-0078: Sections 7, 9, 11, 18, 22 and 25 after the M18 gate

Status: Accepted
Date: 2026-11-26
Decision issue: #133
Raised by: the M18 gate, findings G7, G10, G11, G12, G13 and G15 (`docs/reviews/M18.md`)

## Context

The M18 gate (at `20c9ac1`) found four places where `docs/design.md` no longer matches the code M18 merged, and two costs of the design that M18 made visible. Each finding was checked against the code before it was recorded. The maintainer chose to handle them in one ADR before M14 is planned: "One ADR before M14 plan (Recommended)" (said to the agent in a session, 2026-11-26). The recommended option of that question applies the text fixes (part A) and puts G11 and G13 as options with a recommendation (parts B and C).

### Part A: text that no longer matches the code

1. **Listing S1 has no marker (G7).** ADR-0074 changed four code lines of Listing S1 (section 7): `pad_len` and `check_allocation` (twice) return `DeviceError::TooLarge` and `DeviceError::AllocationTooLarge`, and `check_precision` returns `DeviceError::UnsupportedPrecision` instead of the `GridError` variants (`git diff -U0 8585bba abb4d70 -- docs/design.md`, the hunks at lines 851, 856, 858 and 871). Neither the lines nor the listing's heading carry "(ADR-0074)". Every other passage ADR-0074 changed does. `docs/adr/README.md` lists "Listing S1" under ADR-0074, so the change can be traced, but the passage does not show it.
2. **`owner_hash` path in section 9 (G7).** Section 9, "Hash (ADR-0026)", names `tetraloma_parallel::owner_hash(key: &[i64]) -> u64`. Since M18.T2 the function is defined in tetraloma-comm (`tetraloma-comm/src/hash.rs`; Listing S2c), and tetraloma-parallel re-exports it. The path still compiles, but section 9 names the re-export as if it were the home.
3. **Stale tetraloma-io row (G10).** In the crate table of section 18, the tetraloma-io row lists "parallel (unused `partition` dependency may be dropped, ADR-0074)". M18.T3 dropped the dependency (`tetraloma-io/Cargo.toml`).
4. **"Foundation crates" is undefined (G12).** Sections 7, 14, 16 and 18 (the tetraloma-foundation-tests row and the `cpu` row of the CI matrix), and the review focus of `AGENTS.md`, use "foundation crates" as a term. Section 25 (Glossary) does not define it, and dependency rule 9 names the four crates without the term. ADR-0075 calls it "the design's word for the four crates".
5. **`mpi (optional)` that no source uses (G15).** The rows tetraloma-testing-base and tetraloma-testing in section 18 list `mpi (optional)` as a dependency. Since M18.T7 no source of either crate uses `mpi` (`git grep "mpi::" 20c9ac1 -- tetraloma-testing tetraloma-testing-base` finds nothing). `ThreadComm`, the only implementation of `CommDyn::raw` among the test-support crates, lives in tetraloma-comm-testing. Both crates' `mpi` features only forward (`tetraloma-testing-base/Cargo.toml:26`, `tetraloma-testing/Cargo.toml:27`, each with `dep:mpi`). PR #128 kept the dependencies so that the manifests match these rows.

### Part B: `CommDyn::raw` without a default (G11)

Listing S2c (section 11):

```rust
    /// Raw handle for PT-Scotch and ParMETIS; None for Serial.
    #[cfg(feature = "mpi")]
    fn raw(&self) -> Option<mpi::ffi::MPI_Comm>;
```

The method exists only under `mpi` and has no default body. So every `CommDyn` written outside tetraloma-comm needs a direct `mpi` dependency and an `mpi` feature that is on whenever tetraloma-comm's is: `ThreadComm`, test communicators, and the communicators of users who implement `CommDyn`, as the book's tutorials show and ADR-0075 decision 3 provides for (lesson of M6, T0, and plan M18, "Carried over"). M18 split one crate into several, and the cost now shows in several places:

- Since M18.T2, the test communicator `Uniform` in `tetraloma-parallel/src/index_map.rs` and its three unit tests compile only without `mpi`, because tetraloma-parallel has no `mpi` dependency of its own (G6, issue #131).
- Three manifests explain their `mpi` feature with "ThreadComm implements `CommDyn::raw`, which exists only under tetraloma-comm's `mpi`: turn this on whenever …" (tetraloma-comm-testing, tetraloma-testing-base, tetraloma-testing).
- The feature strings `tetraloma-comm-testing/mpi`, `tetraloma-testing-base/mpi` and `tetraloma-testing/mpi` appear in `ci.yml`, `ptscotch.yml`, `scripts/check-docs.sh`, `scripts/check-user-docs.sh`, `docs/dev-environment.md` and the hpc-test skill.

### Part C: `#[non_exhaustive]` errors and their conversions (G13)

The four foundation errors are `#[non_exhaustive]` (section 7; Listings S2c and S3). So each of the seven conversions between workspace crates ends in `_ => unreachable!(...)`:

- `From<CommError>`, `From<DeviceError>` and `From<ParallelError>` for `GridError` in tetraloma-topology;
- `From<CommError>` and `From<DeviceError>` for `ParallelError` in tetraloma-parallel;
- `From<CommError>` for `PartitionError` in tetraloma-partition;
- tetraloma's crate-private conversion of `PartitionError` (`pipeline/dual_graph.rs`).

A new variant of a foundation error compiles in every crate and panics when it is first converted. The guard is a unit test in each defining crate with a match over its own enum and no wildcard arm (plan M18, A4). A new variant breaks that crate's test build, and the test's comment names the conversions to update. The library build stays green.

## Options

### Part A

1. **Keep the design as written.** Listing S1 stays without its marker, section 9 names a re-export, and the rows of tetraloma-io, tetraloma-testing-base and tetraloma-testing describe dependencies that the code no longer has. M14's plan reads sections 11 and 18 when it adds tetraloma-function, so it would plan from text that does not match the code.
2. **Correct the text** (listed under Changes). The only code change is to drop `dep:mpi` from two unpublished test crates, in the follow-up task of issue #131.

### Part B

1. **Keep `raw` as written.** Every non-MPI communicator keeps the `mpi` dependency and the mirrored feature, and `Uniform`'s tests stay gated.
2. **Give `raw` the default body `{ None }`.** `Mpi` overrides it. `Serial`, `ThreadComm` and users' non-MPI communicators inherit it and need neither `mpi` nor a mirrored feature. Adding a default to a trait method does not break existing implementations. The cost: an MPI-backed communicator written outside tetraloma-comm that forgets to override `raw` gets the agreed error `NeedsMpiCommunicator` from PT-Scotch or ParMETIS at run time, instead of a compile error.

### Part C

1. **Keep the design as written,** and state the rule the guard relies on as a convention (section 22): a task that adds a variant to a foundation error adds its arms to the seven conversions in the same task.
2. **Drop `#[non_exhaustive]` from the four foundation errors.** The conversions become exhaustive matches, and a new variant breaks the build of every converting crate at once. The cost falls on users: a new variant then breaks every exhaustive match a user writes on `CommError`, `DeviceError`, `ParallelError` or `PartitionError`, for example in an implementation of `CommDyn` or `Partitioner`. Under semver that makes each new variant a breaking change of a published crate. `GridError` stays `#[non_exhaustive]` either way.

## Recommendation

A2, B2 and C1.

- **A2:** the edits describe what is merged; M14 plans from these sections.
- **B2:** it removes a mirrored feature from every communicator that does not use MPI, users' included, and lets #131 lift the `cfg` on `Uniform`'s tests. Forgetting the override fails with an agreed error that names the cause, not with undefined behaviour.
- **C1:** the foundation errors are public API of published crates, and `#[non_exhaustive]` keeps adding variants a minor change for users. Inside the workspace, the exhaustive unit tests already fail the defining crate's test build, which CI runs on every push. The convention makes the rest of the rule explicit.

## Changes if accepted

For A2, B2 and C1. Every edit carries the marker (ADR-0078), unless it only adds a marker that ADR-0074 omitted.

- **Design sections:**
    - **Section 7, Listing S1:** the heading becomes "**Listing S1 — device containers (tetraloma-device) (ADR-0074)**". This adds the marker ADR-0074 omitted; the code lines do not change.
    - **Section 9, "Hash":** "`tetraloma_parallel::owner_hash(key: &[i64]) -> u64`" becomes "`tetraloma_comm::owner_hash(key: &[i64]) -> u64` (re-exported by tetraloma-parallel; ADR-0074, ADR-0078)".
    - **Section 11, Listing S2c, `CommDyn::raw`** (B2):

      ```rust
          /// Raw handle for PT-Scotch and ParMETIS. The default is None, which Serial and
          /// communicators without MPI keep; Mpi returns its communicator (ADR-0078).
          #[cfg(feature = "mpi")]
          fn raw(&self) -> Option<mpi::ffi::MPI_Comm> { None }
      ```

    - **Section 18, crate table:**
        - row tetraloma-io, "Depends on": "parallel (unused `partition` dependency may be dropped, ADR-0074)" becomes "parallel (ADR-0078)";
        - row tetraloma-comm-testing, "Depends on": "tetraloma-comm; mpi (optional, for `CommDyn::raw` of `ThreadComm`)" becomes "tetraloma-comm (feature `mpi` forwards to tetraloma-comm's; ADR-0078)" (B2);
        - row tetraloma-testing-base, "Depends on": "; mpi (optional)" becomes "; feature `mpi` forwards to tetraloma-comm-testing and tetraloma-parallel (ADR-0078)";
        - row tetraloma-testing, "Depends on": "mpi (optional, forwards `tetraloma-comm-testing/mpi`; ADR-0028, ADR-0074)" becomes "feature `mpi` forwards to tetraloma and tetraloma-testing-base (ADR-0028, ADR-0074, ADR-0078)".
    - **Section 22, Conventions,** a new bullet after the one on FFI (C1):

      > - Foundation errors (`CommError`, `DeviceError`, `ParallelError`, `PartitionError`) are `#[non_exhaustive]`, so their conversions into one another and into `GridError` end in an `unreachable!` arm. A task that adds a variant adds its arm to every one of those conversions, and to the `GridError` variant it maps to, in the same task. The defining crate's unit test, which matches its own enum without a wildcard, names the conversions (ADR-0078).

    - **Section 25, Glossary,** a new entry after "Standing matrix":

      > - **Foundation crates:** tetraloma-comm, tetraloma-device, tetraloma-parallel and tetraloma-partition, the published crates below the mesh (dependency rule 9; ADR-0074, ADR-0078).

- **Interfaces and tests** (in the task of issue #131, not in this ADR):
    - `CommDyn::raw` gets its default body (B2), and `ThreadComm` drops its `raw` (tetraloma-comm-testing).
    - The `cfg(not(feature = "mpi"))` on `Uniform` and its three tests goes (tetraloma-parallel).
    - `dep:mpi` goes from tetraloma-comm-testing, tetraloma-testing-base and tetraloma-testing.
    - The feature strings and documentation lines that exist only for `raw` follow, each listed in the task.
- **Roadmap:** none.
- **Plans and tasks affected:** issue #131 (M18 gate G5, G6, G8) gains the items above. M14's plan builds on sections 11, 18 and 22 as amended.
- **New tasks:** none.

## Decision

> accept adr as recommended

Said to the agent in a session, 2026-11-27.

The recommendation is A2, B2 and C1, so the changes listed under "Changes if accepted" apply as written.

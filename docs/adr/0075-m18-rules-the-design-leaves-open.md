# ADR-0075: M18, rules the design leaves open: the foundation-only test crate, its checks and the new error types in the facade

Status: Accepted
Date: 2026-11-23
Decision issue: #89
Raised by: the plan of M18 (pull request #88, `docs/plans/M18.md`), its open questions 1 to 3, and findings G1, G3 and G9 of its review

## Context

Planning M18 found three places where ADR-0074's edits to `docs/design.md` do not say what the tasks need.

### 1. The foundation-only test crate (section 18)

- The roadmap entry of M18 asks for "a workspace test crate whose normal dependencies hold no workspace crate other than tetraloma-comm, tetraloma-device, tetraloma-parallel, tetraloma-comm-testing and tetraloma-testing-base".
- ADR-0074 lists "the foundation-only test crate of M18" among the new unpublished test crates.
- The crate table of section 18 has rows for tetraloma-comm-testing and tetraloma-testing-base, and none for this crate. It has no name.
- ADR-0066 and `scripts/check-name.sh` require the name `tetraloma-*`.
- Dependency rule 4 allows runtime features only in tetraloma-testing-base, tetraloma-testing, examples, benchmarks and binaries. tetraloma-mpi-tests, the other crate that holds only tests, has none.

### 2. The checks of that crate, and their MPI run (sections 16, 18 and 22)

- The roadmap entry asks the crate to run seven primitives on 10^6 elements and the scatterer on 1–4 ranks of `ThreadComm`, and asks that "the same scatter tests pass on 1–4 ranks under MPICH and Open MPI".
- Both checks exist already, above the foundation. `tetraloma/tests/scatter_threads.rs` checks `forward` and `reverse` with every `ReverseOp` against a host reference. `tetraloma-device/tests/primitives_large.rs` checks the primitives at 10^6 and 10^7 elements, with the host references of `tetraloma-device/tests/common/mod.rs`.
- Sections 16 and 18 list what tetraloma-testing-base holds, and name only the `IndexMap` checks.
- Section 22 says MPI tests live in tetraloma-mpi-tests, which depends on tetraloma, and `scripts/mpi-test.sh` (a protected path, section 23) runs the targets of that crate only.
- Since M6 the checks that thread-rank tests and MPI targets share are library functions of the test support; the MPI target starts MPI and calls them (`docs/plans/M6.md`, Deviations, T0 and T3).

### 3. The new error types in the facade (sections 7, 12 and 18)

- After M18, `Partitioner::partition` returns `PartitionError`, the methods of `CommDyn` `CommError`, `IndexMap` and `Scatterer` `ParallelError`, and the device buffers `DeviceError`.
- Section 7 and the row of tetraloma in section 18 have `tetraloma` re-export `GridError`, `Result` and `ConnKey`, and convert `PartitionError`. No listing has it re-export the four new error types.
- Code that implements `Partitioner` or `CommDyn` through `tetraloma` must name the error it returns. Today that is `RoundRobin` in `tetraloma-examples/src/distributed_mesh.rs`, which the book's tutorial on distributed meshes includes, and `ThreadComm` and the test partitioners of tetraloma-testing. Neither crate depends on a foundation crate (section 18).
- Listing S3 (section 12) says tetraloma converts `PartitionError` "via a helper function where `Partitioner::partition` is called". It does not say whether a user can call the helper.

### The maintainer's answers so far

To the plan's questions 1 to 4, said to the agent in a session on 2026-11-23: "1-4 as recommended." The recommendations of questions 1 to 3 are option 2 below with the points P1a, P2a and P3a.

## Options

1. **Keep the design as written.** The name of the test crate, the home of the shared checks and the names through which code above the facade reaches the errors are recorded only in the Decisions table of `docs/plans/M18.md`. Section 18 then lacks a workspace crate, and its rows for tetraloma-testing-base and tetraloma disagree with the code after M18.
2. **State the three rules.** The points below settle each.

### Points to settle under option 2

- **P1. The test crate.**
  - a. `tetraloma-foundation-tests`, unpublished, without features, with a row in the crate table. A run on a GPU backend names `--features tetraloma-testing-base/<backend>`, which is a feature of a dependency. Rule 4 stays as it is. The name follows tetraloma-mpi-tests and uses the design's word for the four crates.
  - b. The same crate forwarding the runtime features, as tetraloma-examples and tetraloma-bench do. Rule 4, "Cargo features" and section 22 would then name one more crate, for a convenience that a dependency feature already gives.
- **P2. The checks, and the MPI run.**
  - a. The scatter and primitive checks are functions of tetraloma-testing-base, called by the existing tests, by tetraloma-foundation-tests, and by one target of tetraloma-mpi-tests, which reaches them through tetraloma-testing. One set of checks; no crate gains a dependency; `scripts/mpi-test.sh` and the MPI-tests rule of section 22 stay as they are. The MPI executable is built with the facade in its dependency tree; the check that runs is the same.
  - b. The checks are written in tetraloma-foundation-tests, with an MPI target of its own. The checks then exist twice. `scripts/mpi-test.sh` must build and run a second package, the MPI harness (`run`, the abort hook, the call of the rank guard) needs a home below the mesh, and the MPI-tests rule of section 22 changes. This is what the first crate without a mesh will need for MPI tests of its own.
- **P3. The error types in the facade.**
  - a. `tetraloma` re-exports `CommError`, `DeviceError`, `ParallelError` and `PartitionError` at its root, not in the prelude. The conversion helper stays crate-private. A user of the facade needs no second crate to write a partitioner or a communicator. A user who calls a partitioner directly, in a function that returns `tetraloma::Result`, maps the error by hand.
  - b. No re-export. tetraloma-examples gains the dependencies tetraloma-comm, tetraloma-parallel and tetraloma-partition, tetraloma-testing gains tetraloma-comm and tetraloma-parallel, and the book tells a user who writes a partitioner to depend on tetraloma-partition.
  - c. As a, with a public conversion function. A new public item, for the one case of a direct call; ADR-0074 chose a helper over `From` because the orphan rule forbids the impl, not to offer a second public way.

## Recommendation

Option 2 with P1a, P2a and P3a.

- **P1a:** the smallest change: one row, no rule touched.
- **P2a:** one set of checks, and a protected path left alone. P2b can come with the crate that needs it.
- **P3a:** the facade exists so that a user needs one crate. The prelude stays as it is, so no glob import gains a name.

## Changes if accepted

For option 2 with P1a, P2a and P3a. Each edit carries the marker (ADR-0075).

- **Design sections:**
    - **Section 7, the paragraph after Listing S1:** after "tetraloma re-exports all three, and the re-export of `GridError` (and its `Result` alias) from tetraloma-parallel ends (ADR-0009, ADR-0074).", new sentence:

      > tetraloma also re-exports `CommError`, `DeviceError`, `ParallelError` and `PartitionError` at its root, not in its prelude, so that code which implements `Partitioner` or `CommDyn`, or matches an error of a foundation item, needs no foundation crate (ADR-0075).

    - **Section 12, Listing S3,** the comment of `PartitionError`: "via a helper function where Partitioner::partition is called (ADR-0074)" becomes "via a crate-private helper function where Partitioner::partition is called (ADR-0074, ADR-0075). tetraloma re-exports PartitionError (ADR-0075)".
    - **Section 16, "Test support below the mesh":** "tolerances, `IndexMap` checks, re-exporting `tetraloma-comm-testing`" becomes "tolerances, `IndexMap` checks, the scatter and primitive checks, re-exporting `tetraloma-comm-testing`", and the bullet gains:

      > The scatter and primitive checks are one set of functions: the tests of tetraloma-device and tetraloma call them, `tetraloma-foundation-tests` calls them without a mesh crate in its build, and a target of tetraloma-mpi-tests calls the scatter check under MPI (ADR-0075).

    - **Section 18, crate table, row tetraloma, "Purpose":** "`PartitionError` conversion (ADR-0074)" becomes "`PartitionError` conversion by a crate-private helper (ADR-0074, ADR-0075), `CommError`, `DeviceError`, `ParallelError` and `PartitionError` (re-exported at the root, ADR-0075)".
    - **Section 18, crate table, row tetraloma-testing-base, "Purpose":** "`IndexMap` checks, re-exports of `tetraloma-comm-testing`" becomes "`IndexMap` checks, the scatter and primitive checks (ADR-0075), re-exports of `tetraloma-comm-testing`".
    - **Section 18, crate table,** new row after tetraloma-mpi-tests:

      > | tetraloma-foundation-tests (unpublished) | The tests of the foundation crates without a mesh crate in the build: primitives at 10^6 elements and the scatterer on thread ranks, through the checks of tetraloma-testing-base. No features: a run on a GPU backend names a runtime feature of tetraloma-testing-base (ADR-0075) | tetraloma-device, tetraloma-parallel, tetraloma-comm-testing, tetraloma-testing-base |

- **Interfaces and tests:**
    - New public paths: `tetraloma::CommError`, `tetraloma::DeviceError`, `tetraloma::ParallelError` and `tetraloma::PartitionError`. No other signature changes.
    - New unpublished crate: tetraloma-foundation-tests.
    - Tests: none beyond the plan of M18. Its rows A6 and A7 are tests of tetraloma-foundation-tests, and A8 is the target `foundation_scatter` of tetraloma-mpi-tests.
- **Roadmap:** none. The bullet "The same scatter tests pass on 1–4 ranks under MPICH and Open MPI" is read as: the same check function, called by a target of tetraloma-mpi-tests.
- **Process documents:** `docs/adr/README.md`, the row of this ADR.
- **Plans and tasks affected:** `docs/plans/M18.md`: T1 (the crate, the checks and the MPI target) and T2 to T5 (each adds the re-export of its error type). The plan records the answers in its Decisions table. Its task issues do not exist yet.
- **New tasks:** none.

## Decision

> 1-4 as recommended., 2026-11-23, said to the agent in a session.

The answer is to the open questions 1 to 4 of the plan of M18. The recommendations of questions 1 to 3 are option 2 of this ADR with P1a, P2a and P3a, so the changes listed for them apply as written. The answer was given to the plan's questions, before this text was written. Question 4 (nine tasks) is not part of this ADR; the plan's Decisions table records it.

> Accept ADR75., 2026-11-23, said to the agent in a session.

This second answer was given with this pull request open, with the status Accepted and the design edits applied in it. It accepts the ADR as written.

`docs/plans/M18.md` is not on the main branch yet, so its changes (the answers in its Decisions table; tasks T1 to T5) are made in the plan's pull request #88, not in this one.

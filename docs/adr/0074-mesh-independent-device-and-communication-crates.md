# ADR-0074: Mesh-independent device and communication crates

Status: Accepted
Date: 2026-11-23
Decision issue: #86
Raised by: the maintainer, in a design discussion about using the device and parallel crates from crates that hold no mesh (no task)

## Context

tetraloma-device and tetraloma-parallel hold what any GPU-resident, MPI-distributed crate needs below the mesh: device buffers, launch plans, primitives, communicators, index maps and the scatterer. The design and the codebase also place mesh vocabulary in them, so a crate without a mesh cannot use them cleanly. The evidence, verified against `docs/design.md` and the current code:

1. **Errors.** Section 7 (the paragraph after Listing S1): `GridError` and the `Result<T, E = GridError>` alias live in tetraloma-device, "the lowest crate that returns them", and are re-exported by tetraloma-parallel and tetraloma. In `tetraloma-device/src/error.rs`, `GridError` holds 31 variants today, of which:
   - **tetraloma-device** itself returns only **5**: `TooLarge`, `AllocationTooLarge { bytes, max }`, `UnsupportedPrecision { scalar, device }`, `LengthMismatch { expected, found }` and `Server(ServerError)`;
   - the communicator half of **tetraloma-parallel** (`comm.rs`, `counts.rs`, `graph.rs`, `mpi.rs`, `serial.rs`) returns **5**: `TooLarge`, `LengthMismatch`, `IndexOutOfRange { index, len }`, `MessageTooLarge { count }` and `RemoteFailure { rank }`;
   - `IndexMap` and `Scatterer` in **tetraloma-parallel** return **8** (the union of those two sets: the 5 communicator variants plus `AllocationTooLarge`, `UnsupportedPrecision` and `Server`);
   - **tetraloma-partition** returns **7**: `PartitionerFailed { name, code }` and `NeedsMpiCommunicator(&'static str)` plus the same 5 communicator/validation variants (`TooLarge`, `LengthMismatch`, `IndexOutOfRange`, `MessageTooLarge`, `RemoteFailure`).

   The remaining 21 variants in `tetraloma-device/src/error.rs` are mesh, tag, I/O and manifold variants (`WrongCellDim`, `EntitiesNotCreated`, `ConnectivityNotCreated`, `UnsupportedConnectivity`, `Io`, `Parse`, `UnsupportedElementType`, `UnknownTag`, `NonManifold`, `InteriorFacet`, `NeedsReferencePoint`, `NonOrientable`, etc.) that none of these crates ever returns. Sections 26 and 28 add `UnsupportedElement` and `SpaceMismatch` to `GridError` in tetraloma-device at M14.

2. **Mesh types in the device crate.**
   - `ConnKey` (Listing T1, section 9) is defined in `tetraloma-device/src/error.rs` solely because `GridError::ConnectivityNotCreated` and `GridError::UnsupportedConnectivity` carry it. In the code it is used only in tetraloma-topology and above (`tetraloma-topology`, `tetraloma-geometry`, `tetraloma`, `tetraloma-testing`, `tetraloma-examples`) — nowhere below tetraloma-topology.
   - Section 10 and the crate table of section 18 (ADR-0071) schedule moving "the hidden `Strategy` and degree-policy reader shared by tetraloma-geometry and tetraloma-function" from tetraloma-geometry into tetraloma-device at M14/M15. In the code today they have not moved yet: `Strategy`, `StrategyFamily`, `StrategyOverride` and `DegreePolicy` live in `tetraloma-geometry/src/strategy.rs` (used only in tetraloma-geometry and tetraloma-bench), depend directly on `tetraloma_dims::CellType`, and are not needed below tetraloma-geometry (tetraloma-function already depends on tetraloma-geometry and already shares its hidden device basis evaluator in section 10).

3. **Communicators and partitioners need CubeCL.** Listing S2c (section 11) puts `CommDyn`, `Comm`, `CommExt`, `DType`, `CommScalar`, `Serial`, `Mpi`, `Neighbourhood`, `owner_hash` and `local_gpu_index` in tetraloma-parallel, which depends on tetraloma-device and so on `cubecl`, even though none of those items uses CubeCL (`Client` and `DeviceBuffer` appear only in `IndexMap::scatterer` and `Scatterer`). `ReduceOp` is defined in tetraloma-device and re-exported by tetraloma-parallel. Because `tetraloma-partition` depends on `tetraloma-parallel` solely for `CommDyn`, `CommExt`, `Serial`, `CommDyn::raw` (under `ptscotch`/`parmetis`) and `GridError`/`Result` — using nothing from `IndexMap`, `Scatterer`, `tetraloma-device` or `cubecl` — **tetraloma-partition also pulls in CubeCL today** even though it is pure host code.

4. **Test support sits above the mesh.** `test_client()`, `test_client_at`, `cpu_client`, `supports`, `bitwise_device`, `TEST_DEVICE_VAR`, `TOL_F32`, `TOL_F64`, `ThreadComm`, `comm_checks` and the rank-count guard (`check_world_size`) live in tetraloma-testing, which depends on tetraloma (section 18). Today tetraloma-device, tetraloma-parallel and tetraloma-partition all carry `tetraloma-testing` in `[dev-dependencies]` (a dev-dependency cycle through the facade, unused in tetraloma-partition's code), and the multi-rank `ThreadComm` tests of `IndexMap`, `Scatterer` and `Partitioner` live in `tetraloma/tests/`. A crate below the mesh cannot test device code or multi-rank code without depending on the facade.

5. **The coefficient scalar.** `FieldScalar` (section 29, Listing F2; not yet implemented) is planned for tetraloma-function, although it needs only `Real` (defined in tetraloma-device), `bytemuck::Pod` and `num_complex::Complex` (with `num-complex`'s `bytemuck` feature).

Two consumers that need these crates without a mesh are planned for this workspace: a native graph partitioner behind `Partitioner` (section 12), and a sparse linear algebra family. Neither has a design yet. Every milestone that builds on the present layout raises the cost of changing it. M14 is the next one: it adds `GridError` variants, introduces `FieldScalar`, and builds function spaces on `IndexMap` and `Scatterer`.

Not changed by this ADR: `KernelKind` (Listing S2, section 8; 13 variants in code today, plus 6 function variants in Listing S2 for M14/M15, including `DofTransform`, which is an enum variant rather than a type, trait, function or constant) is a closed enum in tetraloma-device with mesh and function families, and `VectorPolicy::quad_tile` (with `testing::with_quad_tile` and `TETRALOMA_QUAD_TILE`) is backed by protected path 1 (`tetraloma-device/vector-policy.toml`). Opening `KernelKind` and the vector policy before a crate without a mesh adds a kernel family is a later ADR (point P4).

### Direction from the maintainer in the design discussion (confirmed)

- The mesh library, the linear algebra, the partitioner and the communication support stay in one repository and one workspace.
- tetraloma-parallel becomes the support layer for communication across all of them.

## Options

1. **Keep the design as written.** No work now. Crates without a mesh import `GridError` and `ConnKey`; mesh variants keep accumulating in the lowest crate (the next at M14); multi-rank tests below the mesh need the facade; communicators and tetraloma-partition keep pulling in `cubecl`.
2. **Remove the mesh vocabulary in place.** The errors are split, `GridError` and `ConnKey` move up to tetraloma-topology, ADR-0071's planned move of `Strategy` and the degree-policy reader to tetraloma-device is cancelled (they stay in tetraloma-geometry), test support becomes available below the mesh, and `FieldScalar` is defined in tetraloma-device. Every path through `tetraloma` stays as it is. Communicators and tetraloma-partition still depend on `cubecl`, so host-only code cannot use them without it. No new published crate.
3. **Option 2, and a host-only crate tetraloma-comm.** The communicators, the neighbourhood exchange, `owner_hash`, `local_gpu_index` and the `mpi::ffi` wrappers move to a new crate without a `cubecl` dependency; tetraloma-parallel keeps `IndexMap` and `Scatterer` and re-exports the moved items, so existing paths stay valid; tetraloma-partition can depend on tetraloma-comm instead of tetraloma-parallel and become host-only too (point P6). Cost: twelve published crates instead of eleven, `ReduceOp` must be reachable without `cubecl` (point P2), and dev-dependency test support for host-only crates must stay free of `cubecl` (point P7).
4. **Rename the two crates to a family outside `tetraloma-*`.** ADR-0066 and `scripts/check-name.sh` require every workspace package (other than the two `-sys` crates) to be named `tetraloma` or `tetraloma-*`, and the rename would move protected path 1 of section 23 (`tetraloma-device/vector-policy.toml`). It gains nothing while tetraloma is the name of the whole workspace.
5. **Move the two crates to a repository of their own.** `cubecl` and `cubecl-runtime` are pinned at `=0.11.0` in one place (section 23, row 4). Two repositories would need coordinated pin bumps and releases, and this contradicts the maintainer's direction above.

### Points to settle under options 2 and 3

- **P1. Home of `GridError`, its `Result` alias, `ConnKey`, `Strategy` and the degree-policy reader.**
  - a. `GridError`, its `Result` alias and `ConnKey` move to tetraloma-topology, the lowest mesh crate above the foundation crates (no item moved there is needed below tetraloma-topology). `Strategy` and the degree-policy reader stay where they already live in code, tetraloma-geometry (cancelling ADR-0071's planned move to tetraloma-device), exposed as `#[doc(hidden)] pub` items for tetraloma-function (which already depends on tetraloma-geometry). tetraloma-partition does not depend on tetraloma-topology, so it gets its own `PartitionError`; `tetraloma` (where `Partitioner::partition` is called, in `tetraloma/src/pipeline/dual_graph.rs` — tetraloma-io does not call partitioners) converts `PartitionError` to `GridError` via a helper function, as tetraloma-geometry and tetraloma already convert `RefCellError` (Rust's orphan rule prevents `tetraloma` from implementing `From<PartitionError> for GridError` without making tetraloma-topology depend on tetraloma-partition).
  - b. A new crate below tetraloma-topology and tetraloma-partition for `GridError`, its alias and `ConnKey` (with `Strategy` and the degree-policy reader still staying in tetraloma-geometry). tetraloma-partition keeps returning `GridError` and stays tied to mesh errors. One more published crate.
- **P2. `ReduceOp` (option 3 only).**
  - a. Defined in tetraloma-comm; tetraloma-device depends on tetraloma-comm, without its `mpi` feature, and re-exports it. One type; `tetraloma_device::ReduceOp` and `tetraloma_parallel::ReduceOp` stay valid. The layering is linear: comm, device, parallel.
  - b. Two enums, one in each crate. The two crates stay independent, but callers of `all_reduce_bytes` and `exscan_bytes` change and the foundation crates expose two similar enums (`tetraloma` and `tetraloma::prelude` do not re-export `ReduceOp` today).
  - c. A third crate for shared small types. Symmetric, but one more published crate for one enum.
- **P3. Test support below the mesh.**
  - a. New unpublished test-support crate(s) below the mesh (`tetraloma-testing-base` with `test_client()`, `test_client_at`, `cpu_client`, `supports`, `bitwise_device`, `TEST_DEVICE_VAR`, `TOL_F32`, `TOL_F64`, `ThreadComm`, communicator checks and the rank-count guard; see P7 for keeping host-only crates free of `cubecl`). tetraloma-testing depends on it and re-exports them, so existing imports hold. Dependency rule 4 is kept: runtime features stay out of library crates (rule 4 and section 22 add `tetraloma-testing-base` beside `tetraloma-testing`).
  - b. `testing` features on tetraloma-comm and tetraloma-device. No new crate, but `test_client()` needs CubeCL runtime features in a library crate, against rule 4.
- **P4. `KernelKind` (and `VectorPolicy::quad_tile`).**
  - a. Unchanged here; opened by a later ADR, before the first kernel family of a crate without a mesh.
  - b. Opened now: each crate registers its families and ships its policy entries. This touches protected path 1 of section 23 and the benchmark records behind it.
- **P5. `FieldScalar`.**
  - a. Defined in tetraloma-device beside `Real`, and re-exported by tetraloma-function. Adds `bytemuck` (already in `[workspace.dependencies]`) and `num-complex` (feature `bytemuck`) to tetraloma-device. M14 and M15 then build on its final home.
  - b. As section 29 has it, in tetraloma-function.
- **P6. `tetraloma-partition`'s communication dependency (option 3 only; added beyond the initial draft).**
  - a. tetraloma-partition depends on tetraloma-comm instead of tetraloma-parallel (and forwards `mpi` to tetraloma-comm under `ptscotch` and `parmetis`). Because tetraloma-partition uses only `CommDyn`, `CommExt` and `Serial` (no `IndexMap`, `Scatterer` or device types), it becomes host-only and free of `cubecl`, checked by `scripts/check-workspace-deps.sh --no-cubecl tetraloma-partition` and classified as host-only by `scripts/test-cpu.sh`. Directly serves the planned native graph partitioner without pulling in CubeCL.
  - b. Keep tetraloma-partition depending on tetraloma-parallel. Pulls tetraloma-device and `cubecl` into every partitioner build with no benefit.
- **P7. Keeping host-only foundation crates free of CubeCL in dev-dependencies (option 3 + P3a; added beyond the initial draft).**
  `scripts/check-workspace-deps.sh --no-cubecl <crate>` checks `-e normal,build,dev` (including dev-dependencies), and `scripts/test-cpu.sh` (ADR-0073) classifies workspace crates via workspace-wide `cargo metadata` (which resolves `dev-dependencies` and unifies features across the workspace). If `tetraloma-testing-base` depends on `cubecl` (for `test_client()`), any host-only crate (`tetraloma-comm`, `tetraloma-partition`) that lists `tetraloma-testing-base` in `[dev-dependencies]` pulls `cubecl` into its dev-dependency graph, failing `--no-cubecl` and causing `scripts/test-cpu.sh` to classify it as a CubeCL crate.
  - a. Split unpublished low-level test support into two crates: `tetraloma-comm-testing` (host-only, no `cubecl`: `ThreadComm`, the pure-comm checks of `comm_checks`, and the rank-count guard `check_world_size`; depends only on `tetraloma-comm` and optional `mpi`) and `tetraloma-testing-base` (depends on `tetraloma-comm-testing`, `tetraloma-device` with `testing`, `tetraloma-parallel`, and `cubecl` with runtime features: `test_client()`, `test_client_at`, `cpu_client`, `supports`, `bitwise_device`, `TEST_DEVICE_VAR`, `TOL_F32`, `TOL_F64`, the `IndexMap` checks, and re-exports `tetraloma-comm-testing`). tetraloma-comm and tetraloma-partition dev-depend only on `tetraloma-comm-testing`; tetraloma-device and tetraloma-parallel dev-depend on `tetraloma-testing-base`; tetraloma-testing depends on `tetraloma-testing-base` and re-exports everything. Both tetraloma-comm and tetraloma-partition stay free of `cubecl` across normal, build and dev dependencies, and `scripts/test-cpu.sh` classifies both as host-only.
  - b. Keep one `tetraloma-testing-base` crate (with `cubecl`) and relax `scripts/check-workspace-deps.sh --no-cubecl` to `-e normal,build` (or remove `ThreadComm` dev-dependencies from tetraloma-comm and tetraloma-partition). Weakens `--no-cubecl` relative to tetraloma-element and causes `scripts/test-cpu.sh` to treat tetraloma-comm and tetraloma-partition as CubeCL crates if they dev-depend on it.
  - c. Put `ThreadComm` behind a `testing` feature on tetraloma-comm (which does not violate rule 4 because `ThreadComm` needs no CubeCL runtime feature), and keep `tetraloma-testing-base` for device test support. Avoids a second unpublished test crate, at the cost of carrying test-only code in a published library crate.

## Recommendation

Option 3, with P1a, P2a, P3a, P4a, P5a, P6a and P7a.

- **Option 3 over option 2:** host-only code can use communicators without `cubecl`, and a CubeCL pin bump cannot touch the communicator layer.
- **P1a:** no new published crate; `Strategy` and the degree-policy reader stay where they already live (tetraloma-geometry) instead of moving unnecessarily; and tetraloma-partition becomes usable without mesh errors, which the planned partitioner needs.
- **P2a:** one type and no changed call site.
- **P3a:** the only choice that keeps dependency rule 4.
- **P4a:** opening `KernelKind` is a design of its own, with benchmark consequences, and nothing needs it before the first kernel family outside the mesh.
- **P5a:** puts `FieldScalar` in its final home before M14 and M15 build on it, at the small cost of `bytemuck` and `num-complex` on tetraloma-device.
- **P6a:** tetraloma-partition already uses nothing from tetraloma-device or `cubecl`, so switching its dependency from tetraloma-parallel to tetraloma-comm makes the partitioner layer host-only at almost zero cost.
- **P7a:** keeps `scripts/check-workspace-deps.sh --no-cubecl` strict (`-e normal,build,dev`) and keeps `scripts/test-cpu.sh` classifying tetraloma-comm and tetraloma-partition as host-only crates, without putting test-only code into a published crate.

Accepted before M14 is planned, the change costs no plan and no issue.

## Changes if accepted

For option 3 with the recommended points (P1a, P2a, P3a, P4a, P5a, P6a, P7a). Each design edit carries the marker `(ADR-0074)`.

- **Design sections (`docs/design.md`):**
  - **Section 7, Listing S1 and the paragraph after it:** `pad_len`, `check_allocation` and `check_precision` return `DeviceError`. tetraloma-device defines `DeviceError` (`#[non_exhaustive]`, `Debug`, `Clone`, `Display`, `std::error::Error`, `From<ServerError>`) and its alias `Result<T, E = DeviceError>`, with the exact 5 variants tetraloma-device returns today: `TooLarge`, `AllocationTooLarge { bytes: usize, max: u64 }`, `UnsupportedPrecision { scalar: &'static str, device: &'static str }`, `LengthMismatch { expected: usize, found: usize }` and `Server(ServerError)`. `GridError`, its `Result<T, E = GridError>` alias and `ConnKey` move to tetraloma-topology. `GridError` keeps all 31 existing variants (plus M14's 2) and `From<ServerError>`, and gains `From<CommError>`, `From<DeviceError>` and `From<ParallelError>`, each mapping every error to the exact `GridError` variant produced today. The re-export of `GridError` (and `GridError`'s `Result`) from tetraloma-parallel ends; `tetraloma` re-exports `GridError`, `Result` and `ConnKey` from tetraloma-topology.
  - **Section 7, Listing S1b:** `ReduceOp` is defined in tetraloma-comm and re-exported by tetraloma-device and tetraloma-parallel.
  - **Section 9, Listing T1:** `ConnKey` is defined in tetraloma-topology (and re-exported by tetraloma).
  - **Section 10, "Geometric factors":** `Strategy` and the reader of `degree-policy.toml` stay in tetraloma-geometry (where they live in code today) as `#[doc(hidden)] pub` items shared with tetraloma-function, beside the hidden device basis evaluator; ADR-0071's planned move of them to tetraloma-device is cancelled.
  - **Section 11, Listing S2c:** the listing is split.
    - **tetraloma-comm** holds `CommDyn`, `Comm`, `CommExt`, `DType`, `CommScalar`, `ReduceOp`, `Serial`, `Mpi` (feature `mpi`), `Neighbourhood`, `owner_hash`, `local_gpu_index`, `CommError` and `Result<T, E = CommError>`. `CommError` (`#[non_exhaustive]`) holds the exact 5 variants those items return today: `TooLarge`, `LengthMismatch { expected: usize, found: usize }`, `IndexOutOfRange { index: usize, len: usize }`, `MessageTooLarge { count: usize }` and `RemoteFailure { rank: usize }`.
    - **tetraloma-parallel** holds `IndexMap`, `Scatterer`, `ReverseOp`, `ScatterNumeric`, `Transport`, `ParallelError` and `Result<T, E = ParallelError>`, and re-exports the public items of tetraloma-comm (except `tetraloma_comm::Result`). `ParallelError` (`#[non_exhaustive]`) covers the exact 8 variants `IndexMap` and `Scatterer` return today (`TooLarge`, `AllocationTooLarge`, `UnsupportedPrecision`, `LengthMismatch`, `Server(ServerError)`, `IndexOutOfRange`, `MessageTooLarge`, `RemoteFailure`) with `From<CommError>`, `From<DeviceError>` and `From<ServerError>`.
  - **Section 11, "MPI bindings":** the safe wrappers over `mpi::ffi` (`MPI_Dist_graph_create_adjacent`, `MPI_Neighbor_alltoallv` and `MPI_Comm_split_type`) live in tetraloma-comm; counts above `i32::MAX` return `CommError::MessageTooLarge`.
  - **Section 12, Listing S3:** `Partitioner::partition` returns `tetraloma_partition::Result<Vec<i32>>` with `PartitionError`. `PartitionError` (`#[non_exhaustive]`) holds the exact 7 variants tetraloma-partition returns today — `PartitionerFailed { name: &'static str, code: i32 }`, `NeedsMpiCommunicator(&'static str)`, `TooLarge`, `LengthMismatch { expected: usize, found: usize }`, `IndexOutOfRange { index: usize, len: usize }`, `MessageTooLarge { count: usize }` and `RemoteFailure { rank: usize }` — with `From<CommError> for PartitionError`. All 7 remain `GridError` variants; `tetraloma` (in `tetraloma/src/pipeline/dual_graph.rs`, where `Partitioner::partition` is called) converts `PartitionError` to `GridError` via a helper function (not `From`, because tetraloma-topology does not depend on tetraloma-partition).
  - **Section 14, "Site", "API documentation" and "Checks":** twelve published library crates, adding tetraloma-comm; in "API documentation" item 3, tetraloma-comm and tetraloma-partition dev-depend on `tetraloma-comm-testing`, and tetraloma-device and tetraloma-parallel dev-depend on `tetraloma-testing-base`.
  - **Section 16:** test support below the mesh comes from `tetraloma-comm-testing` (host-only) and `tetraloma-testing-base` (device); the foundation crates are tested without the facade.
  - **Section 18, crate table:**
    - new rows: `tetraloma-comm` (depends on `bytemuck`; `mpi` optional; dev: `proptest`, `tetraloma-comm-testing`), `tetraloma-comm-testing` (unpublished; depends on `tetraloma-comm`, `mpi` optional), and `tetraloma-testing-base` (unpublished; depends on `tetraloma-comm-testing`, `tetraloma-device` with `testing`, `tetraloma-parallel`, `cubecl` with runtime features, `mpi` optional);
    - changed rows: `tetraloma-device` (adds `DeviceError`, `FieldScalar`, `tetraloma-comm` without `mpi`, `bytemuck`, `num-complex` with feature `bytemuck`; drops `GridError`, `ConnKey` and the planned `Strategy`/degree-policy reader; dev: `proptest`, `tetraloma-testing-base`), `tetraloma-parallel` (depends on `tetraloma-comm` forwarding `mpi`, `tetraloma-device`, `bytemuck`; drops `GridError` re-export; adds `ParallelError`; dev: `proptest`, `tetraloma-testing-base`), `tetraloma-partition` (depends on `tetraloma-comm` instead of `tetraloma-parallel`, forwarding `mpi` under `ptscotch`/`parmetis`, making it host-only; adds `PartitionError`; dev: `proptest`, `tetraloma-comm-testing`), `tetraloma-topology` (adds `GridError`, `Result`, `ConnKey`), `tetraloma-geometry` (keeps `Strategy` and the degree-policy reader as `#[doc(hidden)] pub` items shared with `tetraloma-function`), `tetraloma-io` (note: does not import `tetraloma-partition` in code today; its unused `partition` dependency can be dropped), `tetraloma-function` (re-exports `FieldScalar` from `tetraloma-device`, reads `Strategy` and the degree-policy reader from `tetraloma-geometry`), `tetraloma` (re-exports `GridError`, `Result`, `ConnKey` from `tetraloma-topology`; converts `PartitionError`), and `tetraloma-testing` (depends on `tetraloma-testing-base` and re-exports its items and `ThreadComm`).
  - **Section 18, "Dependency rules":**
    - **rule 4:** add `tetraloma-testing-base` beside `tetraloma-testing` as an allowed carrier of CubeCL runtime features;
    - **new rule 9:** `tetraloma-comm`, `tetraloma-device`, `tetraloma-parallel` and `tetraloma-partition` depend on no mesh crate (`tetraloma-dims`, `tetraloma-refcell`, `tetraloma-topology`, `tetraloma-geometry`, `tetraloma-io`, `tetraloma-element`, `tetraloma-function`, `tetraloma`), and `tetraloma-comm` and `tetraloma-partition` depend on no `cubecl` crate.
  - **Section 18, "Cargo features":** list `tetraloma-testing-base` beside `tetraloma-testing` under **Runtime features** and **Test-support feature** (`tetraloma-device/testing`).
  - **Section 18, "CI matrix", rows `cpu` and `docs`:** `cpu` runs the rule-9 and `--no-cubecl` checks and the public-name check; `docs` covers twelve library crates.
  - **Section 22, "Conventions":** the CubeCL runtime-features bullet adds `tetraloma-testing-base`; the FFI rule names `tetraloma-comm` (in place of `tetraloma-parallel`) for MPI; the user-documentation rule covers twelve published library crates.
  - **Section 23, "Protected paths":** no row changes. `tetraloma-device/vector-policy.toml` and `tetraloma-geometry/degree-policy.toml` keep their paths and contents.
  - **Section 26:** the `ElementError` conversion note updates "tetraloma-device depends on neither crate" to "tetraloma-topology does not depend on tetraloma-element" (fields unchanged).
  - **Section 28, Listing F1:** `UnsupportedElement` and `SpaceMismatch` are added to `GridError` in tetraloma-topology (not tetraloma-device).
  - **Section 29, Listing F2:** `FieldScalar` is defined in tetraloma-device and re-exported by tetraloma-function.
- **Interfaces and tests:**
  - New public items: the crate `tetraloma-comm`; `CommError` (and `tetraloma_comm::Result`), `DeviceError` (and `tetraloma_device::Result`), `ParallelError` (and `tetraloma_parallel::Result`), `PartitionError` (and `tetraloma_partition::Result`); `FieldScalar` in `tetraloma-device`.
  - Unchanged for users of `tetraloma`: every path `tetraloma` and `tetraloma::prelude` export, every `GridError` variant, every numerical result.
  - New unpublished test crates: `tetraloma-comm-testing`, `tetraloma-testing-base`, and the foundation-only test crate of M18.
  - New checks: `scripts/check-workspace-deps.sh` enforces rule 9 and checks `--no-cubecl tetraloma-comm` and `--no-cubecl tetraloma-partition`; a new script checks the public type, trait, function and constant names of the four foundation crates on the generated API pages.
- **Process documents and scripts:**
  - `AGENTS.md`: "eleven published library crates" becomes twelve; the review checks dependency rule 9 (and rule 4's `tetraloma-testing-base`).
  - `.github/workflows/ci.yml`: the new `check-workspace-deps.sh` checks, the public-name check, and `tetraloma-comm` in the `docs` job.
  - `scripts/check-workspace-deps.sh`, `scripts/doc_audit.py`, `scripts/check-user-docs.sh`, `scripts/build-site.sh`, `scripts/check-docs.sh`: updated for `tetraloma-comm`, rule 9 and `tetraloma-comm-testing`/`tetraloma-testing-base`.
  - `docs/user/`: update crate references and documentation for `tetraloma-comm` and the split error types.
  - `docs/adr/README.md`: the ADR-0074 row.
- **Plans and tasks affected:** none. M11's open tasks (#71–#75) touch tetraloma-refcell, tetraloma-element, tetraloma-examples, conformance/ and doc/fixture scripts only, and M12–M17 are not planned yet. In `docs/roadmap.md`, M14 changes from `Depends on: M12` to `Depends on: M12, M18`.
- **New tasks:** milestone M18 in `docs/roadmap.md`, below. Its plan and issues follow the plan-milestone skill.

### Roadmap entry

```markdown
## M18: Mesh-independent device and communication crates

Depends on: none
Design: 7, 9, 11, 12, 14, 16, 18, 22, 29
Acceptance:
- `scripts/check-workspace-deps.sh` fails if the normal dependencies of tetraloma-comm, tetraloma-device, tetraloma-parallel or tetraloma-partition include a mesh crate of dependency rule 9.
- `scripts/check-workspace-deps.sh --no-cubecl tetraloma-comm` and `scripts/check-workspace-deps.sh --no-cubecl tetraloma-partition` pass.
- No public type, trait, function or constant of tetraloma-comm, tetraloma-device, tetraloma-parallel or tetraloma-partition has `Grid`, `Mesh`, `Cell`, `Conn` or `Dof` in its name, checked on the generated API pages.
- Every error that tetraloma-comm, tetraloma-device and tetraloma-parallel can return converts through `From` — and every error that tetraloma-partition can return converts through `tetraloma`'s conversion helper — into the `GridError` variant it produced before the milestone, checked variant by variant.
- `tetraloma-device/vector-policy.toml` is byte-identical to its content before the milestone.
- A workspace test crate whose normal dependencies hold no workspace crate other than tetraloma-comm, tetraloma-device, tetraloma-parallel, tetraloma-comm-testing and tetraloma-testing-base runs fill, copy, reduce, scan, radix sort, gather and segmented reduce on 10^6 elements on the CPU runtime, exactly for i32 and u32 and to 10^−5 (f32) and 10^−12 (f64) relative for floats.
- In that crate, `Scatterer::forward` and `Scatterer::reverse` with Add, Min, Max and Insert equal a host reference on 1–4 ranks of `ThreadComm`, for f32, f64, i32 and u32 and block sizes 1–3, exactly for integers and forward exchanges and to the float tolerances above for reverse.
- The same scatter tests pass on 1–4 ranks under MPICH and Open MPI.
- tetraloma-device exports `FieldScalar` for f32, f64, `Complex<f32>` and `Complex<f64>`, with `PARTS` equal to 1 and 2.
- `scripts/check-user-docs.sh` passes with tetraloma-comm among the published library crates.
- The standing matrix passes unchanged.

Out of scope: an open set of kernel families (`KernelKind` keeps its mesh and function entries, and `VectorPolicy::quad_tile` stays unchanged); scatter begin and end; point-to-point messages; communicator splitting; GPU-aware transport; any change to kernels, policies or numerical behaviour.
Runs: none
```

## Decision

> Accept ADR 74 with options as recommended in the ADR. I confirm the direction from the maintainer bullets.

2026-11-23, said to the agent in a session. Accepted **Option 3** with **P1a, P2a, P3a, P4a, P5a, P6a and P7a**, and confirmed the two maintainer-direction bullets.

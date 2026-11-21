# tetraloma roadmap

The order in which `docs/design.md` is built, as milestones with testable acceptance criteria. ADR-0069 moved the roadmap here from the design.

Rules for new entries (from M11; ids below M11 are not used again):

- The heading is `## M<n>: <title>`. An id never changes and is never reused.
- The fields Depends on, Design, Acceptance, Out of scope and Runs appear in that order.
- Depends on lists M11+ ids, or `none`: every milestone before M11 is merged.
- Design lists the section numbers of `docs/design.md` the milestone implements.
- Each acceptance bullet is one sentence that one automated test can decide, with its range and tolerance stated.
- Out of scope names what the milestone leaves out; Runs names the machines or long runs it needs, or `none`.
- A milestone is roughly four to eight tasks. No status, checkboxes or dates.

The milestones before ADR-0069 follow, as the design stated them (E3 had no roadmap entry; its plan is `docs/plans/E3.md`).

## Milestones before ADR-0069

Each milestone is one Claude Code work package. Every acceptance test is automated and runs on the targets of "Testing and validation", and later milestones depend only on the public interfaces of earlier ones. E0 sets up and checks the development environment before any library code exists. M1a (type layer) precedes M1b (reference cells), because tetraloma-refcell builds on tetraloma-dims. M3s measures where SIMD vectors pay off once the primitives and mesh kernels exist. M9h studies the degree-dependent kernels up to p = 8 before M10 benchmarks the final kernels (ADR-0041).

| Milestone | Depends on |
| --- | --- |
| E0 Development environment | – |
| M0 Workspace and CI | E0 |
| M1a Type layer | M0 |
| M1b Reference cells | M1a |
| M2 Device primitives and communication core | M0 |
| M3 Serial mesh core | M1b, M2 |
| M3s SIMD vectorization and benchmarks | M3 |
| M4 Gmsh I/O (serial) | M3 |
| M5 VTK output | M3 |
| M6 Parallel core | M3 |
| M7 Partitioners | M6 |
| M8 Parallel I/O | M4, M5, M7 |
| M9 Submeshes and manifolds | M6 |
| M9h High-order performance | M9 |
| M10 Hardening | all |
| M10c CPU vector policy | M10 |
| E1 Rename to tetraloma | M10c |
| E2 Real MPICH runs in hosted CI | E1 |
| D1 User documentation | E1; E2 for its MPICH acceptance item (ADR-0068) |

### E0 Development environment

**Scope:**

- the targets of "Development environment and test targets": the M3 Max, the Myriad V100 and A100 nodes, and Locust;
- `rust-toolchain.toml` with Rust 1.99 at the repository root, and a `.gitignore` that covers build output, `target/hpc/` and `scripts/hpc/targets.env`;
- `tools/env-check/`, a standalone crate outside the future workspace. For each runtime compiled in (`cpu`, `metal`, `cuda`) it:
    - opens the device and records its name and properties (f64 and u64 support, `max_page_size`, `max_bindings`, `max_cube_count`, plane sizes, `load_width`, `vector_register_count`, `max_units_per_cube`, and `io_optimized_vector_sizes` for f32 and f64);
    - runs an element-wise add over 10^7 elements in f32 and, where supported, f64, at vector size 1 and at the widest size the device offers, with `calculate_cube_count_elemwise`;
    - runs a u32 atomic histogram;
    - with its `mpi` feature, checks an allreduce of rank ids;
    - checks the results and writes a JSON report;
- `scripts/hpc/`: `remote-test.sh` with the interface above, one job script per target, `targets.example.env`, `status.sh` and `cancel.sh`;
- `docs/dev-environment.md`, filled in with everything E0 finds.

**Acceptance:**

- **M3 Max:**
    - env-check passes on the CPU runtime (f32, f64) and on Metal (f32, with f64 reported as unsupported);
    - a 4-rank MPI allreduce passes;
    - `vtk` ≥ 9.2 and `meshio` import in Python;
    - the output of `gmsh --version` and `claude --version` (≥ 2.1.286) is recorded.
- **Myriad V100:** `scripts/hpc/remote-test.sh myriad-v100 -- cargo run --release --manifest-path tools/env-check/Cargo.toml --features cuda` runs from the M3 Max without interaction.
    - The report shows sm_70, a CUDA 12.x toolkit no newer than the driver's CUDA version, and passing f32 and f64 kernels.
    - With `--gpus 2`, both GPUs pass and a 2-rank MPI allreduce passes within the node.
    - Whether the MPI is CUDA-aware is recorded.
- **Myriad A100:** the same, with sm_80 and the newest toolkit the driver supports (CUDA 13 if available).
- **Locust:** the same, with sm_90 on an aarch64 host; env-check also passes on the CPU runtime on Grace. If Locust is unavailable, this item is recorded as pending and does not block M0.
- **Runner robustness:**
    - a deliberately failing command exits non-zero and prints its first error;
    - a run that exceeds `--max-wait` is deleted and exits with status 124;
    - no job is left in `qstat` after the acceptance runs.
- **Turnaround:** queue wait and run time are recorded for each remote target, as the median of three runs.
- **Record:** `docs/dev-environment.md` has no open fields left, apart from Locust details that ARC cannot provide.

### M0 Workspace and CI

**Scope:** the workspace of "Workspace layout" as skeleton crates; `rust-toolchain.toml` with Rust 1.99; `cubecl = "=0.11.0"` in `[workspace.dependencies]`; tetraloma-testing with `test_client()`; the tetraloma-mpi-tests skeleton; the hosted CI jobs cpu, mpi-mpich, mpi-openmpi and vulkan, plus skeletons for the conformance and ptscotch jobs. tetraloma-testing selects the device from `TETRALOMA_TEST_DEVICE` both locally and inside `scripts/hpc` jobs.

**Acceptance:**

- an element-wise add kernel in the workspace passes on the CPU runtime (f32, f64) and Metal (f32) on the M3 Max, on Vulkan (f32) in hosted CI, and on CUDA (f32, f64) on a Myriad V100 through `scripts/hpc`;
- a 4-rank MPI smoke test (allreduce of rank ids) passes under MPICH and Open MPI;
- the conformance job builds and links ndelement at f8cbffe and compares one table (triangle edges);
- `cargo test --workspace` builds neither ndelement, BLAS nor MPI, checked with `cargo tree`.

### M1a Type layer (tetraloma-dims)

**Scope:** Listings B1 and B2. This includes `CellType`, `Cell1`–`Cell3`, `TopoDim` and the `WrongCellDim` conversions, plus a test-only stub `Mesh` with Listing B4's impl bounds.

**Acceptance:**

- **compile-fail tests:**
    - `normals()` on a volume mesh gives E0599;
    - a `Cell3` in a `CellBlock<D2>` gives E0308;
    - `boundary()` on a curve mesh gives E0599;
    - the `.stderr` files are generated with `TRYBUILD=overwrite` and reviewed.
- **compile-pass tests:**
    - `normals()` on surface and planar-curve meshes;
    - `boundary()` on volume and surface meshes;
    - `boundary().boundary()` on a volume mesh.
- `CellType` ↔ `CellN` round trips for every cell; a wrong dimension returns `WrongCellDim`.

### M1b Reference cells (tetraloma-refcell, tetraloma-conformance)

**Scope:**

- tables A.1–A.4 for the eight cells, generated by the lexicographic rule;
- Lagrange layouts for any p, including the prism and pyramid rules of A.5;
- the DefElement pyramid space, with an orthonormal rationomial basis;
- point families: equispaced on every cell, GLL on tensor cells;
- quadrature, and tabulation with tetraloma-refcell's own LU;
- orientation bits (A.6) and the format permutation generators (A.9);
- the conformance crate with fixture export (A.10).

**Acceptance:**

- node counts match A.4 for p ≤ 10;
- partition of unity and reproduction of each element's own space to 10^−12 for p ≤ 8 (f64);
- sub-entity tables equal ndelement's for all eight cells;
- orientation bits equal `compute_orientation` for 10 000 random vertex numberings per cell type with D_TOP ≥ 2;
- point sets equal ndelement's node by node for intervals, triangles, quadrilaterals, tetrahedra and hexahedra, p ≤ 4, equispaced and (tensor cells) GLL;
- the generators reproduce every A.9 row and DOLFINx's Gmsh P3 tables (tet20 after A.11; quad16, hex64, prism40 and pyramid30 unchanged);
- quadrature integrates monomials (rationomials on pyramids) up to its degree to 10^−13;
- `cargo test --workspace` still builds neither ndelement nor BLAS.

### M2 Device primitives and communication core (tetraloma-device, tetraloma-parallel)

**Scope:**

- `Real` and the `DeviceBuffer` family with limit checks and `VECTOR_PAD` padding, plus `check_precision`;
- `LaunchPlan` and `VectorPolicy`, with vector size 1 and tile width 16 everywhere until M3s, and the `TETRALOMA_VECTOR_SIZE` and `TETRALOMA_QUAD_TILE` overrides;
- scan, radix sort (u32-word keys with u32 payload), unique-by-key, run-length, histogram, gather/scatter, segmented reduce, fill and copy. Fill, copy, reduce, the tile-local scan phase, the digit counting of the sort (SortDigits) and gather are generic over the vector size (ADR-0009);
- `CommDyn`, `Comm`, `CommExt` and `Serial`, and `IndexMap` without ghosts.

**Acceptance:**

- property tests against CPU references, with sizes up to 10^5 and 256 cases plus fixed sizes of 10^6 and 10^7. They run on the CPU runtime, and on Vulkan and Metal with u32 keys and f32 values;
- results are bitwise identical across two runs;
- the throughput of sorting 10^7 two-word keys is recorded per backend;
- every vector-generic primitive gives the same results at vector size 1 and at every larger power of two up to 16 that the device offers, for lengths 0, 1, 15, 16, 17 and 10^6 + 3: integers bitwise, float reductions within the tolerances;
- on the CPU runtime, `LaunchPlan` uses one unit per hardware thread and blocks of whole 128-byte lines; with injected device properties, no plan exceeds `max_units_per_cube`, `max_cube_dim` or `max_cube_count` (property test);
- with an injected limit, an allocation above `max_page_size` returns `AllocationTooLarge`;
- an f64 request on Metal returns `UnsupportedPrecision`.

### M3 Serial mesh core

**Scope:**

- `Mesh` from host arrays, built with origin ids;
- device entity creation ("Topology construction"), connectivity and permutation info;
- geometry blocks, `QuadRule` and `FacetQuadRule`;
- E-vectors of the geometry nodes, built on demand and cached per geometry block, and the tiled `QuadField`;
- factor kernels (X, J, det J, K, facet normals, normals), generic over the vector size, and `measure`;
- `AnyMesh`, `AnyMeshRef`, `with_mesh!` and `DeviceGrid` (change log, BLK-3); `boundary()` is declared and returns `NotYetImplemented` until M9 (ADR-0012);
- the tetraloma-testing mesh generators;
- the real `Mesh` replaces the M1a stub in the compile-fail tests.

**Acceptance:**

- closed-form entity counts and Euler characteristic 1 for boxes of every cell type and the mixed box, at three refinements;
- unit volume for p = 1–4, to 10^−12 in f64 and 10^−5 in f32;
- sphere-area convergence rate ≥ p+1 − 0.2 for p = 1–4 over four refinements (f64);
- f32 results agree with f64 to 10^−5 relative;
- Jacobians on a mixed triangle–quadrilateral surface mesh use one `QuadRule` and match a host reference to 10^−12;
- permutation info equals the conformance fixtures for every cell;
- with an injected limit, `QuadField` results are identical with 1 and 7 chunks;
- factor kernels on E-vectors agree with a scalar L-vector reference kernel at every vector size from 1 up to 16 that the device offers and the tile holds, and at tile widths 1 and 16, bitwise on the CPU runtime, with +0 and −0 equal (ADR-0013), for the direct kernels and for strategies that sum over nodes in node order; within the tolerances for sum-factorised and cooperative kernels (ADR-0054), and within the tolerances on GPUs.

### M3s SIMD vectorization and benchmarks

**Scope:**

- tetraloma-bench, with the protocol of "Vectorization and CPU execution": vector sizes, tile widths, CPU block sizes, problem sizes, medians, compile times and JSON output;
- benchmarks of every candidate family that exists after M3: Fill, Copy, Reduce, ScanTile, SortDigits, Gather, TopologyKeys, PermutationInfo, EVector and GeomFactors;
- for TopologyKeys and PermutationInfo, the cell-fastest copy of the connectivity they need, built behind the policy and measured including the transpose;
- the decision rule applied for every family, device class and scalar type; `vector-policy.toml` and `docs/perf/vectorization.md` written.

**Acceptance:**

- benchmark JSON committed under `docs/perf/data/` for the M3 Max (CPU runtime, Metal), Locust (CPU runtime on Grace, CUDA) and one socket of a whole Kathleen node (CPU runtime on its AVX-512 Xeon; ADR-0020), plus a Myriad D node if Kathleen was unavailable, and Myriad's V100 and A100 nodes if Locust was unavailable (ADR-0014, ADR-0016);
- `docs/perf/vectorization.md` lists, for every candidate family, device class and scalar type, the measured speedup of each vector size over size 1, the chosen size and the reason;
- every vector size above 1 in `vector-policy.toml` is at least 10% faster than size 1 at 10^6 and 10^7 items on its device class (the geometric mean over its measured devices), and every chosen configuration (vector size, tile width, block size, CPU units) is within 3% of the fastest one the 10% rule admits (ADR-0015; for a shared 4-byte entry, at a size every type admits, ADR-0019), or, where none is, the closest one, at most 10% slower (25% for a shared 4-byte entry, ADR-0021) and marked in the report (ADR-0018);
- a test checks that `vector-policy.toml` covers every `KernelKind` and `DeviceClass`, and that every numeric value is a power of two from 1 to 16 and every `cpu_units` value is `all` or `performance` (ADR-0017);
- with the chosen policy, the whole test suite passes on every target, including the vector-size invariance tests;
- entities created per second and geometric factors per second are recorded on the M3 Max CPU runtime and one socket of a Kathleen node, with the chosen policy and with every kernel scalar (ADR-0016, ADR-0020).

### M4 Gmsh I/O (serial)

**Scope:** reader and writer, the element table (A.7), serendipity upgrade, MeshTags from physical groups by key matching, D_TOP and D_GEO inference, and origin ids from tags.

**Acceptance:**

- read → write → read gives identical topology (cell types and degrees, cell and vertex origin ids, names and tags keyed by origin ids), and coordinates equal to 10^−15 relative to the largest absolute coordinate (f64; 10^−5 in f32) (ADR-0022);
- the committed fixture corpus reads without error with correct node counts (file: Gmsh's node count; mesh: the nodes of D_TOP elements). It is written by a pinned Gmsh version and covers every element type up to order 4, pyramids up to order 3;
- nodes and elements agree with mshio on every fixture mshio can read; mshio 0.4.2 cannot read the 18-node prism (type 13), and the test asserts that it fails on exactly the files that contain it (ADR-0023);
- tag counts equal Gmsh's physical-group sizes;
- D_GEO is inferred correctly on 1D, 2D and 3D fixtures, and duplicate node tags are rejected.

### M5 VTK output

**Scope:** VTU (version 2.2) with linear and Lagrange cells, cell and point data, the PVTU skeleton and the pyramid fallback, and the device GLL → equispaced conversion for both writers.

**Acceptance:**

- VTK's Python reader (pinned, ≥ 9.2) reads every fixture;
- the integrated volume or area of affine P2 and P3 meshes (`vtkIntegrateAttributes`) equals tetraloma's `measure` to 10^−10 (f64); on curved P2 and P3 meshes, VTK's `EvaluateLocation` at Gauss points equals `physical_points` to 10^−10 (f64) (ADR-0024);
- meshio reads identical raw arrays from the inline-base64 files (with and without zlib), and every appended file parses to the same arrays as its base64 twin (ADR-0025);
- a P2 pyramid mesh is written as linear cells with one warning.

### M6 Parallel core

**Scope:**

- `Mpi` and the `mpi::ffi` wrappers (distributed graph, neighbourhood all-to-all-v);
- `IndexMap` with ghosts, and `Scatterer` (host-staged, deterministic reverse); its pack and unpack kernels take their shape from `LaunchPlan` and are generic over the vector size (`KernelKind::Pack`);
- pipeline stages 1–10 from host arrays with the block partitioner, ghost modes, ownership rules, global numbering and locality ordering.

**Acceptance** on 1–4 ranks under MPICH and Open MPI:

- global entity counts and tag counts are identical to one rank, and total volume agrees to 10^−12 (f64) and 10^−5 (f32) relative (ADR-0026);
- canonical entity vertex lists (origin ids) and permutation bits per cell origin id are identical to one rank;
- a forward scatter of global ids reproduces the ghost global ids;
- a reverse Add of ones equals the number of ranks holding each index;
- two runs are bitwise identical;
- on a randomly permuted structured box, locality ordering at least halves the mean over owned cells of (max − min local index of the cell's owned nodes); ghost nodes, ordered by (owner rank, global index), are excluded (ADR-0027).

### M7 Partitioners

**Scope:** vertex weights on M6's distributed dual graph (ADR-0026), ptscotch-sys and its wrapper, parmetis-sys and its wrapper (feature-gated), METIS on rank 0 (vendored) and Scotch on rank 0.

**Acceptance** on a 10^6-cell tetrahedral box and a 2 · 10^5-cell mixed hexahedron/prism/pyramid mesh, at 4 and 8 ranks (8-rank runs on one Kathleen node through `scripts/hpc`; ADR-0016), read as in ADR-0029 (imbalance for every external partitioner; cut(PT-Scotch) ≤ 1.10 · Scotch's and cut(METIS) ≤ 1.25 · Scotch's; reproducibility per partitioner and rank count; ADR-0029, ADR-0032):

- weighted imbalance ≤ 5%;
- PT-Scotch's edge cut is within 10% of sequential Scotch on the same graph, and METIS's within 25% of Scotch;
- a fixed seed reproduces the partition for a fixed rank count.

### M8 Parallel I/O

**Scope:** chunked binary Gmsh reading, partitioned writing, PVTU with `vtkGhostType`, and VTKHDF (hdf5-metno with `mpio`).

**Acceptance** (read as in ADR-0035: bullet 1 item 14, bullet 2 item 13, bullet 3 item 15):

- the global mesh, compared by origin ids, is identical for 1–8 ranks;
- reading a 10^7-cell binary file on 8 ranks of one Kathleen node keeps peak resident memory per rank below twice the rank's final mesh size plus 256 MiB (measured with getrusage);
- VTKHDF read back by VTK's Python reader equals the PVTU content;
- after VTK removes ghost cells, every cell appears exactly once.

### M9 Submeshes and manifolds

**Scope:** boundary, facet, edge and cell submeshes with `EntityMap` (`boundary()` replaces M3's `NotYetImplemented`; ADR-0039); distributed manifold orientation (`orient`); the manifold tests of normals, pseudo-inverse and facet normals, whose kernels exist from M3 (ADR-0012); pull-back, generic over the vector size.

**Acceptance:**

- normals on the boundary of a centred ball satisfy n·x > 0 at every quadrature point;
- ∮ x·n = D_GEO · V to 10^−10 (f64);
- `orient(Outward)` on a closed sphere with randomly flipped cells recovers outward normals on 1–4 ranks;
- error cases:
    - an edge with three cells gives `NonManifold`;
    - interior facets in `facet_submesh` give `InteriorFacet`;
    - `Outward` on an open surface gives `NeedsReferencePoint`;
    - a Möbius strip gives `NonOrientable`;
- pull-back recovers reference points to 10^−12 for p = 1–3;
- pull-back gives the same reference points at vector size 1 and at the widest size the device offers, to 10^−12 (read as in ADR-0039 item 14: bitwise on the CPU runtime).

### M9h High-order performance

**Scope (ADR-0041):**

- degree sweeps (p = 1–8) in tetraloma-bench, with the protocol of "Vectorization and CPU execution", for every degree-dependent kernel family: EVector, GeomFactors (points, Jacobian, det J, pseudo-inverse, normals, facet normals), PullBack and the device basis evaluator; with, per kernel and device, the time per cell and per quadrature point, the first-use compile and load time, and on CUDA the registers and local memory per thread;
- strategies, each compared with M9's direct evaluation: runtime loops instead of full unrolling (bounding code size and JIT time); sum factorisation for quadrilaterals, hexahedra and prisms; tabulations or coefficients in shared memory; a workgroup cooperating on one cell only where these fall short, with fixed-order reductions that do not depend on the plane size;
- a degree policy: the strategy of each family chosen per degree range and device class from the measurements, recorded in `docs/perf/high-order.md`, with an override for tests (as `TETRALOMA_VECTOR_SIZE` is for vector sizes);
- C18 closed: its cause isolated with the CUDA kernel attributes, and reported upstream if it is CubeCL's.

**Acceptance** (read as in ADR-0042: bullet 1 item 2, bullet 2 item 7, bullet 3 item 1, bullet 4 item 11, bullet 5 items 2, 8 and 9, bullet 6 item 12, and ADR-0050 to ADR-0053 and ADR-0055):

- every cell type except pyramids, at p = 1–8 and with both node families where offered: physical points, Jacobians, det J, pseudo-inverses, normals and facet normals from the device match the host reference (tetraloma-refcell's f64 tabulation) to 10^−12 (f64) and 10^−5 (f32), and the pull-back recovers reference points to the same tolerances; on the CPU runtime, Metal (f32) and CUDA (Locust); pyramids as before (p ≤ 3, experimental above 2);
- every kernel variant up to p = 8 compiles, loads and launches on each backend, and the first-use compile and load time of each is at most 30 s on Locust;
- for quadrilaterals and hexahedra, the time per quadrature point of geometric factors and of a pull-back Newton step at p = 8 over that at p = 4 is measured and reported per device class against a target of 3 (direct evaluation: about 5.8 times), with the speedup over M9 at p = 8 (ADR-0053);
- no policy family is more than 5% slower than in M9 at p ≤ 4 on any device class, in a paired comparison where M9's direct kernel is selectable (ADR-0055);
- the determinism, vector-size invariance and parallel invariance tests pass at p = 1–8: bitwise on the CPU runtime, to 10^−12 (f64) and 10^−5 (f32) on GPUs;
- `docs/perf/high-order.md` lists, for every family, device class and degree range, the measured cost of each strategy, the chosen one and the reason, with benchmark JSON under `docs/perf/data/` from the targets of M3s (Locust's Grace CPU: the drift baseline and the paired check only, ADR-0051).

### M10 Hardening

**Scope:** a hardened host-staged halo exchange and `local_gpu_index` (ADR-0026, ADR-0059), benchmarks (including a re-run of the M3s vectorization study for every kernel family and a check of M9h's degree policy, both with the candidate protocol of "Benchmarks"; ADR-0057), ending with four generic parameter sets, CPU and GPU for f32 and f64 (ADR-0058), documentation and examples.

**Acceptance** (read as in ADR-0057, ADR-0058, ADR-0059 and ADR-0060):

- the host-staged halo exchange (`forward` and `reverse`, halos of 10^3, 10^4 and 10^5 nodes with block 3) is recorded on the local Mac (CPU runtime, Metal) and Locust (CUDA, 1 and 2 ranks); for 10^5 nodes, its host-to-device copy takes at most 1.5 times its device-to-host copy on Metal and on CUDA with one process per GPU (on the CPU runtime, whose read returns a view of the runtime's storage, and on CUDA with two ranks sharing one GPU, both are recorded and docs/perf/halo-exchange.md reports the ratio with its cause; ADR-0061, ADR-0062), and `reverse` at most twice `forward` on each, and results stay bitwise identical to M9h's (ADR-0059);
- for quadrilaterals and hexahedra, the time per point of geometric factors and of a pull-back Newton step at p = 8 is at most 3 times that at p = 4 on every benchmark platform under the generic degree policy, each degree under its own choice (ADR-0042 item 1 as amended by ADR-0064, ADR-0053, ADR-0058), or the gap is reported with its cause;
- benchmarks (entities created per second, geometric factors per second at p = 1, 4 and 8; ADR-0041) are recorded on the local Mac (CPU runtime, Metal), Locust and one socket of a whole Kathleen node (AVX-512; ADR-0020), plus a Myriad D node if Kathleen was unavailable, and Myriad's V100 and A100 nodes if Locust was unavailable (ADR-0014, ADR-0016);
- the M3s study covers every candidate `KernelKind`, including Pack, PullBack and any kernel family M9h adds (ADR-0041), and the degree-dependent families are measured at p = 1, 4 and 8, each with the candidate protocol of "Benchmarks" (ADR-0057); `vector-policy.toml`, `degree-policy.toml`, `docs/perf/vectorization.md` and `docs/perf/high-order.md` are updated from these runs on the benchmark platforms, and from M9h's records between those degrees;
- `DeviceClass` has exactly the variants `Cpu` and `Gpu`. A test checks that `vector-policy.toml` and `degree-policy.toml` hold exactly four sets, Cpu and Gpu for 4-byte and 8-byte scalars, and that each entry follows the generic decision rule of "Benchmarks": no platform is more than 25% slower than its own best candidate, unless the report marks the entry with its worst case (ADR-0058);
- every example runs on the CPU runtime and on at least one GPU backend;
- rustdoc builds without warnings, and every public item is documented.

### M10c CPU vector policy

**Scope (ADR-0065):** the `[Cpu]` vector set re-derived with interleaved timing in two processes over a grid of widths, blocks and units; the generic rule's fallback without the admission gate, applied to all four sets.

**Acceptance:**

- every `[Cpu]` vector entry is measured on the three Cpu platforms with the protocol and grid of "Benchmarks", and the records carry both processes; `docs/perf/vectorization.md` gives each platform's noise between processes;
- `vector-policy.toml` and `degree-policy.toml` follow the generic decision rule, with the fallback of ADR-0065, on the committed records; every entry that misses the bound is marked with its worst case and the cause;
- the standing matrix passes with the new sets.

### E1 Rename to tetraloma

**Scope (ADR-0066):** the project's permanent name, tetraloma, throughout: crate names and directories, Rust paths, environment variables (`TETRALOMA_*`), scripts, CI, documentation and the Claude Code configuration; the repository moves to github.com/tetracl/tetraloma. No type, function or behaviour changes.

**Acceptance:**

- `scripts/check-name.sh` passes, in CI: no old name in tracked paths or content outside the allowlist of ADR-0066, every workspace crate except the two `-sys` crates is named `tetraloma` or `tetraloma-*`, the workspace manifest names the repository, and only `TETRALOMA_*` variables are read;
- the standing matrix passes unchanged.

### E2 Real MPICH runs in hosted CI

**Scope (ADR-0068):** hosted CI's MPICH jobs start every rank as a singleton (Ubuntu 24.04's MPICH 4.2.0 is built with PMIx, which its hydra launcher does not serve; Launchpad bug 2072338), so the MPICH half of "under both MPICH and Open MPI" has not run on more than one rank. E2 adds a rank-count guard: `scripts/mpi-test.sh` and `scripts/run-examples.sh` pass the requested rank count to every rank (`TETRALOMA_EXPECT_RANKS`), and `tetraloma_mpi_tests::run` and the MPI examples fail at once if the world size differs. The jobs `mpi-mpich` and `mpi-mpich-hdf5` move to `ubuntu-26.04` with Ubuntu's MPICH 4.3.2 and its parallel HDF5 (within the supported 1.10.10–2.2.0); the other jobs stay on `ubuntu-24.04`. Failures that real multi-rank MPICH runs expose are fixed in E2. Fallback if `ubuntu-26.04` breaks those jobs: Debian's fixed MPICH `4.2.0-5.1` on `ubuntu-24.04`. No public API changes; tetraloma-mpi-tests and tetraloma-examples (unpublished) gain the check.

**Acceptance:**

- the guard fails a singleton launch (checked once under Ubuntu 24.04's MPICH);
- `scripts/mpi-test.sh 1 2 3 4` and the hdf5 targets pass under MPICH in CI with the guard;
- the MPI examples pass on 1–4 ranks under MPICH in CI with the guard;
- the Open MPI jobs pass unchanged, with the guard, in CI and locally.

### D1 User documentation

**Scope (ADR-0067):** the user documentation of "User documentation": the book in `docs/user/` with the contents listed there, its figures and its programs in tetraloma-examples; the doc tools; the site at `https://tetracl.github.io/tetraloma/` with the API of the nine published library crates, built and deployed by CI; the API documentation of those crates rewritten for users; and the checks of `scripts/check-user-docs.sh`. No public item, signature or behaviour changes; only doc comments change in the library crates.

**Acceptance:**

- `scripts/check-user-docs.sh` passes in CI;
- the book's doctests pass on the CPU runtime;
- the book's programs pass on the CPU runtime, Metal and CUDA in f32 and, where supported, f64, and the MPI programs on 1–4 ranks under MPICH and Open MPI;
- the library sources differ from the base commit only in comments;
- the site is deployed from `main`;
- the hosted CI jobs pass.

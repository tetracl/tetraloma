# ADR-0077: Sections 18, 23, 26 and A.10 after the M11 gate

Status: Proposed
Date: 2026-11-24
Decision issue: #119
Raised by: the M11 gate, findings G1–G4, G9 and G10 (`docs/reviews/M11.md`)

## Context

The gate of M11 (at `bff9f18`) found six places where `docs/design.md` disagrees with the code M11 merged or with the user book, or lacks a rule that M12 to M14 build on. The maintainer chose to handle them in one ADR before M12 is planned: "One ADR before M12 plan (Recommended)" (said to the agent in a session, 2026-11-24). No finding needs a code change; the design text has to catch up.

### 1. Applying the transformations (G1)

- Section 26, "DOF transformations", "Application": "Let T_c map the reference basis to cell c's basis. Evaluation applies T_c, transposed operations apply T_cᵀ, and interpolation applies T_c^{−ᵀ}. A signed permutation is orthogonal, so all three are the same signed permutation."
- For an orthogonal T_c, T_c^{−ᵀ} = T_c, but T_cᵀ = T_c^{−1}: the inverse permutation, which differs from T_c unless T_c is an involution. Lagrange elements have rotations of order 3 on triangular faces (from p = 4 on tetrahedra: `base_transformation(Triangle, Rotation)` is the permutation `[2, 0, 1]`, its transpose `[1, 2, 0]`) and of order 4 on quadrilateral faces (from p = 3 on hexahedra).
- The book (`docs/user/src/fem/dof-transformations.md`, M11.T6) states it correctly: M_c = T_cᵀ maps canonical global coefficients to cell-local ones; basis data and interpolation use T_c, the global-to-local map uses M_c.
- Section 26 also does not say which of the two `base_transformation` returns. The code returns the factors of M_c: "Applying a transformation `M` to a sub-entity's canonical DOF vector `x` produces the cell-local vector `y = M x`" (`tetraloma-element/src/lib.rs`, `BaseTransformation`).
- Section 28's signed-permutation fold, which M14 implements, reads this passage.

### 2. The degree of a tensor factor (G2)

- Section 26, "Tensor factors": "`LegendreL2`: moments against P_{p−1}; their factor in the remaining direction, and the factor of Legendre DG." `MomentH1`: "endpoint evaluations plus interior Legendre moments up to degree p − 2". Listing E1 stores a factor as `(Factor1d, u32)`, "kind and degree", without saying what the degree counts.
- The code stores `(LegendreL2, p)` for Legendre DG of degree p, and that factor spans P_p with p + 1 functions (`tetraloma-element/src/tensor.rs`: `build_lagrange_tensor_factors`; `tabulate_1d` returns `polyset::dim(Interval, d)` functions). Read with p as the factor's degree, the design's "moments against P_{p−1}" says the opposite.
- The maintainer fixed the convention in the plan of M11, Decision 17: "(Factor1d::LegendreL2, d) spans P_d (d + 1 functions), so Lagrange Legendre degree p stores (LegendreL2, p) and M12 RTCE_p/RTCF_p/NCE_p/NCF_p will store (LegendreL2, p - 1)". The question there said "design §26 leaves this open"; the gate found that the design states a convention, so the rule now lives only in a plan, where ADR-0072 found that later milestones do not look. `MomentH1`'s degree is not stated anywhere.

### 3. Contracts M12 to M14 rely on (G3)

M11 fixed these in tests, rustdoc, the book and the plan's Decisions; section 26 does not state them:

- `BaseTransformation::SignedPermutation` is a gather from canonical to cell-local order: y[i] = −x[perm[i]] if `negate[i]`, else x[perm[i]]; `negate` is indexed by the output (M11.T0, `accept.rs`; `lib.rs`).
- The per-cell composition: on an edge, `Reversal` once if its bit is set; on a face, reflection once if its reflection bit is set, then rotation as many times as its rotation count, u_local = M_R^rot M_S^refl u_global (`accept.rs`, the continuity test of A10; `dof-transformations.md`, "Neighbouring cells"). Section 26 says only "as in Scroggs, Dokken, Richardson and Wells". The order matters on quadrilateral faces: the review of M11.T0 found that this order passes the continuity check in every case, and ndelement's order of application (rotation first) does not.
- `base_transformation` is `None` when the cell has no lower-dimensional sub-entities of that type, the element has no interior DOFs on them, or the generator does not apply to the entity (Decision 13). Listing E1 says only "None if the element has no DOFs on sub-entities of that type".
- `entity_dofs` and `entity_closure_dofs` panic on an out-of-range (d, e); closure DOFs come by ascending sub-entity dimension, then index (Decision 7).
- `Element::get` caches errors per key as it caches elements; `Element::tabulate` checks the derivative order first, returns `ElementError::RefCell(PointsShape)` for a point array whose length is not a multiple of tdim, and an empty table for no points (Decision 11).
- `DenseLu::solve` takes `b` row-major n × nrhs, and n = 0 or nrhs = 0 with an empty `b` is a no-op (Decision 21, confirmed at the gate and added to the plan in the gate's pull request).
- Derives and trait impls beyond Listing E1: `Debug` on `Element`; `Display`, `std::error::Error` and `From<RefCellError>` on `ElementError`; `Clone, Debug, PartialEq` on `ElementTabulation` and `BaseTransformation`; `Clone, Debug, PartialEq, Eq` on `TensorFactors`; `Debug` on `DenseLu` (Decisions 7, 8, 12, 16).

### 4. The conformance crate's dependencies (G4)

- `conformance/Cargo.toml` depends directly on tetraloma-dims, tetraloma-refcell, rlst 0.6 (ndelement's array type, for the exporter's tabulations; M11.T1) and serde_json, besides ndelement and the BLAS/LAPACK crates.
- Section 18's row for `conformance/` lists "tetraloma-refcell (path), ndelement (git), blas-src, lapack-src, openblas-src". Listing A.1's manifest lacks tetraloma-dims and rlst, and its tree says the exporter "writes tetraloma-refcell/tests/fixtures/" only.
- The maintainer accepted the rlst dependency in a session on 2026-11-23 (recorded on pull request #84). The build gains no crate: rlst 0.6.1 was already in `conformance/Cargo.lock` through ndelement.

### 5. Oracles that nothing exports (G9)

- A.10, "Oracles for elements and grid functions": ndelement gives "Lagrange tabulations, DOF layouts and base transformations", and Basix "Isaac GLL points (`gll_isaac`), Legendre variants and base transformations".
- M11 exports ndelement's Lagrange tabulations and base transformations on the five cells ndelement offers, Basix's `gll_isaac` points, and Basix's Lagrange tabulations where ndelement has no element (`conformance/src/bin/export_fixtures.rs`, `scripts/fixtures/element/generate.py`). Nothing exports DOF layouts, the `Legendre` variant or Basix's base transformations, and no criterion of the roadmap (M11 to M17) asks for them.
- What checks those parts instead: `entity_dofs` and the closures follow tetraloma's own tables (A.4–A.6); base transformations on prisms and pyramids rest on the continuity test of A10 and the permutation test of A8; the `Legendre` variant on A6, A7 and A11.

### 6. Where the refcell fixtures come from (G10)

- Section 23, row 2, protects `tetraloma-refcell/tests/fixtures/` as "the reference-cell conventions exported from ndelement"; A.10, "Fixtures", says the conformance crate exports them.
- Since M11.T1, `tetraloma-refcell/tests/fixtures/gll_isaac.json` and `tetraloma-element/tests/fixtures/lagrange_basix.json` are written from Basix 0.11.0 by `scripts/fixtures/element/generate.py`, which refuses any other version. Its `--check` needs the pinned Python environment; hosted CI's conformance job runs only the exporter's `--check`. The M11 gate ran both locally.

## Options

1. **Keep the design as written.** Section 26 keeps a wrong statement that M14 will implement from, the `Factor1d` degrees stay defined only by a plan's Decision and the code, and the contracts of item 3 stay where M12 to M14's planners must look them up in M11's plan and rustdoc. Sections 18, 23 and A.10 keep describing the conformance setup as it was before M11. Every later review will find the same differences again.
2. **Correct and complete the text.** Section 26 states the application correctly, the degree of every kind of factor, the composition order and the contracts of item 3, in its prose and in Listing E1; sections 18, 23 and A.10 describe the conformance crate, the fixtures and the oracles as M11 built them. No signature, test, roadmap entry or plan task changes: the listing catches up with code that is merged and reviewed.
3. **As option 2, but keep A.10's oracle list** and add a task to M12 that exports the missing oracles (ndelement's DOF layouts; Basix's `Legendre` tabulations and its Lagrange base transformations on prisms and pyramids), with a criterion for each in M12's roadmap entry. It adds checks against an external definition where M11 has internal ones only, at the cost of about one task (exporter and generator extensions, fixtures and three tests) in a milestone whose subject is N1curl and RT.

## Recommendation

Option 2.

- Items 1 to 4 and 6 describe what is merged; leaving them is what costs, because M12 and M14 plan from section 26.
- For item 5, the internal checks are strong for the parts in question: the continuity test compares both sides of every shared entity for every relative orientation and every pair of cell types, and A6, A7 and A11 pin the `Legendre` variant. An oracle export can be added by a later ADR if a milestone needs one.

## Changes if accepted

For option 2. Every edit carries the marker (ADR-0077).

- **Design sections:**
    - **Section 18, crate table, row `conformance/`, "Depends on":** "tetraloma-refcell (path), ndelement (git), blas-src, lapack-src, openblas-src" becomes "tetraloma-dims and tetraloma-refcell (path), ndelement (git), rlst (ndelement's array type, for the exporter's tabulations), serde_json, blas-src, lapack-src, openblas-src (ADR-0077)".
    - **Section 23, row 2** (`tetraloma-refcell/tests/fixtures/`, the ndelement pin): "What must not change" becomes "the reference-cell conventions exported from ndelement, and the Isaac GLL points written from Basix (`gll_isaac.json`, A.10; ADR-0077)"; "Check" gains "`scripts/fixtures/element/generate.py --check` (with the pinned Python environment; ADR-0077)".
    - **Section 23, row `tetraloma-element/tests/fixtures/`:** "Check" gains "the exporter's and the generator's `--check` (A.10; ADR-0077)".
    - **Section 26, Listing E1** (comments, derives and trait impls only; no signature changes):
        - `pub struct Element { /* private */ }` gains the comment "implements Debug (ADR-0077)".
        - `Element::get` gains "/// Errors are cached per key, as elements are (ADR-0077)."
        - `entity_dofs` gains "/// Interior DOFs of sub-entity (d, e); over all (d, e) they partition 0..dim. Panics if (d, e) is out of range (ADR-0077)."; `entity_closure_dofs` gains "/// DOFs on the closure of (d, e), by ascending sub-entity dimension, then index. Panics as entity_dofs (ADR-0077)."
        - The comment of `Element::tabulate` gains: "nderivs is checked first; a point array whose length is not a multiple of tdim gives ElementError::RefCell(PointsShape), and no points an empty table (ADR-0077)".
        - The comment of `base_transformation` becomes "/// The factor of M_c (DOF transformations) for `generator` on sub-entities of type `entity`; None if the cell has no lower-dimensional sub-entities of that type, the element has no interior DOFs on them, or `generator` does not apply to `entity` (ADR-0077)."
        - `ElementTabulation` gains `#[derive(Clone, Debug, PartialEq)]` (ADR-0077).
        - `BaseTransformation`: the comment becomes "/// Acts on the interior DOFs of one sub-entity, from canonical to cell-local order: y = M x (ADR-0077)."; it gains `#[derive(Clone, Debug, PartialEq)]`; `SignedPermutation` gains "/// y[i] = −x[perm[i]] if negate[i], else x[perm[i]] (ADR-0077)"; the comment of `Dense` becomes "// row-major n × n: y[i] = Σ_j matrix[i·n + j] x[j] (ADR-0077)".
        - `Factor1d` gains "/// A factor (kind, d) spans P_d on [0, 1], with d + 1 functions (Tensor factors; ADR-0077)."
        - `TensorFactors` gains `#[derive(Clone, Debug, PartialEq, Eq)]` (ADR-0077).
        - `ElementError` gains the comment "implements Display, std::error::Error and From<RefCellError> (ADR-0077)".
        - `DenseLu` gains `#[derive(Debug)]`, and `DenseLu::solve` "/// b is row-major n × nrhs (b[i·nrhs + j]); n = 0 or nrhs = 0 with an empty b is a no-op (ADR-0077)."
    - **Section 26, "DOF transformations", the bullet "Composition"** gains, after its second sentence:

      > On the interior DOFs of edge e, M_c applies `Reversal` of the interval once if bit e of the word is set; on those of face f, reflection once if the face's reflection bit is set, then rotation as many times as its rotation count: u_local = M_R^rot M_S^refl u_global, each factor a base transformation (ADR-0077).

    - **Section 26, "DOF transformations", the bullet "Application"** becomes:

      > **Application** (ADR-0077). Let T_c map the reference basis to cell c's basis (φ_c = T_c φ_ref). Its transpose M_c = T_cᵀ maps canonical coefficients to cell-local ones (u_local = M_c u_global), and `base_transformation` returns the factors of M_c (Composition). Evaluation and element vectors apply T_c, the map of global coefficients into a cell applies M_c, and interpolation applies T_c^{−ᵀ} = M_c^{−1}. A signed permutation is orthogonal, so T_c^{−ᵀ} = T_c and M_c^{−1} = M_cᵀ: no solve is needed, and one signed index map serves all three, applied as a gather for M_c and as a scatter for T_c. T_c and M_c are different permutations unless T_c is an involution, which rotations of order 3 and 4 are not.

    - **Section 26, "Tensor factors":** after "There are three kinds of factor", the sentence "A factor is a pair (kind, d); it spans P_d, the polynomials of degree at most d on [0, 1], with d + 1 functions (ADR-0077)." The three bullets become:

      > - `NodalH1`: Lagrange at a point family; (`NodalH1`, p) is the factor of nodal Lagrange and DG of degree p; at degree 0 it is the one function 1 (ADR-0072).
      > - `MomentH1`: endpoint evaluations plus interior Legendre moments up to degree d − 2; (`MomentH1`, p) is the factor of N1curl and RT of degree p in their continuous directions (ADR-0077).
      > - `LegendreL2`: integral moments against an orthonormal Legendre basis of P_d; (`LegendreL2`, p − 1) is the factor of N1curl and RT of degree p in the remaining directions, and (`LegendreL2`, p) that of Legendre DG of degree p (ADR-0077).

    - **A.10, "Conformance setup", the bullet "Fixtures"** becomes:

      > **Fixtures:** the conformance crate exports JSON fixtures to `tetraloma-refcell/tests/fixtures/` and `tetraloma-element/tests/fixtures/`, together with the pinned commits of ndelement, Basix, DOLFINx and Symfem. `scripts/fixtures/element/generate.py` writes the Basix fixtures (`gll_isaac.json` in tetraloma-refcell, `lagrange_basix.json` in tetraloma-element) from the pinned Basix and refuses any other version. Both have a `--check` mode: hosted CI's conformance job runs the exporter's; the generator's needs the pinned Python environment (ADR-0077). The fixtures are committed, so ordinary `cargo test --workspace` needs neither BLAS nor a git dependency.

    - **A.10, "Oracles for elements and grid functions":** the bullet **ndelement** becomes "JSON fixtures for Lagrange tabulations and base transformations on the cells it offers, and for N1curl and RT spans and DOF counts (ADR-0077)"; the bullet **Basix** becomes "Isaac GLL points (`gll_isaac`), and the Lagrange tabulations of the cells and families ndelement lacks; its triangle and tetrahedron numbering is converted with A.11 (ADR-0077)". After the list:

      > DOF layouts are checked against tetraloma's own tables (A.4–A.6). Base transformations on prisms and pyramids and the `Legendre` variant have no oracle; they rest on the continuity, permutation, interpolation, orthonormality and tensor-factor tests of M11 (ADR-0077).

    - **A.10, Listing A.1:** the tree gains `tetraloma-element/tests/fixtures/*.json` ("exported by tetraloma-conformance and by scripts/fixtures/element/generate.py, committed") and `scripts/fixtures/element/generate.py` ("the Basix fixtures"); the comment of `export_fixtures.rs` becomes "writes the ndelement fixtures of tetraloma-refcell and tetraloma-element"; the comment of `tetraloma-refcell/tests/fixtures/*.json` becomes "exported by tetraloma-conformance (gll_isaac.json by generate.py), committed"; the manifest gains `tetraloma-dims = { path = "../tetraloma-dims" }` and `rlst = "0.6"` with the marker.
- **Interfaces and tests:** none change. The rustdoc of `Factor1d::MomentH1` says "up to degree `p − 2`"; M12, which first constructs it, rewords it to d − 2.
- **Roadmap:** none.
- **Plans and tasks affected:** none open. `docs/plans/M11.md`, Decision 17, keeps its wording as history; this ADR states its rule in the design. M12's plan builds on the tensor-factor degrees and the contracts of Listing E1; M14's on "Application" and "Composition".
- **New tasks:** none.

Option 3 would add to this: the two A.10 oracle bullets keep "DOF layouts" and "Legendre variants and base transformations", with "(exported in M12; ADR-0077)"; M12's roadmap entry gains a criterion for each of the three exports; the plan of M12 gets one task for them.

## Decision

Pending.

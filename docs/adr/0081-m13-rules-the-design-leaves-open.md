# ADR-0081: M13, rules the design leaves open: symmetric table source, error types, Listing Q1 contracts and parallel M12/M13 A7

Status: Proposed
Date: 2026-11-28
Decision issue: #144
Raised by: the plan of M13 (`docs/plans/M13.md`)

## Context

Planning M13 (`docs/roadmap.md`, "M13: Quadrature rules and point sets") found four places where sections 18 and 27 of `docs/design.md` and the M13 roadmap entry do not match the existing code, the verified quadrature tables or parallel planning with M12.

### 1. Parallel M12/M13 scope of acceptance criterion A7 (`docs/roadmap.md`, M13)

- `docs/roadmap.md` gives M13 `Depends on: M11` (not M12), and M12 is being planned in parallel in a separate session.
- Today `Element::get` returns `ElementError::Unsupported` for `Family::N1curl` and `Family::RaviartThomas`.
- Yet M13's 7th acceptance bullet states: "`PointSet::tabulate_element` equals the host tabulation of every element of M11 and M12 with p ≤ 6 to 10^−12 in f64 and 10^−5 in f32."
- `PointSet::tabulate_element` delegates to `Element::tabulate` and handles any `reference_value_size`, so its implementation is already generic over scalar and vector-valued elements. Only M13's test cannot construct M12 elements if M13 finishes before M12 merges.

### 2. Source of `Symmetric` quadrature tables on simplices (section 27)

- Section 27's rule table lists `Symmetric | fully symmetric tables (Xiao–Gimbutas), degree 1–30` for triangles and tetrahedra, and "Table provenance" requires MIT/Apache-2.0-compatible licences recorded in `tetraloma-refcell/tables/sources.json`.
- M13 acceptance criterion A3 requires: "Every `Symmetric` rule on triangles and tetrahedra maps onto itself under every vertex permutation of the cell, with points and weights equal to 10^−15."
- Direct verification in the pinned Python environment (`fenics-basix==0.11.0`, MIT License):
  - `basix.make_quadrature(triangle, d, xiao_gimbutas)` covers degrees 1–30, is strictly interior with positive weights, has fewer points than `Gauss` for degrees 4–30, and is fully symmetric under all 6 triangle permutations (maximum point error 1.63·10^−16, weight error 0).
  - `basix.make_quadrature(tetrahedron, d, xiao_gimbutas)` covers only degrees 1–15 (`d = 16` fails with "Xiao-Gimbutas not implemented for this order"), **and its rules are asymmetric**: only degrees 1 and 5 are invariant under vertex permutations (for example, degree 2 has four distinct weights `[0.016935, 0.046463, 0.050087, 0.053182]`). Thus Xiao–Gimbutas tetrahedral rules cannot satisfy A3 even up to degree 15.
- `inducer/modepy`'s `jaskowiec_sukumar_tet_data.py` (Jaśkowiec & Sukumar 2021, tetrahedra degrees 1–20) carries a Creative Commons Attribution 4.0 (`CC-BY-4.0`) licence header, not an MIT/Apache-2.0-compatible software licence.
- `OptimalDesignLab/SummationByParts.jl` (Worku, Hicken & Zingg; commit `9d2a6e7aa8a34d18e568a60052870307ac9d6321`, pure MIT "Expat" License in `LICENSE.md`) provides positive-interior, fully symmetric rules in compact symmetry-orbit form (`pi_quadrature_data/tri/compact/` for degrees 1–84 and `pi_quadrature_data/tet/compact/` for degrees 1–40):
  - Every rule is stored as symmetry orbits (`S1`, `S21`, `S111` on triangles; `S1`, `S31`, `S22`, `S211`, `S1111` on tetrahedra), so expanding the orbits produces exact permutation symmetry under all 6 triangle and 24 tetrahedron vertex permutations.
  - Every degree 4–30 has strictly fewer points than `Gauss` on both triangles (e.g. degree 4: 6 vs. 9; degree 30: 169 vs. 256) and tetrahedra (e.g. degree 4: 14 vs. 27; degree 15: 216 vs. 512; degree 30: 1,564 vs. 4,096).

### 3. `RefCellError` and `GridError` variants for Listing Q1 (section 27)

- Section 27 states: "Any other request returns `RefCellError::UnsupportedDegree`, or `UnsupportedNodeFamily` for a type on a cell it does not cover."
- In `tetraloma-refcell/src/error.rs`, `RefCellError::UnsupportedNodeFamily { cell: CellType, family: NodeFamily }` carries `NodeFamily`, so it cannot represent an unsupported `QuadratureType`.
- `RefCellError` is currently exhaustive (no `#[non_exhaustive]`; no crate outside `tetraloma-refcell` matches it exhaustively) and lacks variants for the error conditions of the other three new functions in Listing Q1:
  - `sub_entity_points_to_cell`: out-of-range sub-entity dimension `d` or index `e`;
  - `rule_permutation`: `entity` not a sub-entity cell type (`Point`, `Interval`, `Triangle`, `Quadrilateral`), `rule.cell != entity`, invalid A.6 `bits`, or a rule that is not invariant under the reorientation;
  - `permute_vertices`: `perm` not a valid vertex symmetry of `cell`.
- In `tetraloma-topology/src/error.rs`, `GridError` (already `#[non_exhaustive]`) has no variant for an unsupported `QuadratureType` (from `QuadRule::new` / `FacetQuadRule::new`), an unsupported derivative order (from `PointSet::tabulate_element`), or a cell mismatch between a `PointSet` and an `Element` (`e.key().cell != self.cell.into()`).

### 4. Edge-case contracts of Listing Q1 (sections 18 and 27)

- **Degree 0 and `CellType::Point` in `quadrature_rule`:** Section 27's rule table lists `Default` on triangles and tetrahedra as "`Symmetric` for degree 1–30; `Gauss` for 31–60" without stating degree 0, omits `CellType::Point` (needed by `FacetQuadRule::new` on intervals, whose facets are points), and gives `GaussLobatto` as `n = 2–41` with `n = ⌈(degree + 3) / 2⌉` (covering `degree = 0..=79`, exact to `2n − 3`).
- **`FacetQuadRule::new`:** Section 27 notes that `FacetQuadRule::new` with `Default` supports symmetric rules for degree 1–30 on triangular facets, without stating how each facet calls `quadrature_rule` across `Default`, `Gauss`, `Symmetric` and `GaussLobatto` (or at degrees 0 and 31–60 under `Default`).
- **Direction of `rule_permutation` and `permute_vertices`:** Section 27 does not state whether `rule_permutation` and `permute_vertices` use the gather or scatter direction, nor whether `permute_vertices` on non-simplices (`Quadrilateral`, `Hexahedron`, `Prism`, `Pyramid`) restricts `perm` to cell symmetries (automorphisms of the vertex-edge structure) so that the induced reference map is a well-defined affine bijection of the reference cell onto itself (avoiding degenerate bowties and avoiding the `1/(1 − z)` singularity at the pyramid apex).
- **`CellPoints` accessors:** Listing Q1 lists `new`, `block` and `len`. Workspace clippy requires `is_empty` alongside `len`, and `tetraloma-function` (M15, a separate crate) needs access to `cells`, `points` and `client` on `CellPoints`.
- **Facade and prelude re-exports:** Section 18 lists `QuadratureType` in `tetraloma-refcell` and `CellPoints` in `tetraloma-geometry`, without stating their re-exports through `tetraloma-geometry`, `tetraloma` and `tetraloma::prelude` (where `NodeFamily`, `PointSet`, `QuadRule` and `FacetQuadRule` already live).

### The maintainer's answers so far

Said to the agent in a session on 2026-11-28, on the planning questions Q1–Q5:
- Q1: "M12 is currently planning in a separate session. Can the parallel planning proceed?" (confirmed feasible with M12/M13 split for A7).
- Q2: "Q2 as recommended" (after verification showed Basix's tetrahedral `xiao_gimbutas` rules are asymmetric and fail A3; recommended: `OptimalDesignLab/SummationByParts.jl` MIT tables for degrees 1–30 on tetrahedra, preserving §27 and A1/A4's 1–30 range, and compact-orbit MIT tables for triangles 1–30).
- Q3: "as recommended"
- Q4: "as recommended"
- Q5: "as recommended"

## Options

1. **Keep the design as written.** Cannot be built: `RefCellError::UnsupportedNodeFamily` cannot carry a `QuadratureType`, Xiao–Gimbutas tetrahedral rules fail A3 (and stop at degree 15), and M13's A7 cannot construct M12 elements while M12 is planned and implemented in parallel.
2. **State the four rules in sections 18 and 27 and clarify M13's A7 in the roadmap.**
   - Symmetric tables (degrees 1–30 on both triangles and tetrahedra) come from MIT-licensed `OptimalDesignLab/SummationByParts.jl` (Worku, Hicken & Zingg) in compact symmetry-orbit form, keeping the full 1–30 degree range of §27 and roadmap A1/A4 unchanged.
   - `RefCellError` becomes `#[non_exhaustive]` and gains the six variants for Listing Q1; `GridError` gains the three matching variants.
   - Section 27 states the exact degree-0/`Point` rules, `FacetQuadRule::new` delegation, gather conventions for `rule_permutation` and `permute_vertices` (restricted to cell symmetries), `CellPoints` accessors (`is_empty`, `cells`, `points`, `client`), and section 18 states the re-exports of `QuadratureType` and `CellPoints`.
   - `docs/roadmap.md` clarifies M13's 7th bullet so M13 tests all M11 elements (and M12's if already merged), and M12 covers N1curl and RT if M13 merges first.
3. **As option 2, capping tetrahedral `Symmetric` rules at degree 15** (using `SummationByParts.jl` degrees 1–15 on tetrahedra and falling back to `Gauss` for 16–60). This additionally amends §27's table and M13's roadmap bullets A1 and A4, with little benefit since `SummationByParts.jl` already supplies compact-orbit MIT tables for degrees 16–30 (~15 small text files) with a 2.6× point reduction at degree 30.

## Recommendation

Option 2.

- Using `OptimalDesignLab/SummationByParts.jl` (MIT) compact-orbit tables for both triangles (1–30) and tetrahedra (1–30) gives one clean, uniform MIT-licensed source, guarantees exact permutation symmetry (A3) by construction, satisfies A1, A2 and A4 over the full 1–30 range without narrowing the roadmap, and keeps `tetraloma-refcell/tables/` compact.
- Dedicated error variants in `RefCellError` and `GridError` fix the type-level impossibility in §27 and give every Listing Q1 function explicit, testable error paths.
- Fixing the gather conventions, cell-symmetry restriction, `Point`/degree-0 behaviour, `CellPoints` accessors and re-exports in §18 and §27 keeps `docs/design.md` the single source of truth for M13–M17 and BEM callers.
- Clarifying A7 lets M12 and M13 proceed in parallel without either milestone blocking the other.

## Changes if accepted

For option 2. Each edit carries the marker `(ADR-0081)`.

- **Design sections:**
  - **Section 18, crate table, row `tetraloma-geometry`, "Purpose":** add re-export of `QuadratureType` (`QuadratureType` re-exported from `tetraloma-refcell`, ADR-0081).
  - **Section 18, crate table, row `tetraloma`, "Purpose":** add re-exports of `QuadratureType` and `CellPoints` at the root and in `prelude` (ADR-0081).
  - **Section 27, Listing Q1 (`tetraloma-geometry`):** add `is_empty`, `cells`, `points` and `client` to `CellPoints` (`ADR-0081`):
    ```rust
    impl<F: Real, TD: TopoDim> CellPoints<F, TD> {
        pub fn new(client: &Client, block: usize, cells: DeviceBuffer<u32>, points: DeviceBuffer<F>) -> Result<Self>;
        pub fn block(&self) -> usize;
        pub fn len(&self) -> usize;
        pub fn is_empty(&self) -> bool;
        pub fn cells(&self) -> &DeviceBuffer<u32>;
        pub fn points(&self) -> &DeviceBuffer<F>;
        pub fn client(&self) -> &Client;
    }
    ```
  - **Section 27, "Behaviour and invariants", Rule types table:**
    - `Default` on triangles and tetrahedra: "`Symmetric` for degree 1–30; `Gauss` for 0 and 31–60 (ADR-0081)".
    - `Symmetric` on triangles and tetrahedra: "fully symmetric compact-orbit positive-interior tables (Worku–Hicken–Zingg, `SummationByParts.jl`, MIT), degree 1–30 (ADR-0081)".
    - `GaussLobatto` on intervals, quadrilaterals and hexahedra: "`degree` 0–79 (`n = ⌈(degree + 3) / 2⌉` ∈ 2–41 points per direction, exact to degree 2n − 3) (ADR-0081)".
    - Add sentence for `CellType::Point`: "`Point` accepts `Default` and `Gauss` for degrees 0–60 (one point of weight 1, empty coordinates) and rejects `Symmetric` and `GaussLobatto` (ADR-0081)."
  - **Section 27, "Behaviour and invariants", error paragraph:** replace "`RefCellError::UnsupportedDegree`, or `UnsupportedNodeFamily` for a type on a cell it does not cover" with:
    - `RefCellError` is `#[non_exhaustive]` and adds `UnsupportedQuadratureType { cell: CellType, kind: QuadratureType }`, `InvalidSubEntity { cell: CellType, dim: usize, index: usize }`, `InvalidOrientationBits { entity: CellType, bits: u32 }`, `UnsymmetricRule { entity: CellType }`, `InvalidVertexPermutation { cell: CellType }`, and `CellMismatch { expected: CellType, found: CellType }` (ADR-0081).
    - `GridError` adds `UnsupportedQuadratureType { cell: &'static str, kind: &'static str }`, `UnsupportedDerivativeOrder(u32)`, and `CellMismatch { expected: &'static str, found: &'static str }` (ADR-0081).
    - `PointSet::len()` returns the number of points (`points.len() / TD::N`), even when `weights` is empty; passing a non-empty weightless `PointSet` to `measure_block` returns `GridError::LengthMismatch` (ADR-0081).
  - **Section 27, "Interior facets":**
    - State that `FacetQuadRule::new` delegates each facet to `quadrature_rule(facet_type, degree, kind)` (so `Default` covers 0–60 on every cell and produces symmetric rules on triangular facets for 1–30) (ADR-0081).
    - State `rule_permutation(entity, rule, bits)` contract: `entity` is `Point`, `Interval`, `Triangle` or `Quadrilateral`; `rule.cell` must equal `entity`; `bits` is the A.6 bit field (`0` on Point; `0..=1` on Interval; `0..=2 | 4..=6` on Triangle; `0..=7` on Quadrilateral); returns the gather permutation `perm` where `perm[q]` is the cell-local rule index matching canonical point `q` under `Φ_bits`, or `UnsymmetricRule` if the rule is not invariant (ADR-0081).
  - **Section 27, "Point sets of the caller's choice":**
    - State `permute_vertices(cell, perm, points)` contract: `perm` must be a combinatorial symmetry (vertex-edge graph automorphism) of `cell` (1/2/6/8/24/48/12/8 symmetries for Point/Interval/Triangle/Quadrilateral/Tetrahedron/Hexahedron/Prism/Pyramid; non-symmetries return `InvalidVertexPermutation`); gather convention `Φ(v̂_k) = v̂_{perm[k]}`, so a cell with vertices `[V_{perm[0]}, …]` satisfies `X_permuted(ξ) = X_original(permute_vertices(cell, perm, ξ))` (ADR-0081).
- **Roadmap (`docs/roadmap.md`, M13, bullet 7):**
  - Clarify: "`PointSet::tabulate_element` equals the host tabulation of every element of M11 (and M12 once M12 is merged; if M13 merges first, M12 extends the check to N1curl and RT) with p ≤ 6 to 10^−12 in f64 and 10^−5 in f32 (ADR-0081)."
- **Process documents:** `docs/adr/README.md`, add the row for ADR-0081.
- **Interfaces and tests:**
  - `RefCellError` becomes `#[non_exhaustive]` with six new variants; `GridError` gains three new variants.
  - `CellPoints` gains `is_empty`, `cells`, `points`, `client`.
  - `tetraloma-geometry` re-exports `QuadratureType`; `tetraloma` and `tetraloma::prelude` re-export `QuadratureType` and `CellPoints`.
- **Plans and tasks affected:** `docs/plans/M13.md` (T0–T5); M12's plan adds the N1curl/RT extension of A7 if M13 merges before M12.
- **New tasks:** none.

## Decision

> Pending maintainer acceptance of ADR-0081.

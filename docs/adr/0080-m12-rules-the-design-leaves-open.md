# ADR-0080: M12, rules the design leaves open: `TensorFactors` signs, vector layouts and the A2 span oracle

Status: Proposed
Date: 2026-11-28
Decision issue: #143
Raised by: the plan of M12 (`docs/plans/M12.md`), its open questions 1 to 3

## Context

Planning M12 (`N1curl and RT elements`) found three places where `docs/design.md` (sections 18, 26 and A.10) and `docs/roadmap.md` (M12 acceptance bullet 2) do not fit or leave open what M12's tasks need.

### 1. Per-component sign on `TensorFactors` for `RTCF` and `NCF` (section 26, Listing E1)

- On quadrilaterals and hexahedra, `RaviartThomas` is the tensor-product space `RTCF_p` (`Q_{p,p−1} × Q_{p−1,p}`) or `NCF_p` (`Q_{p,p−1,p−1} × Q_{p−1,p,p−1} × Q_{p−1,p−1,p}`), whose facets carry normal moment functionals and whose push-forward is the contravariant Piola map `u = J û / det J`.
- For two neighbouring cells sharing a facet to have continuous normal components across every relative orientation (M12 A7; M14–M17) with a single `base_transformation` per `(entity, generator)` pair independent of which facet of the cell is shared, the oriented reference facet normals must share consistent handedness with the facet's own vertex list:
  - **Quadrilateral** (`(-t_y, t_x)` for edge tangent `t = v_1 − v_0`): `n = (0, +1) = +e_y` on edges `0` and `3` (`y = 0, 1`), and **`n = (−1, 0) = −e_x`** on edges `1` and `2` (`x = 0, 1`). Using `+e_x` and `+e_y` instead produces a contradiction on `Generator::Reversal`: a 90°-rotated neighbour (`det J = +1`) sharing edge `x = 1` with edge `y = 0` requires `M_rev = [+1]`, whereas a 180°-rotated neighbour sharing edge `y = 1` with edge `y = 1` requires `M_rev = [−1]`.
  - **Hexahedron** (`(v_1 − v_0) × (v_2 − v_0)`): `n = +e_x` on `x = 0, 1` faces, **`n = −e_y`** on `y = 0, 1` faces, and `n = +e_z` on `z = 0, 1` faces.
- These are the exact facet normals used by `ndelement::reference_cell::facet_normals` (`f8cbffe`) and Basix (`0.11.0`).
- Applying the same per-component sign to the interior moments of that component makes every basis function of component `c` equal `(±1) · Π_d f_{c,d}(ξ_d) e_c`, with `−1` on component `0` of `RTCF_p` and component `1` of `NCF_p`, and `+1` on every component of `Lagrange`, `RTCE_p` and `NCE_p`.
- Listing E1's `TensorFactors` currently has only `pub factors: Vec<Vec<(Factor1d, u32)>>` and `pub lexicographic: Vec<Vec<usize>>`, with no field to carry the per-component sign.

### 2. Inner column layouts of `Element::coefficients()` and `Element::interpolation_matrix()` (section 26, Listing E1)

- Listing E1 gives `coefficients()` the shape `[dim][polyset dim · reference value size]` and `interpolation_matrix()` the shape `[dim][m · reference value size]`.
- In M11 every element was scalar (`reference_value_size() == 1`), so the inner product order was invisible. M12 introduces vector elements (`reference_value_size() == tdim`), and M15 (`evaluate_at`) and M16 (`interpolate`) read both tables directly.
- Without fixing both inner orderings in Listing E1, downstream milestones must guess between `[component][polyset]` vs `[polyset][component]` for `coefficients()`, and between `[point][component]` vs `[component][point]` for `interpolation_matrix()`.

### 3. M12 acceptance criterion A2: cell-closure point count and `conformance/` execution (sections 18, A.10; `docs/roadmap.md`)

- M12 acceptance bullet 2 states: *"For the same elements and degrees [N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra, p = 1–8], the span of the basis functions associated with each sub-entity closure equals ndelement's (Basix's where ndelement lacks the element), by a rank test at 100 random points with tolerance 10^−10."*
- Inspection of `ndelement` at `f8cbffe` (`ndelement/src/ciarlet/{nedelec,raviart_thomas}.rs`) shows that `ndelement` implements both `nedelec` and `raviart_thomas` on all four cells (`Triangle`, `Quadrilateral`, `Tetrahedron`, `Hexahedron`), so Basix is not needed as a fallback for A2.
- **Vacuous 100-point rank test on `d = tdim`:** at 100 points in 3D, a vector tabulation has `100 × 3 = 300` scalar rows. For every proper sub-entity (`d < tdim`), the closure DOF count at `p ≤ 8` is at most 144 (hexahedron `NCE_8` face closure), so 100 points is strictly overdetermined. For the cell closure (`d = tdim`), however, `dim ≥ 300` on tetrahedra (`N1curl` `p = 7, 8`: 315, 440; `RT` `p = 8`: 396) and hexahedra (`NCE` `p = 4..=8`: 300, 540, 882, 1344, 1944; `NCF` `p = 5..=8`: 450, 756, 1176, 1728). A `300 × dim` matrix has rank at most 300, making a 100-point rank test on `d = tdim` mathematically vacuous whenever `dim ≥ 300`.
- **Committed fixture size:** storing `ndelement`'s 100+ point tabulations across all four cells and both families for `p = 1..=8` in committed JSON files under `tetraloma-element/tests/fixtures/` requires ~3.9 million floats at 100 points (~75–85 MB in JSON) and ~7+ million floats (~150 MB) once `d = tdim` uses enough points to be non-vacuous. All existing committed fixtures across the repository total 4.4 MB.
- `conformance/` (`tetraloma-conformance`, excluded from the main workspace) already links `ndelement` and BLAS/LAPACK, runs directly against `tetraloma-refcell` for M1b's tables, orientation and Lagrange tests, and is triggered by `.github/workflows/conformance.yml` on every change to `tetraloma-element/**` as well as nightly and at every milestone gate.

## Options

1. **Keep the design as written.**
   - `TensorFactors` cannot represent the sign of `RTCF` component 0 and `NCF` component 1 without either breaking `ContravariantPiola` continuity across rotated neighbours (A7) or secretly negating `Factor1d::MomentH1` inside `tabulate_1d` (still requiring a private field on `TensorFactors`, breaking struct literals and contradicting `MomentH1`'s definition).
   - The vector inner layouts of `coefficients()` and `interpolation_matrix()` remain unspecified in Listing E1.
   - A2 either commits ~80–150 MB of JSON fixtures to git and runs a vacuous 100-point rank test on `d = tdim` for `dim ≥ 300`, or deviates from A.10 without an ADR.
2. **Adopt the three clarifications (recommended).**
   - Add `pub negate: Vec<bool>` (one per reference component, matching `BaseTransformation::SignedPermutation::negate` and preserving `#[derive(Clone, Debug, PartialEq, Eq)]`) to `TensorFactors` in Listing E1 and document the `RTCF`/`NCF` component signs in section 26 ("Tensor factors").
   - Fix `Element::coefficients()` to row-major `[dim][component][polyset]` (matching `ndelement` and contiguous per-component dot products against `polyset`) and `Element::interpolation_matrix()` to row-major `[dim][point][component]` (matching `ElementTabulation` and Listing F3 `interpolate_values` so `dofs = M û(points)` is a direct matrix-vector product).
   - Run M12 A2 directly in `conformance/` (`conformance/tests/hcurl_hdiv.rs`), adding `tetraloma-element = { path = "../tetraloma-element" }` (with `[profile.dev.package.*] opt-level = 3`) to `conformance/Cargo.toml`, using `100` random points for proper sub-entity closures (`d < tdim`) and `max(100, ⌈dim / tdim⌉ + 20)` random points for the cell closure (`d = tdim`) at tolerance `10^−10`. Update section 18 (`conformance/` row), A.10 (Fixtures, Oracles, Listing A.1, also recording Symfem's `generate.py` fixture for A1) and `docs/roadmap.md` (M12 acceptance bullet 2).
3. **As Option 2 for items 1 and 2, plus a small committed span fixture in `tetraloma-element/tests/fixtures/` for low degrees (`p ≤ 3` or `4`, ~2–3 MB) alongside the full `p = 1..=8` test in `conformance/`.**
   - Duplicates the span rank test and adds ~2–3 MB of JSON fixtures while `conformance/` already tests the full range `p = 1..=8` on every `tetraloma-element/` push and at every gate.

## Recommendation

Option 2.

- `TensorFactors::negate: Vec<bool>` is the minimal, explicit representation of the mathematically forced `RTCF`/`NCF` component signs; it keeps `Factor1d::MomentH1` pure, preserves `Eq` on `TensorFactors`, and costs zero extra operations in M17's sum-factorised evaluation.
- Fixing `[dim][component][polyset]` and `[dim][point][component]` in Listing E1 gives M12, M15 and M16 one unambiguous contract.
- Running A2 directly in `conformance/` with `max(100, ⌈dim / tdim⌉ + 20)` points on `d = tdim` makes the rank test non-vacuous across all `p = 1..=8` with zero git bloat and follows M1b's established `conformance/` pattern.

## Changes if accepted

For Option 2. Every edit carries the marker `(ADR-0080)`.

- **Design sections (`docs/design.md`):**
  - **Section 18, crate table, row `conformance/`:** update "Purpose" to state that `tetraloma-conformance` compares `tetraloma-refcell` and `tetraloma-element` with `ndelement` at the pinned commit, and add `tetraloma-element (path; ADR-0080)` to "Depends on".
  - **Section 20, A.10 ("Fixtures"):** note that `scripts/fixtures/element/generate.py` also writes the Symfem DOF-count fixture (`hcurl_hdiv_dofs_symfem.json` in `tetraloma-element`) from the pinned Symfem and refuses any other version (ADR-0080).
  - **Section 20, A.10 ("Oracles for elements and grid functions"):**
    - **ndelement:** update to "JSON fixtures for Lagrange tabulations and base transformations on the cells it offers, and direct in-process `conformance/` tests for N1curl and RT sub-entity closure spans on all four cells up to p = 8 (ADR-0077, ADR-0080)."
    - **Symfem:** update to "JSON fixture of sub-entity DOF counts for N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra, p = 1–8, in DefElement numbering (`generate.py`; ADR-0080)."
  - **Section 20, A.10 (Listing A.1):** update `generate.py` comment to include Symfem; add `tests/hcurl_hdiv.rs` under `conformance/`; add `tetraloma-element = { path = "../tetraloma-element" }` to `conformance/Cargo.toml`.
  - **Section 26, Listing E1:**
    - `Element::coefficients`: specify `[dim][component][polyset]` (row-major `[dim][reference value size · polyset dim]; ADR-0080`).
    - `Element::interpolation_matrix`: specify `[dim][point][component]` (row-major `[dim][m · reference value size]; ADR-0080`) with `û(points)` in `[point][component]` order.
    - `TensorFactors`: add `pub negate: Vec<bool>` with comment `/// Per-component sign flip: if true, negate the tensor product for that component (ADR-0080).`
  - **Section 26, "Tensor factors":** add a paragraph explaining the consistent facet-normal handedness on `RTCF` and `NCF`, the matching sign on interior moments of that component, and `TensorFactors::negate[c]` (`true` for `RTCF` component 0 and `NCF` component 1; `false` elsewhere), and update the closing sentence to include negation where `negate[c]` is true.
- **Roadmap (`docs/roadmap.md`):**
  - **M12, acceptance bullet 2:** clarify `(max(100, ⌈dim / tdim⌉ + 20) for the cell closure when dim ≥ 100 · tdim)` and `, run in conformance/ (ADR-0080)`.
- **Interfaces and tests** (in M12's task pull requests, not in this ADR):
  - M12.T0 adds `pub negate: Vec<bool>` to `TensorFactors` (setting `vec![false]` on Lagrange elements), adds `tetraloma-element` (path) and `[profile.dev.package.*] opt-level = 3` to `conformance/Cargo.toml`, and adds the ignored A2 test in `conformance/tests/hcurl_hdiv.rs`.
  - M12.T2–T6 implement the vector layouts, `RTCF`/`NCF` signs, and A1–A11.
- **Process documents:** `docs/adr/README.md` (add the ADR-0080 row in the second commit of this pull request).
- **Plans and tasks affected:** `docs/plans/M12.md` (not yet merged; written against Option 2 in `Plan M12`).
- **New tasks:** none.

## Decision

*Pending maintainer answer.*

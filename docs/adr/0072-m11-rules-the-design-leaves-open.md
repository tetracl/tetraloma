# ADR-0072: M11, rules the design leaves open: degree 0, derivative orders and the book's examples

Status: Proposed
Date: 2026-11-22
Decision issue: #65
Raised by: the plan of M11 (pull request #64, `docs/plans/M11.md`), its open questions 1 to 3, and finding G10 of its review

## Context

Planning M11 found three places where sections 18 and 26 of `docs/design.md` do not say what the tasks need. The plan's review (finding G10) suggested stating them in the design, so that section 26 stays the place where later milestones look them up.

### 1. The book's examples of finite elements (section 18)

- Section 14 adds a part "finite elements and grid functions" to the book, and requires every `rust` block of the book to run as a doctest of tetraloma-examples.
- Section 18 gives tetraloma-examples the dependencies `tetraloma, tetraloma-refcell, tetraloma-testing; cubecl with runtime features; mpi (optional)`. `tetraloma-examples/Cargo.toml` has exactly these.
- The facade does not depend on tetraloma-element in M11, and no listing gives it a re-export of that crate or of tetraloma-function.

So a chapter on elements cannot carry a running example. The same gap opens for tetraloma-function when M14 to M16 write the tutorials "function spaces and interpolation" and "grid functions in VTK output".

### 2. Nodal elements of degree 0 (section 26)

- Section 26 gives DG the degrees from 0 on every cell ("degree 0 is the lowest DG"), and names the variants `Nodal(Equispaced)`, `Nodal(Gll)` and `Legendre`.
- No node family has a point at degree 0: `lagrange_points` returns `UnsupportedDegree` (`tetraloma-refcell/src/lagrange.rs`).
- Section 26 ties `discontinuous_only()` to `Legendre` only.
- `Legendre` of degree 0 has the basis function 1/√|K̂|, not 1.
- ndelement at the pin and Basix 0.11.0 both define P0 as one DOF, the value at the mean of the cell's vertices, with basis function 1, and both refuse it as a continuous element. For Basix this was checked on all seven cell types, the pyramid included.

### 3. Derivative orders (section 26)

- Listing E1 has `Element::tabulate(points, nderivs)` with "derivatives in tetraloma-refcell's order up to nderivs", and `polyset::tabulate` and `TensorFactors::tabulate_1d`, which take `nderivs` and return `Vec<f64>` without a `Result`. None states a range.
- tetraloma-refcell tabulates orders 0 and 1 and returns `UnsupportedDerivativeOrder` above. Block 0 holds the values and block 1 + d the derivative along ξ_d. No order is defined for blocks of second derivatives.
- Its orthonormal sets carry first partial derivatives written out by hand through the chain rule of the collapsed coordinates (`polynomials.rs`, `polynomials/pyramid.rs`).
- Nothing in sections 27 to 32 uses more than first derivatives: `Op` is `Value, Grad, Curl, Div` (Listing F2), and section 26 says that second derivatives of the geometry are not computed.

What second derivatives would take:

- **On the host, in tetraloma-refcell and tetraloma-element:** the orthonormal sets with second derivatives on seven cell types. ndelement's own code for this covers five cell types in about 1,080 lines (`polynomials/legendre.rs`); it has no prism or pyramid. The pyramid's rational functions have no limit of their second derivatives at the apex. Beyond that: an order for the blocks, fixtures from ndelement and Basix (both tabulate second derivatives), a tolerance, and an acceptance bullet. The plan's estimate is one more task of about 500 to 600 lines.
- **To use them on a mesh:** second derivatives of the geometry (section 10), because a physical second derivative on a curved cell needs them; new `Op` variants with their kernels in every strategy of section 29; the device basis evaluator of `evaluate_at`; and their entries in the degree policy. That is a milestone of its own, whenever it is done.
- **Compatibility of a later extension:** every signature already takes `nderivs`, and the layouts are `[derivative]…`, so more blocks can follow the existing ones. The order of ndelement and Basix (by total order; within one order by decreasing power of ξ_0, then of ξ_1) equals tetraloma-refcell's for orders 0 and 1. A later extension therefore changes no signature and moves no existing block; it turns an error into a result.

### The maintainer's answers so far

To the plan's questions 1 (degree 0), 3 (the book's examples) and 4, said to the agent in a session on 2026-11-22: "Q1, Q3, Q4 as recommended." The recommendations were the midpoint element for question 1 and this ADR for question 3. Question 2 (derivative orders) is open.

## Options

1. **Keep the design as written.** The two new book chapters carry no Rust blocks and link to the examples in the API documentation, which section 14 allows. Degree 0 and the range of `nderivs` are recorded only in the Decisions table of `docs/plans/M11.md`; M12 (its own elements and factors), M13 (`tabulate_element`) and M14 (`SpaceMismatch` for elements that are discontinuous only) would have to find them there.
2. **State the three rules, with first derivatives only.** tetraloma-examples may depend on tetraloma-element and tetraloma-function. Section 26 defines the nodal element of degree 0 as the midpoint element. Section 26 states that `nderivs` is 0 or 1, what happens above, and the order that blocks of higher derivatives will take when an extension adds them. No signature, test or roadmap entry changes.
3. **As option 2, with second derivatives on the host in M11.** Section 26 states `nderivs` ≤ 2 and the block order, and what the pyramid returns at its apex. The roadmap entry of M11 gains a bullet (second derivatives against ndelement and Basix, with a tolerance to be set), and the plan a ninth task. M12 to M17 do not use the result; a mesh-level operator still needs the milestone described above.

## Recommendation

Option 2.

- The dependency is the smallest change that lets the book show running examples; naming tetraloma-function now avoids a second ADR for the same row in M14.
- The midpoint element is what the maintainer chose, and what both oracles define.
- For the derivatives, deferring costs nothing later: the host work is the same then as now, nothing built in M11 to M17 has to change, and the part that makes second derivatives usable is a milestone either way. Doing the host part now would add a task to a milestone that already has eight, for a result nothing in the roadmap consumes. Fixing the block order now is free and keeps the extension additive.

## Changes if accepted

For option 2. Each edit carries the marker (ADR-0072).

- **Design sections:**
    - **Section 18, crate table, row tetraloma-examples, "Depends on":** "tetraloma, tetraloma-refcell, tetraloma-testing" becomes "tetraloma, tetraloma-refcell, tetraloma-element and tetraloma-function (for the book's examples of sections 26 to 32; ADR-0072), tetraloma-testing".
    - **Section 26, Listing E1,** the comment of `Element::tabulate`: "derivatives in tetraloma-refcell's order up to nderivs" becomes "derivatives in tetraloma-refcell's order; nderivs is 0 or 1 (Derivative orders)".
    - **Section 26, "Variants",** the bullet **Element variants**, last sentence, becomes:

      > A `Legendre` Lagrange element, and a nodal one of degree 0, has only interior DOFs, so `discontinuous_only()` is true (ADR-0072).

    - **Section 26, "Variants",** new bullet after **Element variants**:

      > **Degree 0** (ADR-0072). Every variant exists at degree 0 on the cells that offer it. A nodal element of degree 0 has one DOF, the value at the cell's midpoint (the mean of its vertices), and its basis function is 1. `Legendre` of degree 0 has the basis function 1/√|K̂|. Layouts and coordinate elements still need p ≥ 1: `lagrange_points` returns `UnsupportedDegree` for degree 0.

    - **Section 26, "Tensor factors",** the bullet `NodalH1` gains: "at degree 0 it is the one function 1 (ADR-0072)".
    - **Section 26, "Behaviour and invariants",** new paragraph after "Construction":

      > **Derivative orders** (ADR-0072). `nderivs` is 0 or 1. Block 0 of a tabulation holds the values and block 1 + d the derivative along ξ_d, as in tetraloma-refcell. Above 1, `Element::tabulate` returns `ElementError::RefCell(RefCellError::UnsupportedDerivativeOrder)`; `polyset::tabulate` and `TensorFactors::tabulate_1d`, which return no `Result`, panic, as they do for a point array whose length is not a multiple of the dimension. Higher orders are a later extension. Their blocks will follow those of the first derivatives by total order, and within one order by decreasing power of ξ_0, then of ξ_1 (the order of ndelement and Basix), so the blocks of orders 0 and 1 keep their place.

    - **Section 26, "Out of scope",** new bullet:

      > Second and higher derivatives of basis functions, and Hessians and Laplacians of grid functions, which also need second derivatives of the geometry (section 10) (ADR-0072).

- **Interfaces and tests:** no signature changes. `tetraloma-examples/Cargo.toml` gains tetraloma-element in M11 (T4) and tetraloma-function in M14. Tests: the panics and the error above order 1 (T3, T5, T7 of M11); the test of A12 does not list a nodal element of degree 0.
- **Roadmap:** none.
- **Plans and tasks affected:** `docs/plans/M11.md`: T0 (the keys the test of A12 lists), T3 (`polyset::tabulate`), T4 (the dependency and the chapters' examples), T5 (the midpoint element; the error above order 1) and T7 (the factor of degree 0; `tabulate_1d`). Its open question 2 closes, with the answer in its Decisions table. Decisions 1 and 3 are already recorded there.
- **New tasks:** none.

Option 3 would add to this: `nderivs` ≤ 2 in the paragraph "Derivative orders", with the block order in force and the pyramid's values at its apex; one bullet in the roadmap entry of M11; one task in `docs/plans/M11.md`. The out-of-scope bullet would name only the grid-function operators.

## Decision

Not decided yet.

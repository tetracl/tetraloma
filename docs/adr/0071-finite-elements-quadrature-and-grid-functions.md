# ADR-0071: Finite elements, quadrature extensions and grid functions

Status: Accepted
Date: 2026-11-21
Decision issue: #60
Raised by: the maintainer's request to plan three extensions (elements, quadrature, grid functions), after a design discussion with an agent

## Context

tetraloma is a mesh library. Section 2 requires "no assumption about the discretisation built on top (FEM, BEM, DG, FV)", and section 4 (after Listing B7) expects that "function spaces, assemblers and BEM operators are generic over `G: DeviceGrid`", but no such crate exists. The maintainer asked for three extensions inside tetraloma, on every compute device CubeCL supports:

- an element library with Lagrange elements for continuous and discontinuous Galerkin, Nédélec and Raviart–Thomas elements at arbitrary degree, on the reference cells of section 6, with push-forwards and pull-backs for D_TOP ≤ D_GEO;
- a quadrature library with high-order rules on the elements;
- a grid-function library that creates, stores and evaluates functions on grids, including by interpolation.

What exists to build on:

- tetraloma-refcell: reference cells and sub-entity tables (A.1–A.3), orthonormal sets with derivatives on every cell and a dense LU (both crate-private), Lagrange layouts and `LagrangeElement`, `gauss_rule`, facet point maps, orientation bits (A.6).
- tetraloma-geometry: `QuadRule`, `PointSet` (tabulates `CoordinateElement`s only), `FacetQuadRule`, `QuadField`, E-vectors, factor kernels, pull-back, the device basis evaluator (crate-private), and the strategies direct, loops, sum-factorised, shared and cooperative with the crate-private degree policy (section 10; ADR-0042).
- tetraloma-topology and tetraloma-parallel: entity blocks with [owned | ghost] IndexMaps, permutation info keyed on origin ids, `Scatterer` with block sizes.

What does not fit the design as written:

1. No element definitions beyond coordinate Lagrange elements: no DG variants, no H(curl) or H(div) elements, no DOF transformations, no interpolation operators.
2. No DOF numbering and no function storage.
3. Quadrature gaps:
    - collapsed rules on triangles are not symmetric, so interior-facet points cannot be matched from both sides by an index permutation;
    - there are no GLL rules;
    - rules exist on facets but not on edges of 3D cells, which integral-moment DOFs need.
4. Point sets come only from built-in rules, and only coordinate elements can be tabulated.
5. Section 6 offers GLL points on tensor cells only, so arbitrary-degree spaces on simplices have no well-conditioned points.
6. The copy rule of section 22 does not allow the uploads and exchanges that function spaces need.

### The maintainer's answers in the design discussion

The discussion went through twelve questions, then output and comparison frameworks. The answers, word for word (said to the agent in a session, November 2026):

| Question | Answer |
| --- | --- |
| 1. Scope on prisms, pyramids and hybrid meshes | "This works H(curl) and H(div) on pyramids are no priority. They are more important on 2d triangles and quads and 3d tets and hexahedra" |
| 2. Families: first kind only in v1, others on demand | "Happy with your suggestion." |
| 3. Variants: Equispaced, Gll (Isaac points on simplices), Legendre; Legendre moments for N1curl and RT | "Your suggestion works for me" |
| 4. ndelement | "[tetraloma] should be independent of ndelement. Ndelement should only be used for conformance testing comparisons." |
| 5. Quadrature: symmetric simplex rules, GLL, sub-entity rules, symmetric facet rules | "This covers my needs" |
| 6. BEM singular quadrature | "It should live in a future BEM crate" |
| 7. Explicit entity creation; spaces borrow the grid | "I agree with your suggestion" |
| 8. Complex coefficients as real pairs; interleaved components | "Yes" |
| 9. Interpolation sources, host escape hatch, writer rule | "Yes. This works." |
| 10. Evaluation on point sets and per-cell points now, point location later | "Yes" |
| 11. Geometry view now, geometry updates later | "It works for me" |
| 12. High-order kernels in the first version | "Would a different GPU parallelisation that can deal with high p be feasible in v1? High p is an important application.", then "Yes, please" |
| Output through `VtkWriter` | "Yes" |
| Prisms; comparison frameworks | "One small change. Prisms are not a priority either. Also, what is a good comparison framework? Would you suggest mfem or something else for comparison tests?", then, on Basix, Symfem, DOLFINx and DefElement for correctness and MFEM, libCEED and deal.II with the CEED bake-off problems for performance: "That works well" |

In the answer to question 4, "[tetraloma]" replaces the project's former name, which `scripts/check-name.sh` does not allow (ADR-0066).

The answer to question 1 and the later "Prisms are not a priority either" are read together: Lagrange and DG on all cells; H(curl) and H(div) on triangles, quadrilaterals, tetrahedra and hexahedra only.

## Options

1. **Keep the design as written.** tetraloma stays a mesh library; elements and functions live in downstream crates. This costs nothing here, but each downstream crate must re-derive the conventions the mesh already fixes: origin-id orientation, E-vector and tile layouts, the vector and degree policies, deterministic writes. It does not do what the maintainer asked.
2. **Depend on ndelement for element definitions.** Rejected by the maintainer's answer to question 4. It would also bring BLAS and LAPACK into every build, against protected path 4 of section 23 ("a default build free of ndelement, BLAS and MPI"). Its DefElement numbering exists only at a git commit, which crates.io cannot publish. The tables would not be usable in `comptime!`.
3. **Two new crates, tetraloma-element and tetraloma-function, plus extensions of tetraloma-refcell, tetraloma-geometry, tetraloma-device and tetraloma.** This is the design text below. Its cost is seven milestones, M11–M17, of which M17 (sum factorisation) is the largest.
4. **One crate for elements and functions.** Element definitions must stay host-only, f64 and free of CubeCL, like tetraloma-refcell (section 6). Merging them into a kernel crate would lose that.

Two variants of option 3 were considered and are not proposed:

- **CubeCL's native complex types.** In CubeCL 0.11.0 only the CUDA runtime registers `ComplexUsage` (cubecl-cuda, `runtime.rs`); the wgpu, CPU and HIP runtimes do not.
- **The geometry as a grid function** (Firedrake). It would invert the crate graph, and entity-derived numbering would force collective edge and face creation at every mesh construction.

## Recommendation

Option 3. It delivers the three extensions the maintainer asked for, in the shape the discussion settled. It builds on what tetraloma already has: the orthonormal sets and LU of tetraloma-refcell, the strategies and degree policy of M9h, `LaunchPlan::per_group`, IndexMaps and `Scatterer`. It keeps ndelement in `conformance/` only.

The numbers the discussion did not fix are proposed here; please confirm or change them:

- the degree ranges of section 26: host construction up to p = 20 on tensor cells, 15 on simplices and prisms, 8 on pyramids;
- the device verification range: p ≤ 8 on every cell, as ADR-0041 set for geometry, with pyramids at p ≤ 3; p ≤ 16 for sum factorisation on quadrilaterals and hexahedra (section 30);
- the comparison benchmarks: CEED BP1 and BP3 at p = 4 and 8 only (M17), following ADR-0057's minimum runs.

## Changes if accepted

- **Design sections:**
    - new sections 26 (Finite elements), 27 (Quadrature rules and point sets), 28 (Function spaces and dofmaps), 29 (Grid functions and their evaluation), 30 (Sum-factorised evaluation), 31 (Interpolation) and 32 (Output of grid functions);
    - edits to sections 1, 2, 4, 6, 8, 10, 14, 16, 18, 19, 20 (A.10), 22, 23, 24 and 25.
  
  The design text is in parts A and B below, the roadmap text in part C. Headings in parts A and C are shown two levels deeper than they will appear in `docs/design.md` and `docs/roadmap.md`; applying the ADR promotes them (#### to ##, ##### to ###).
- **Interfaces and tests:**
    - **New crates:** tetraloma-element (Listing E1) and tetraloma-function (Listings F1–F3); the quadrature additions are Listing Q1.
    - **tetraloma-refcell:** public `polyset` and `DenseLu`; `QuadratureType` and `quadrature_rule`; `sub_entity_points_to_cell`, `rule_permutation` and `permute_vertices`; `NodeFamily` becomes `#[non_exhaustive]`; `Gll` is accepted on triangles, tetrahedra and prisms.
    - **tetraloma-geometry:** `QuadRule::new`, `FacetQuadRule::new`, `PointSet::from_points`, `PointSet::tabulate_element` and `CellPoints`. Hidden hooks under dependency rule 7: the device basis evaluator's `#[cube]` functions.
    - **tetraloma-device:** six `KernelKind` variants; `GridError::UnsupportedElement` and `GridError::SpaceMismatch`. Hidden hooks under dependency rule 7: `Strategy` and the degree-policy reader, moved from tetraloma-geometry.
    - **tetraloma:** `Mesh::create_entities_for` and `VtkWriter::function`.
    - **Tests:** the refcell test that expects `UnsupportedNodeFamily` for Gll on a triangle is replaced by M11's tests. `scripts/check-workspace-deps.sh` checks that tetraloma-element has no CubeCL dependency. `scripts/check-user-docs.sh` audits eleven published crates instead of nine. The new protected paths of section 23 get their checks.
- **Process documents:** `AGENTS.md` rule 11 (Code and Review focus: "nine published library crates" becomes eleven; the review checks dependency rule 8).
- **Plans and tasks affected:** none; no milestone is open.
- **New tasks:** milestones M11–M17 in `docs/roadmap.md` (part C). Their plans and issues follow the plan-milestone skill.

### A. New design sections

#### 26. Finite elements

##### Purpose

Finite elements in Ciarlet's sense on the reference cells of section 6: Lagrange elements for continuous and discontinuous Galerkin methods, and first-kind Nédélec (N1curl) and Raviart–Thomas (RT) elements, at arbitrary degree. They come with the maps between reference and physical cells for D_TOP ≤ D_GEO, and with the DOF transformations that make neighbouring cells agree. tetraloma-element computes them on the host in f64, independently of ndelement (fixed decision 4). Sections 27–32 and downstream FEM and BEM crates use them (ADR-0071).

##### Interface

**Listing E1 — finite elements (tetraloma-element; additions to tetraloma-refcell)**

```rust
// tetraloma-element: host only, f64; depends on tetraloma-dims and tetraloma-refcell.

/// One value per family on every cell type. On quadrilaterals and hexahedra, N1curl and
/// RaviartThomas are the tensor-product spaces (DefElement: RTCE and RTCF; NCE and NCF).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family { Lagrange, N1curl, RaviartThomas }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Point evaluations at a tetraloma-refcell point family.
    Nodal(NodeFamily),
    /// Lagrange: orthonormal basis with integral-moment DOFs; discontinuous spaces only.
    /// N1curl and RaviartThomas: Legendre moments on every sub-entity; their only variant.
    Legendre,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapType { Identity, CovariantPiola, ContravariantPiola, L2Piola }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformationKind { Identity, SignedPermutation, Dense }

/// Generators of a sub-entity's reorientations (A.6): reversal of an interval; rotation and
/// reflection of a triangle or quadrilateral.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Generator { Reversal, Rotation, Reflection }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementKey { pub family: Family, pub cell: CellType, pub degree: u32, pub variant: Variant }

/// A Ciarlet element on one reference cell, in f64. Built once per key, kept in a process-wide
/// thread-safe cache.
pub struct Element { /* private */ }
impl Element {
    pub fn get(key: ElementKey) -> Result<Arc<Element>, ElementError>;
    pub fn key(&self) -> ElementKey;
    pub fn map_type(&self) -> MapType;
    /// 1 for Lagrange; tdim for N1curl and RaviartThomas.
    pub fn reference_value_size(&self) -> usize;
    /// Number of DOFs.
    pub fn dim(&self) -> usize;
    pub fn entity_dofs(&self, d: usize, e: usize) -> &[usize];
    pub fn entity_closure_dofs(&self, d: usize, e: usize) -> &[usize];
    pub fn discontinuous_only(&self) -> bool;
    /// The smallest Lagrange degree whose space contains this element's space.
    pub fn embedded_superdegree(&self) -> u32;
    /// The basis in tetraloma-refcell's orthonormal set: [dim][polyset dim · reference value size].
    pub fn coefficients(&self) -> &[f64];
    /// [derivative][point][basis][component], derivatives in tetraloma-refcell's order up to nderivs.
    pub fn tabulate(&self, points: &[f64], nderivs: u32) -> Result<ElementTabulation, ElementError>;
    /// Reference points [m][tdim] and the matrix [dim][m · reference value size] such that the
    /// DOF values of a reference function û are M û(points).
    pub fn interpolation_points(&self) -> &[f64];
    pub fn interpolation_matrix(&self) -> &[f64];
    pub fn transformation_kind(&self) -> TransformationKind;
    /// None if the element has no DOFs on sub-entities of that type.
    pub fn base_transformation(&self, entity: CellType, generator: Generator) -> Option<&BaseTransformation>;
    /// Quadrilaterals and hexahedra only.
    pub fn tensor_factors(&self) -> Option<&TensorFactors>;
}

pub struct ElementTabulation {
    pub nderivs: u32, pub num_points: usize, pub dim: usize, pub value_size: usize,
    pub values: Vec<f64>,   // [derivative][point][basis][component]
}

/// Acts on the DOFs of one sub-entity.
pub enum BaseTransformation {
    SignedPermutation { perm: Vec<u32>, negate: Vec<bool> },
    Dense { n: usize, matrix: Vec<f64> },   // row-major n × n
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Factor1d { NodalH1(NodeFamily), MomentH1, LegendreL2 }

pub struct TensorFactors {
    pub factors: Vec<Vec<(Factor1d, u32)>>,   // [component][direction]: kind and degree
    pub lexicographic: Vec<Vec<usize>>,       // [component][tensor index] -> element DOF
}
impl TensorFactors {
    /// [derivative][point][basis] of one 1D factor at points of [0, 1].
    pub fn tabulate_1d(&self, component: usize, direction: usize, points: &[f64], nderivs: u32) -> Vec<f64>;
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ElementError {
    Unsupported { family: Family, cell: CellType, degree: u32, variant: Variant },
    RefCell(RefCellError),
}

// tetraloma-refcell: additions.
pub mod polyset {
    /// tetraloma-refcell's orthonormal set of `degree` on `cell`: [derivative][basis][point].
    pub fn tabulate(cell: CellType, degree: u32, points: &[f64], nderivs: u32) -> Vec<f64>;
    pub fn dim(cell: CellType, degree: u32) -> usize;
}
/// Dense LU with partial pivoting, in f64.
pub struct DenseLu { /* private */ }
impl DenseLu {
    pub fn factor(a: Vec<f64>, n: usize) -> Result<Self, RefCellError>;
    pub fn solve(&self, b: &mut [f64], nrhs: usize);
}
// NodeFamily becomes #[non_exhaustive]; NodeFamily::Gll is accepted on triangles, tetrahedra and
// prisms (section 6).
```

tetraloma-function converts `ElementError` into `GridError::UnsupportedElement { family, cell, degree, variant }`. The fields are `&'static str` names and the degree, because tetraloma-device depends on neither crate.

##### Behaviour and invariants

**Families, cells and degrees:**

| Cell | Lagrange (H1), DG | N1curl (H(curl)) | RT (H(div)) | Degrees |
| --- | --- | --- | --- | --- |
| interval | P_p | – | – | 1–20 (DG 0–20) |
| triangle | P_p | N1curl_p | RT_p | 1–15 (DG 0–15) |
| quadrilateral | Q_p | RTCE_p | RTCF_p | 1–20 (DG 0–20) |
| tetrahedron | P_p | N1curl_p | RT_p | 1–15 (DG 0–15) |
| hexahedron | Q_p | NCE_p | NCF_p | 1–20 (DG 0–20) |
| prism | the DefElement prism space | – | – | 1–15 (DG 0–15) |
| pyramid | DefElement Definition 9 | – | – | 1–8 (DG 0–8) |

Any other combination of family, cell, degree and variant returns `ElementError::Unsupported`. Sections 29 and 30 state the degrees up to which the device kernels are verified.

**Degree convention.** The degree is the polyset superdegree, as in Basix:

- degree 1 is the lowest-order Lagrange, N1curl, RT, RTCE, RTCF, NCE and NCF;
- degree 0 is the lowest DG.

With this convention, the traces of degree-p elements on a shared entity are the same space for every pair of cell types.

**Construction:**

- **Polynomial sets** are coefficients in tetraloma-refcell's orthonormal sets. Vector spaces are obtained by projection with a rule exact for the products:
    - N1curl on simplices, degree p: P_{p−1}^d ⊕ {q ∈ P̃_p^d : q · x = 0};
    - RT on simplices, degree p: P_{p−1}^d ⊕ x P̃_{p−1};
    - on quadrilaterals, RTCE_p has components in Q_{p−1,p} × Q_{p,p−1}, and RTCF_p the reverse; on hexahedra, the analogous NCE_p and NCF_p.
- **DOFs** are point evaluations (Nodal variants) or integral moments (Legendre, and every N1curl and RT element). Moments on a sub-entity are taken against an orthonormal basis of the sub-entity's moment space. That space is defined on the sub-entity itself and mapped into the cell by the sub-entity map, so cells of different types define identical DOFs on a shared entity.
    - N1curl: tangential moments against P_{p−1} on edges; tangential vector moments against P_{p−2}² on faces; moments against P_{p−3}³ in the interior of tetrahedra and P_{p−2}² in the interior of triangles; tensor-product Legendre moments on quadrilaterals and hexahedra.
    - RT: normal moments against P_{p−1} on facets; moments against P_{p−2}^d in the interior; tensor-product Legendre moments on tensor cells.
- **The dual basis** comes from the matrix D_ij = l_i(ψ_j) over the orthonormal set ψ, inverted with `DenseLu` in f64.
- **Tensor cells** assemble their elements from 1D factors, so only 1D elements are inverted; the dense construction is used there only by tests, for p ≤ 8.

**Variants:**

- **Point families** are tetraloma-refcell's `NodeFamily` (section 6), shared with coordinate elements; point sets on tensor cells are products of 1D sets.
- **Element variants** are `Nodal(Equispaced)`, `Nodal(Gll)` and `Legendre`. `Nodal(Gll)` is unsupported on pyramids. A `Legendre` Lagrange element has only interior DOFs, so `discontinuous_only()` is true.
- **Defaults:** `Nodal(Gll)` for Lagrange function spaces; `Nodal(Equispaced)` for geometry, unchanged.
- **Conditioning:** applying an element's interpolation to its own basis gives the identity to 10^−12 for p ≤ 10 (Gll, Legendre, N1curl, RT) and for p ≤ 6 (Equispaced).

**Map types.** Reference values have 1 or TD components and physical values 1 or GD components, so embedded cells use the same formulas. J, K and det J are those of section 10; det J is the pseudo-determinant when D_TOP < D_GEO.

| Map type | Families | Push-forward u(x) | Pull-back û(ξ) | Derivative pushed forward |
| --- | --- | --- | --- | --- |
| Identity | Lagrange, DG | û | u | ∇u = Kᵀ ∇̂û |
| Covariant Piola | N1curl, RTCE, NCE | Kᵀ û | Jᵀ u | 3D: curl u = J curl̂ û / det J; 2D and surfaces: rot u = rot̂ û / det J |
| Contravariant Piola | RT, RTCF, NCF | J û / det J | det J · K u | div u = div̂ û / det J |
| L2 Piola | DG in exact sequences (chosen per space, section 28) | û / det J | det J · u | – |

- Curl, rot and divergence commute with the Piola maps, also on curved cells.
- Full gradients of Piola-mapped fields on curved cells need second derivatives of the geometry, which tetraloma does not compute; they are not offered.
- The pseudo-determinant is positive, so contravariant fields on manifolds need no orientation. Normals, where an operation needs them (section 31), carry the sign of the cell's orientation flag (section 10).

**DOF transformations:**

- **Computation.** Base transformations are computed numerically, per sub-entity type and generator, by evaluating the DOF functionals of the reoriented sub-entity on the basis.
- **Composition.** They are composed per cell from the permutation-info word (A.6), as in Scroggs, Dokken, Richardson and Wells (section 6). Because orientation is keyed on origin ids, every rank agrees on them.
- **Classification.** Each element records its `TransformationKind`:
    - Lagrange elements: permutations without negation (`Identity` where every permutation is trivial, as for p ≤ 2);
    - N1curl and RT on quadrilaterals and hexahedra, at every degree: signed permutations, because reversing an edge maps Legendre q_i to (−1)^i q_i and the symmetries of a quadrilateral permute tensor Legendre moments with signs;
    - N1curl and RT on tetrahedra at p ≥ 2: dense blocks on triangular faces; their edges are signed permutations.
- **Application.** Let T_c map the reference basis to cell c's basis. Evaluation applies T_c, transposed operations apply T_cᵀ, and interpolation applies T_c^{−ᵀ}. A signed permutation is orthogonal, so all three are the same signed permutation.

**Tensor factors.** On quadrilaterals and hexahedra, every element is, per reference component, a tensor product of 1D elements, because its DOFs are tensor products of 1D functionals. There are three kinds of factor:

- `NodalH1`: Lagrange at a point family; also the factor of nodal DG.
- `MomentH1`: endpoint evaluations plus interior Legendre moments up to degree p − 2; the factor of N1curl and RT in their continuous directions.
- `LegendreL2`: moments against P_{p−1}; their factor in the remaining direction, and the factor of Legendre DG.

The product of the factors reproduces the element's tabulation to 10^−12 for p ≤ 8.

##### Out of scope

- Second-kind families (N2curl, BDM, BDMCE, BDMCF), DPC and serendipity families.
- Crouzeix–Raviart and enriched (bubble) elements; Regge and HHJ (the double Piola maps); Buffa–Christiansen and other dual-mesh elements.
- N1curl and RT on prisms and pyramids.
- Point-evaluation DOFs for N1curl and RT; hierarchical bases for p-adaptivity; element construction on the device.

#### 27. Quadrature rules and point sets

##### Purpose

High-order quadrature on every reference cell: symmetric rules on simplices, GLL rules on tensor cells, rules on sub-entities of every dimension, and facet rules that match from both sides of an interior facet. Point sets of the caller's choice and per-cell reference points get the same device tabulation as built-in rules; they are the hooks that a future BEM crate builds its singular quadrature on (ADR-0071).

##### Interface

**Listing Q1 — quadrature and point sets (additions to tetraloma-refcell and tetraloma-geometry)**

```rust
// tetraloma-refcell
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QuadratureType { #[default] Default, Gauss, Symmetric, GaussLobatto }

pub fn quadrature_rule(cell: CellType, degree: u32, kind: QuadratureType) -> Result<QuadratureRule, RefCellError>;
/// Points of the reference sub-entity (d, e) of `cell`, in cell coordinates.
pub fn sub_entity_points_to_cell(cell: CellType, d: usize, e: usize, points: &[f64]) -> Result<Vec<f64>, RefCellError>;
/// Index permutation of a symmetric rule on `entity` under the reorientation of its A.6 bits.
pub fn rule_permutation(entity: CellType, rule: &QuadratureRule, bits: u32) -> Result<Vec<u32>, RefCellError>;
/// Reference points of `cell` mapped under a permutation of its vertices.
pub fn permute_vertices(cell: CellType, perm: &[usize], points: &[f64]) -> Result<Vec<f64>, RefCellError>;

// tetraloma-geometry
impl<F: Real, TD: TopoDim> QuadRule<F, TD> {
    pub fn new(client: &Client, cells: &[TD::Cell], degree: u32, kind: QuadratureType) -> Result<Self>;
    // gauss(…) stays and equals new(…, QuadratureType::Gauss).
}
impl<F: Real, TD: TopoDim> FacetQuadRule<F, TD> {
    pub fn new(client: &Client, cells: &[TD::Cell], degree: u32, kind: QuadratureType) -> Result<Self>;
}
impl<F: Real, TD: TopoDim> PointSet<F, TD> {
    /// Reference points [q][tdim] and weights of the caller's choice; weights may be empty.
    pub fn from_points(client: &Client, cell: TD::Cell, points: Vec<f64>, weights: Vec<f64>) -> Result<Self>;
    /// [derivative][q][k][component], computed in f64, narrowed to F, cached per ElementKey.
    pub fn tabulate_element(&self, e: &Element, nderivs: u32) -> Result<DeviceBuffer<F>>;
}
/// Per-cell reference points of one cell block: point i lies in local cell cells[i].
pub struct CellPoints<F: Real, TD: TopoDim> { /* block, cells: DeviceBuffer<u32>, points: [i][tdim] */ }
impl<F: Real, TD: TopoDim> CellPoints<F, TD> {
    pub fn new(client: &Client, block: usize, cells: DeviceBuffer<u32>, points: DeviceBuffer<F>) -> Result<Self>;
    pub fn block(&self) -> usize;
    pub fn len(&self) -> usize;
}
```

##### Behaviour and invariants

**Rule types:**

| Type | Triangles, tetrahedra | Intervals, quadrilaterals, hexahedra | Prisms, pyramids |
| --- | --- | --- | --- |
| `Default` | `Symmetric` for degree 1–30; `Gauss` for 31–60 | `Gauss` | `Gauss` |
| `Gauss` | `gauss_rule` (section 10), degree 0–60 | as left | as left |
| `Symmetric` | fully symmetric tables (Xiao–Gimbutas), degree 1–30 | – | – |
| `GaussLobatto` | – | n = 2–41 points per direction, n = ⌈(degree + 3) / 2⌉, exact to degree 2n − 3 | – |

- Any other request returns `RefCellError::UnsupportedDegree`, or `UnsupportedNodeFamily` for a type on a cell it does not cover.
- **Point and weight properties.** Points of `Gauss` and `Symmetric` rules lie in the interior of the cell and their weights are positive. `GaussLobatto` points include the cell's boundary, and its weights are positive.
- **GLL mass matrices.** With the `Nodal(Gll)` Lagrange element of degree n − 1, the n-point `GaussLobatto` rule gives a diagonal mass matrix.
- **Table provenance.** The `Symmetric` tables live in `tetraloma-refcell/tables/`. Each records its source, the version or commit taken, and its licence in `tables/sources.json`. They are copied only under a licence compatible with MIT/Apache-2.0. A table that fails this check is not shipped, and `Default` uses `Gauss` for its degrees.

**Rules on sub-entities.** `sub_entity_points_to_cell` generalises `facet_points_to_cell` to every sub-entity dimension. Integral-moment DOFs on edges of 3D cells and on faces need it, and so does interpolation into them.

**Interior facets.** Every facet rule maps onto itself under its facet's symmetry group: Gauss rules on intervals and quadrilaterals, `Symmetric` rules on triangles. `FacetQuadRule::new` with `Default` therefore supports degree 1–30 on triangular facets. Two cells sharing a facet see the same physical points after the index permutation `rule_permutation` returns for the facet's A.6 bits. There are 2 cases on intervals, 6 on triangles and 8 on quadrilaterals. Tabulations stay per reference facet.

**Point sets of the caller's choice:**

- `PointSet::from_points` gets the device tabulation cache of section 10.
- `tabulate_element` tabulates any `Element`. The upload is a tabulation upload in the sense of section 22.
- `CellPoints` evaluation (section 29) uses the device basis evaluator of section 10 with `Element::coefficients`, uploaded once per element.
- `permute_vertices` maps reference points under a vertex permutation. Singular quadrature and submesh transfers (section 31) use it.

##### Out of scope

- Singular and near-singular quadrature (Sauter–Schwab, Duffy): a future BEM crate builds it on `PointSet::from_points` and `permute_vertices`.
- Adaptive and nested rules; symmetric rules on prisms and pyramids; rules on curved sub-entities.

#### 28. Function spaces and dofmaps

##### Purpose

Function spaces on any `DeviceGrid`: an element per cell type, a global DOF numbering that follows the mesh's IndexMap conventions, and the device data that evaluation and interpolation need (ADR-0071).

##### Interface

**Listing F1 — function spaces (tetraloma-function; additions to tetraloma and tetraloma-device)**

```rust
// tetraloma-function
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Continuity { Continuous, Discontinuous }

/// Family, degree and variant; one Element per cell type of a grid. `map` overrides the family's
/// map type: None for its own, Some(MapType::L2Piola) for discontinuous Lagrange only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementFamily { pub family: Family, pub degree: u32, pub variant: Variant, pub map: Option<MapType> }
impl ElementFamily {
    pub fn element(&self, cell: CellType) -> Result<Arc<Element>>;
    /// Entity dimensions that carry DOFs for these cell types; empty for Discontinuous.
    pub fn dof_dimensions(&self, cells: &[CellType], continuity: Continuity) -> Vec<usize>;
}

pub struct FunctionSpace<'g, G: DeviceGrid> { /* grid, family, continuity, block shape, dofmaps, IndexMap, Scatterer */ }
impl<'g, G: DeviceGrid> FunctionSpace<'g, G> {
    /// Err(EntitiesNotCreated(d)) for the first dimension the space needs that has not been created.
    pub fn new(grid: &'g G, family: ElementFamily, continuity: Continuity, block_shape: &[usize]) -> Result<Self>;
    /// The mesh geometry as a vector Lagrange space of block shape [GD], without copying.
    pub fn geometry(grid: &'g G) -> Result<Self>;
    pub fn grid(&self) -> &'g G;
    pub fn family(&self) -> ElementFamily;
    pub fn continuity(&self) -> Continuity;
    pub fn block_shape(&self) -> &[usize];
    /// Product of the block shape.
    pub fn block_size(&self) -> usize;
    /// Per cell block, indexed like cell_blocks().
    pub fn dofmap(&self, b: usize) -> &DofMap;
    /// [owned | ghost] DOFs (one entry per DOF, not per block component).
    pub fn index_map(&self) -> &IndexMap;
    pub fn num_global_dofs(&self) -> u64;
}

pub struct DofMap {
    pub element: Arc<Element>,
    pub dofs: DeviceStrided<u32>,           // cells × DOFs per cell; signed permutations folded in
    pub signs: Option<DeviceStrided<u32>>,  // bitset per cell; None if no sign is negative
    pub writers: DeviceStrided<u32>,        // bitset per cell: the slots that write shared DOFs
    pub dense_transforms: bool,             // the DofTransform kernel runs for this block
}

// tetraloma (facade)
impl<F: Real, TD: TopoDim + EmbeddedIn<GD>, GD: Dim, C: Comm> Mesh<F, TD, GD, C> {
    /// Creates every entity dimension a space of this family needs, on every rank (collective).
    pub fn create_entities_for(&mut self, family: &ElementFamily, continuity: Continuity) -> Result<()>;
}

// tetraloma-device: GridError gains
//   UnsupportedElement { family: &'static str, cell: &'static str, degree: u32, variant: &'static str }
//   SpaceMismatch(&'static str)
```

##### Behaviour and invariants

- **Entity-numbered dofmaps.** Entity blocks are homogeneous by type, so every (dimension, entity block) pair carries a fixed number k of DOFs; a prism mesh's triangular and quadrilateral faces simply differ in k.
    - Local DOF numbers are block offset + entity index × k + i, with owned DOFs first and ghosts after, in the order of the entity IndexMaps.
    - A ghost DOF's global number follows from its entity's global number and the owner's per-block offsets. These offsets are exchanged once per space, through the host-staged neighbourhood exchange (section 22); there is no per-DOF communication.
    - Discontinuous spaces use only the cell IndexMaps.
- **Geometry-numbered dofmaps.** `FunctionSpace::geometry` wraps each geometry block's dofmap and the node IndexMap, with the coordinate element as Lagrange element (same degree and node family) and block shape [GD].
- **Entity creation stays explicit.** `new` and the accessors create no topology (section 4), and entity creation is collective. `create_entities_for` creates exactly the dimensions `dof_dimensions` lists, on every rank.
- **Signs and transformations.**
    - Signed-permutation transformations (section 26) are folded into `dofs` once per space: an entry holds the global-orientation slot, and `signs` holds the sign.
    - Elements with dense blocks keep the cell-local order and set `dense_transforms`; evaluation then applies the blocks in a separate kernel (section 29).
- **Transposed dofmap.** Built once per space with the radix sort of section 7: for each DOF, its (cell, slot) pairs in increasing order, as offsets and slots. It drives accumulation from E-vectors to the L-vector.
- **Writer bits.** For each owned DOF, exactly one slot is flagged: that of the touching cell with the lowest local index on the owning rank, found with a segmented minimum over the transposed dofmap. Interpolation writes only flagged slots (section 31).
- **Limits:**
    - local DOFs per rank below 2^32, otherwise `TooLarge`;
    - global DOFs below 2^63;
    - block shapes of rank 0, 1 or 2 with extents 1–3.
- **Errors:**
    - a `discontinuous_only` element, or `map = Some(L2Piola)`, in a `Continuous` space gives `SpaceMismatch`;
    - an unsupported element gives `UnsupportedElement`.
- **Determinism.** Numbering, signs, writer bits and the transposed dofmap are identical in every run on the same rank count. Global DOF counts do not depend on the rank count.

##### Out of scope

DOF reordering for bandwidth; degrees that vary per cell; product (mixed) spaces; constraints such as hanging nodes and periodicity.

#### 29. Grid functions and their evaluation

##### Purpose

Device storage of functions in a space, and their evaluation at quadrature points and at per-cell reference points, with the push-forwards of section 26. It also provides the transposed operation, integration against the basis, which load vectors, matrix-free operators and the comparison benchmarks of section 30 need (ADR-0071).

##### Interface

**Listing F2 — grid functions (tetraloma-function)**

```rust
/// Coefficient scalars: f32, f64, num_complex::Complex<f32>, num_complex::Complex<f64>.
pub trait FieldScalar: Copy + Default + bytemuck::Pod + Send + Sync + 'static {
    type Real: Real;
    const PARTS: usize;   // 1 real, 2 complex
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op { Value, Grad, Curl, Div }

/// Values at points per cell block: the QuadField layout with value size × PARTS components.
pub struct FunctionFields<F: Real> { /* blocks of tiled chunks, components */ }
impl<F: Real> FunctionFields<F> {
    pub fn components(&self) -> usize;
    pub fn num_points(&self, b: usize) -> usize;
    pub fn block(&self, b: usize) -> &[QuadChunk<F>];
    /// Host copy of block b in plain [cell][point][component] order (an explicit download).
    pub fn to_host_cell_major(&self, b: usize) -> Result<Vec<F>>;
}

pub struct GridFunction<'a, G: DeviceGrid, T: FieldScalar<Real = G::F>> { /* space, L-vector */ }
impl<'a, G: DeviceGrid, T: FieldScalar<Real = G::F>> GridFunction<'a, G, T> {
    pub fn zeros(space: &'a FunctionSpace<'a, G>) -> Result<Self>;
    /// [dof][component][part], owned and ghost DOFs.
    pub fn from_coefficients(space: &'a FunctionSpace<'a, G>, values: DeviceBuffer<G::F>) -> Result<Self>;
    pub fn space(&self) -> &FunctionSpace<'a, G>;
    pub fn coefficients(&self) -> &DeviceBuffer<G::F>;
    pub fn coefficients_mut(&mut self) -> &mut DeviceBuffer<G::F>;
    /// Owner → ghosts (forward scatter).
    pub fn update_ghosts(&mut self) -> Result<()>;
    /// Ghosts → owner, Add, then zero the ghosts (reverse scatter).
    pub fn accumulate_ghosts(&mut self) -> Result<()>;
    pub fn evaluate(&self, rule: &QuadRule<G::F, G::TD>, op: Op) -> Result<FunctionFields<G::F>>;
    pub fn evaluate_at(&self, points: &CellPoints<G::F, G::TD>, op: Op) -> Result<FunctionFields<G::F>>;
}

/// A read-only function that shares its coefficients, such as the geometry.
pub struct GridFunctionRef<'a, G: DeviceGrid, T: FieldScalar<Real = G::F>> { /* space, shared L-vector */ }
impl<'a, G: DeviceGrid> GridFunctionRef<'a, G, G::F> {
    /// Requires a space from FunctionSpace::geometry; values relative to origin().
    pub fn geometry(space: &'a FunctionSpace<'a, G>) -> Result<Self>;
}

/// Implemented by GridFunction and GridFunctionRef: what evaluation and interpolation can read.
pub trait Source<G: DeviceGrid> { /* space and coefficients; sealed */ }

impl<'g, G: DeviceGrid> FunctionSpace<'g, G> {
    /// b_i = Σ_cells Σ_q w_q · |det J| · values(x_q) · op(φ_i)(x_q) for every owned DOF i, summed
    /// in a fixed order and reverse-scattered to the owners.
    pub fn integrate_basis(&self, rule: &QuadRule<G::F, G::TD>, values: &FunctionFields<G::F>, op: Op)
        -> Result<DeviceBuffer<G::F>>;
}
```

##### Behaviour and invariants

**Layouts:**

- **L-vector:** interleaved [dof][component][part]. The `Scatterer` block size is the block size × `PARTS`, so the halo exchange and reverse addition of section 11 apply unchanged.
- **E-vector:** [k][component][part][cell], cells fastest, tiled and padded like the geometry E-vector (section 8, "Padding and tiles").
- **`FunctionFields`:** the `QuadField` layout of Listing B4b with a runtime component count.

**Complex values.** Complex arithmetic is written out in real operations in a fixed order, and a comptime flag doubles the component loops. CubeCL's native complex types are not used: in 0.11.0 only the CUDA runtime registers `ComplexUsage`.

**Pipeline:**

| Step | Kernel family (`KernelKind`, degree-policy family) | Buffers |
| --- | --- | --- |
| L → E gather, signed permutations | FunctionGather | L-vector, dofs, signs, E-vector |
| Dense DOF transformations (tetrahedra, p ≥ 2) | DofTransform | E-vector, permutation info, transformation blocks |
| E → Q contraction and push-forward, real | FunctionEval | E-vector, packed tabulation, J or K, det J, output |
| E → Q contraction and push-forward, complex | FunctionEvalComplex | as FunctionEval |
| Q → E transpose (`integrate_basis`) | FunctionEval, FunctionEvalComplex | as FunctionEval, with weights in the tabulation buffer |
| E → L accumulation, fixed order | FunctionAccumulate | E-vector, slot offsets, slots, signs, L-vector |

- **Six buffers.** DOF transformations run before evaluation, never inside it, so every kernel binds at most six buffers (section 22).
- **Strategies.** FunctionEval and FunctionEvalComplex run the strategies of section 10:
    - **direct, loops:** as for geometry;
    - **shared:** the tabulation in shared memory;
    - **cooperative:** a group of units per cell, planned by `LaunchPlan::per_group`, with the cell's coefficients in shared memory and one unit per point and component;
    - **sumfact:** on quadrilaterals and hexahedra, section 30.
- **Degree policy.** tetraloma-function's `degree-policy.toml` chooses a strategy per family, cell type, degree range, device class and scalar size, under the generic decision rule of section 8, and holds the four sets of ADR-0058. `TETRALOMA_FUNCTION_STRATEGY` (the syntax of `TETRALOMA_GEOM_STRATEGY`) and the hidden `tetraloma_function::testing::with_strategy` override it. A strategy that does not apply falls back to loops.
- **Verified range.** Evaluation is verified for p ≤ 8 on intervals, triangles, quadrilaterals, tetrahedra, hexahedra and prisms, and p ≤ 3 on pyramids (as geometry; ADR-0041). The tolerances are those of section 16.
- **Operations:**
    - `Value`;
    - `Grad`, for identity-mapped families;
    - `Curl`, for covariant families (rot in 2D and on surfaces);
    - `Div`, for contravariant families;

  each with the push-forward of section 26. Any other combination gives `SpaceMismatch`.
- **Per-cell points.** `evaluate_at` evaluates the basis at the points of `CellPoints` with the device basis evaluator of section 10 and `Element::coefficients`, then pushes forward as above.
- **Geometry view.** `GridFunctionRef::geometry` shares `nodes()`; its values are relative to `origin()`, as `nodes()` are.
- **Determinism.** Each output value is written by one unit, summing in a fixed order, so results are bitwise reproducible for a fixed configuration. `FunctionAccumulate` sums each DOF's slots in the order of the transposed dofmap. Results are bitwise identical across vector sizes on the CPU runtime (section 8).

##### Out of scope

- Assembly of matrices; operators and solvers.
- Full gradients of Piola-mapped fields on curved cells.
- Evaluation at physical points (point location) and on non-matching meshes.
- Updating the geometry from a grid function, and curved meshes of higher degree built from one.

#### 30. Sum-factorised evaluation

##### Purpose

Evaluation and its transpose at high degree on quadrilaterals and hexahedra, where a dense contraction costs (p + 1)^(2·TD) operations per component and cell (ADR-0071).

##### Interface

No new public items. The strategy is `sumfact` in tetraloma-function's degree policy (section 29), selected by the policy or by its overrides.

##### Behaviour and invariants

- **Applies to:** FunctionEval and FunctionEvalComplex, with their transposes, for every family of section 26 on quadrilaterals and hexahedra, using the tensor factors of section 26 and tensor rules (`Gauss`, `GaussLobatto`).
- **GPUs (cooperative slab).** One group per cell, planned by `LaunchPlan::per_group`. On hexahedra, units cover a 2D slab of n × n entries, n the larger of p + 1 and the points per direction. Each unit holds a register column along the third direction.
    - The third-direction contraction runs in registers.
    - The other two exchange values through one shared n × n slab per cell, one level at a time, with `sync_cube` in uniform control flow between steps.
    - Shared memory is n² · size_of::<F>() per cell: 648 bytes at p = 8 in f64. Where n² exceeds the units a group can have (256 on WebGPU), each unit handles several slab entries.
    - On quadrilaterals the slab is the whole cell.
- **CPUs.** Lanes over cells, as in section 8, with the contractions per lane in local arrays.
- **Verified range:** p ≤ 16 on quadrilaterals and hexahedra, agreeing with the loops strategy to the tolerances of section 16.
- **Limits.** Every plan stays within `max_units_per_cube`, `max_cube_dim` and the shared-memory limit, checked by `LaunchPlan`. A degree whose plan does not fit is not selectable on that device.
- **Determinism** as in section 29.

##### Out of scope

Sum factorisation on simplices (collapsed coordinates), prisms and pyramids; tensor-core (MMA) instructions; matrix-free operators beyond `integrate_basis`.

#### 31. Interpolation

##### Purpose

Grid functions created by interpolation: from functions given as device code, from other spaces on the same grid, through the `EntityMap` of a submesh in either direction, and from data that exist only on the host (ADR-0071).

##### Interface

**Listing F3 — interpolation (tetraloma-function)**

```rust
/// A function of physical coordinates, evaluated on the device by interpolate().
pub trait DeviceExpression: Send + Sync + 'static {
    /// Values per point: the value shape's product, times 2 for complex values.
    fn num_values(&self) -> usize;
    /// Runtime parameters (for example a wavenumber), uploaded once per call; the kernel receives
    /// origin() (GD values) first, then these.
    fn params(&self) -> Vec<f64> { Vec::new() }
    /// True if eval never branches on lane values, so that it may run at vector sizes above 1.
    fn vectorizable(&self) -> bool { false }
    // The #[cube] method (provisional; the first task of M16 checks it against CubeCL 0.11):
    // fn eval<F: Float, N: Size>(x: &Array<Vector<F, N>>, params: &Slice<F>, out: &mut Array<Vector<F, N>>);
    // x relative to origin(); lanes are points of consecutive cells.
}

impl<'g, G: DeviceGrid> FunctionSpace<'g, G> {
    /// Physical interpolation points per cell block, relative to origin().
    pub fn interpolation_points(&self) -> Result<FunctionFields<G::F>>;
}

impl<'a, G: DeviceGrid, T: FieldScalar<Real = G::F>> GridFunction<'a, G, T> {
    pub fn interpolate<E: DeviceExpression>(&mut self, e: &E) -> Result<()>;
    /// Another space on the same grid, optionally through Grad, Curl or Div of the source.
    pub fn interpolate_from<S: Source<G>>(&mut self, source: &S, op: Op) -> Result<()>;
    /// Parent to child (traces) or child to parent (lifting) through a submesh's EntityMap;
    /// `normal_cross` applies n × to a covariant trace.
    pub fn interpolate_through<H: DeviceGrid<F = G::F>, S: Source<H>>(
        &mut self, map: &EntityMap, source: &S, op: Op, normal_cross: bool) -> Result<()>;
    /// Values at interpolation_points(), [point][component][part] per cell block.
    pub fn interpolate_values(&mut self, values: &FunctionFields<G::F>) -> Result<()>;
    /// Host round trip: absolute points to the host, values back (section 22).
    pub fn interpolate_host(&mut self, f: impl FnMut(&[f64], &mut [T])) -> Result<()>;
}
```

##### Behaviour and invariants

- **Element interpolation.** Each element provides interpolation points and the matrix M of section 26. The device pipeline per cell is:
    1. physical points from the factor kernels;
    2. source values;
    3. pull-back (Jᵀu, det J · K u or det J · u);
    4. multiplication by M;
    5. the transformation T_c^{−ᵀ};
    6. the write of the flagged slots.

  Interpolation is exact on the space: a function of the space is reproduced to the tolerances of section 16.
- **Sources:**
    - **Expressions** run at vector size 1 unless `vectorizable()` is true, and then at the size of the Interpolate policy entry.
    - **Same-grid sources** are tabulated at the target's reference points; no point location is involved. A source on a different grid gives `SpaceMismatch`.
    - **`interpolate_through`.** From parent to child (`boundary`, `facet_submesh`, `cell_submesh`):
        - the child cell's reference points are related to the parent facet's through the facet map and `permute_vertices`;
        - a covariant pull-back on the child projects onto the tangent plane, which gives the tangential trace;
        - `normal_cross` applies n × with the child's outward normal (section 15), which gives the rotated trace into surface RT.

      From child to parent, values are written into the DOFs of the parent's entities that the child covers; other DOFs keep their values. Facet children need the parent's (tdim − 1, tdim) connectivity, otherwise `ConnectivityNotCreated`.
    - **Host data.** `interpolation_points` and `interpolate_values` are the building blocks. `interpolate_host` does exactly one download of the points and one upload of the values, and adds `origin()` for the closure.
- **Shared DOFs.** Only flagged slots are written (section 28), with no races and no atomics. A forward scatter then fills the ghosts. The result is bitwise deterministic for a fixed configuration; across different partitions it can differ at rounding level.
- **Complex targets** take complex expressions (`num_values` doubled) and complex or real sources.

##### Out of scope

- Interpolation between non-matching meshes and evaluation at physical points (point location).
- L2 projection and averaging quasi-interpolants.
- Traces on edge submeshes.
- Interpolation into the geometry.

#### 32. Output of grid functions

##### Purpose

Grid functions in VTK output, so they can be inspected in ParaView (ADR-0071).

##### Interface

```rust
// tetraloma (VtkWriter, Listing B13)
impl<'a> VtkWriter<'a> {
    /// Adds a grid function to the next write; all functions of one write share one output space.
    pub fn function<G: DeviceGrid, S: Source<G>>(self, name: &str, f: &'a S) -> Self;
}
```

##### Behaviour and invariants

- **Direct output.** A continuous Lagrange function whose degree and node family equal the geometry's goes out as point data indexed by geometry node, like `.point_data`.
- **Output space.** Otherwise every function of the write is interpolated on the device into one equispaced Lagrange space of degree max(p_geometry, p of each function). The space is discontinuous if any function is not continuous Lagrange; N1curl and RT go into vector DG.
- **Output points.** The output mesh's points are the geometry view (section 29) interpolated into the output space. Discontinuous output duplicates points per cell.
- **Mixing with buffers.** `.point_data` buffers cannot be combined with an output space other than the geometry's: `SpaceMismatch`.
- **Complex functions** are written as `<name>_re` and `<name>_im`.
- **Formats.** VTU, PVTU and VTKHDF, collective as in section 13. High-order pyramids keep the vertices-only rule of ADR-0024.

##### Out of scope

Gmsh `$NodeData` and `$ElementNodeData` output; checkpointing and restart; time series.

### B. Edits to existing design sections

Each edit carries the marker (ADR-0071) in the design.

**Section 1, Summary.** At the end of the Scope bullet, add:

> It also provides finite elements (Lagrange, DG, N1curl and RT at arbitrary degree; sections 26–27), and device-resident function spaces and grid functions with evaluation, interpolation and output (sections 28–32) (ADR-0071).

**Section 2, Requirements and decisions.**

- Requirements, new bullet after **High order**:

  > **Elements and grid functions:** Lagrange elements for continuous and discontinuous Galerkin methods on every cell; N1curl and RT elements on triangles, quadrilaterals, tetrahedra and hexahedra, at arbitrary degree; push-forwards and pull-backs for D_TOP ≤ D_GEO; high-order quadrature; function spaces and grid functions created, stored, evaluated and interpolated on the device (ADR-0071).

- Fixed decision 4 becomes:

  > tetraloma-refcell holds tetraloma's own tables, and tetraloma-element its own element definitions. ndelement is used only in the unpublished tetraloma-conformance crate (ADR-0071).

- Out of scope, new bullet:

  > **Singular quadrature for BEM** (Sauter–Schwab, Duffy) belongs to a future BEM crate, which builds on section 27's point sets (ADR-0071).

**Section 4, after Listing B7.** "Function spaces, assemblers and BEM operators are generic over `G: DeviceGrid`" becomes "Function spaces (section 28), assemblers and BEM operators are generic over `G: DeviceGrid` (ADR-0071)".

**Section 6, Reference cells and conventions.**

- The **Node families** bullet, second and third sentences, become:

  > Gauss–Lobatto–Legendre (GLL) points are offered on intervals, quadrilaterals and hexahedra (tensor GLL), on triangles and tetrahedra (Isaac's recursive GLL points, T. Isaac, SIAM J. Sci. Comput. 42(6), 2020, which restrict on every sub-entity to that sub-entity's GLL points), and on prisms (their product). Requesting GLL on a pyramid returns `GridError::UnsupportedNodeFamily`. `NodeFamily` is `#[non_exhaustive]` (ADR-0071).

- The **DOF transformations** bullet, new last sentence:

  > tetraloma-element computes them for its elements and classifies them (section 26) (ADR-0071).

**Section 8, Vectorization and CPU execution.**

- Listing S2, `KernelKind`: add `FunctionGather, DofTransform, FunctionEval, FunctionEvalComplex, FunctionAccumulate, Interpolate` before `Scalar` (ADR-0071).
- "Where vectors apply", new rows (ADR-0071):

| Kernel family (`KernelKind`) | Lanes over | Layout prerequisite | Expectation |
| --- | --- | --- | --- |
| FunctionGather | cells | cell-fastest E-vector | gathers from the L-vector; only the stores are vectors, like EVector |
| DofTransform | cells | cell-fastest E-vector | small dense products; to measure |
| FunctionEval, FunctionEvalComplex | cells (CPU); groups per cell (cooperative, sumfact on GPUs) | E-vector, tiled fields | dense arithmetic with broadcast tabulations on CPUs; complex doubles the accumulators, so it has entries of its own |
| FunctionAccumulate | DOFs | transposed dofmap | indirect reads; expected to stay at size 1 |
| Interpolate | cells | tiled points and factors | expression evaluation and pull-back; size 1 unless the expression is vectorizable |

- "Benchmarks", **Degree sweep (M9h)**, new last sentences:

  > From M15, tetraloma-function's families (section 29) are measured with the candidate protocol at p = 1, 4 and 8 on every cell type, and from M17 also at p = 16 on quadrilaterals and hexahedra. Their choices go to `tetraloma-function/degree-policy.toml` under the generic decision rule, and their costs to `docs/perf/high-order.md` (ADR-0071).

- "Benchmarks", new bullet **Comparison frameworks** after **Record**:

  > CEED bake-off problems BP1 (scalar mass) and BP3 (scalar Laplacian) are built in tetraloma-bench from `evaluate` and `integrate_basis`. They are run at p = 4 and 8 on hexahedra and compared, on the same machines, with MFEM's partial assembly or libCEED on Locust's H100 and with deal.II's matrix-free framework on one Kathleen socket. Throughput in DOFs per second, versions and build commands go to `docs/perf/function-comparison.md`. These frameworks are never test or workspace dependencies (ADR-0071).

**Section 10, Geometry.**

- "Tabulation and quadrature", **Quadrature** bullet, new last sentence:

  > `QuadratureType` adds symmetric rules on simplices and GLL rules on tensor cells; `PointSet::from_points` and `CellPoints` take points of the caller's choice (section 27) (ADR-0071).

- "Geometric factors", last paragraph, new last sentence:

  > `Strategy` and the reader of `degree-policy.toml` move to tetraloma-device as `#[doc(hidden)] pub` items (dependency rule 7), and the device basis evaluator's `#[cube]` functions become `#[doc(hidden)] pub` items of tetraloma-geometry, so that tetraloma-function uses the same strategies, policy rules and evaluator (ADR-0071).

**Section 14, User documentation.**

- "nine published library crates" becomes "eleven published library crates" throughout, and the Site bullet's list adds tetraloma-element and tetraloma-function (ADR-0071).
- Contents, new part after "element definitions":

  > finite elements and grid functions: element families, degrees and variants, maps and DOF transformations, quadrature types, function spaces, evaluation, interpolation and output (ADR-0071);

- Contents, tutorials, add "function spaces and interpolation; grid functions in VTK output" (ADR-0071).

**Section 16, Testing and validation.**

- New bullet after **Geometry** (ADR-0071):

  > - **Elements and grid functions:**
  >     - the dual-basis property and polynomial reproduction of every element;
  >     - DOF counts and sub-entity spans against the oracles of A.10;
  >     - the de Rham inclusions ∇ Lagrange_p ⊂ N1curl_p, curl N1curl_p ⊂ RT_p and div RT_p ⊂ DG_{p−1};
  >     - continuity across a shared entity for every relative orientation;
  >     - adjoint identities ⟨Bu, v⟩ = ⟨u, Bᵀv⟩ of gather and evaluation;
  >     - interpolation exact on the space, with convergence rates.

- **Parallel invariance**, new sub-bullet (ADR-0071):

  > from M14, global DOF counts and, from M16, L2 norms of interpolated functions (to 10^−12 in f64).

- **Conformance with ndelement** becomes:

  > **Conformance:** tetraloma-conformance against ndelement, and the Python oracles Basix, Symfem and DOLFINx (A.10); comparisons with other frameworks use invariants (DOF counts, spans, point values, integrals, L2 errors, convergence rates), never DOF numbering (ADR-0071).

**Section 18, Workspace layout.**

- Crate table: the tetraloma-refcell, tetraloma-geometry, tetraloma, tetraloma-bench and conformance/ rows add the items of sections 26–32. New rows (ADR-0071):

| Crate | Purpose | Depends on |
| --- | --- | --- |
| tetraloma-element | Finite elements in f64 (section 26): families, variants, Ciarlet construction, tabulation, entity DOFs, interpolation data, map types, base transformations, tensor factors. Host only; usable in `comptime!`. Tests read committed JSON fixtures. | tetraloma-dims, tetraloma-refcell; dev: serde_json |
| tetraloma-function | Function spaces, dofmaps, grid functions, evaluation, interpolation (sections 28–31); its degree policy; test hooks (feature `testing`) | dims, refcell, element, device, parallel, topology, geometry, cubecl; bytemuck; num-complex (feature `bytemuck`); dev: tetraloma-testing |

- tetraloma-geometry gains the dependency tetraloma-element; tetraloma gains tetraloma-element and tetraloma-function.
- Dependency rule 1 adds tetraloma-function. New rule 8 (ADR-0071):

  > tetraloma-element depends only on tetraloma-dims and tetraloma-refcell, has no CubeCL dependency and allocates nothing on a device.

- Cargo features, test-support bullet, new sentence (ADR-0071):

  > tetraloma-function's `testing` feature enables the hidden `tetraloma_function::testing` (strategy overrides), turned on by tetraloma-testing.

**Section 19, Risks and mitigations,** new rows (ADR-0071):

| Risk | Mitigation |
| --- | --- |
| Sum factorisation is the largest single item, and cooperative kernels behave differently on wgpu, Metal and CUDA | Loops and cooperative strategies first (M15); sumfact validated against loops in its own milestone (M17); plans checked against device limits; loops remains the fallback |
| Cost and conditioning of the dense Ciarlet construction at high p | Tensor cells built from 1D factors; f64 throughout; Gll and Legendre variants; dual-basis checks up to p = 10 |
| Licences of quadrature tables | Checked and recorded in `tables/sources.json` before copying; `Gauss` as the fallback |
| JIT compile time per element, degree and rule | Comptime specialisation per degree only; the 30 s first-use bound of ADR-0056 applies to the new families |
| A future pyramid H(curl) family incompatible with DefElement's Definition 9 Lagrange space | Recorded under Open questions; checked before such a family is scheduled |

**Section 20, A.10 Sources and conformance setup,** new paragraph (ADR-0071):

> - **ndelement** (the pin above): JSON fixtures for Lagrange tabulations, DOF layouts and base transformations, and for N1curl and RT spans and DOF counts.
> - **Basix** (pinned in the Python environment): Isaac GLL points (`gll_isaac`), Legendre variants and base transformations; its triangle and tetrahedron numbering is converted with A.11.
> - **Symfem:** DOF counts and spans at low degree, in DefElement numbering.
> - **DOLFINx** (pinned, ADR-0007): interpolation of expressions, point values, submesh transfers and L2 norms.
> - **Quadrature tables:** their sources and licences are recorded in `tetraloma-refcell/tables/sources.json`.
> - **DefElement verification:** tetraloma is registered there after M12.

**Section 22, Conventions** (ADR-0071):

- After the ndelement bullet, new bullet:

  > tetraloma-element is host-only f64 code without CubeCL; element tables reach kernels as comptime values or as uploads on first use.

- The host↔device copy bullet gains, before "(ADR-0039, …)":

  > ; in tetraloma-function, the upload of an element's coefficients, interpolation matrix, DOF-transformation blocks and tensor factors when a kernel first uses them, the host-staged exchange of a function space's per-block DOF offsets, and the download of interpolation points and upload of values in `interpolate_host`

- New bullet after the kernel bullets:

  > DOF transformations are never fused into evaluation kernels. Complex values are pairs of reals, with complex arithmetic written out in a fixed order; CubeCL's complex types are not used.

- The four-sets bullet adds `tetraloma-function/degree-policy.toml`.

**Section 23, Protected paths,** new rows (ADR-0071):

| Path | What must not change | Check |
| --- | --- | --- |
| `tetraloma-function/degree-policy.toml` | the four generic sets; entries change only with benchmark records in `docs/perf/` | `cargo test -p tetraloma-function --test integration policy::` |
| `tetraloma-element/tests/fixtures/` | the oracle values exported from ndelement, Basix and Symfem | `cargo test -p tetraloma-element --test integration fixtures::` |
| `tetraloma-refcell/tables/` | every stored quadrature table and its source and licence entry | `cargo test -p tetraloma-refcell --test integration quadrature_tables::` |

**Section 24, Open questions.** "None." is replaced by (ADR-0071):

> - The throughput target of the comparison benchmarks (section 8, **Comparison frameworks**): set after M17's first measurement.
> - Whether N1curl and RT are ever needed on prisms and pyramids. If they are, the pyramid family must form an exact sequence with the Lagrange space of Definition 9.

**Section 25, Glossary,** new terms (ADR-0071):

> - **L-vector, E-vector:** a function's coefficients once per DOF ([owned | ghost]), and once per cell and local DOF (cells fastest); MFEM's terms.
> - **Writer bit:** the flag of the one (cell, slot) that writes a shared DOF during interpolation (section 28).
> - **Polyset superdegree:** the degree convention of section 26.
> - **Base transformation, signed-permutation fold:** section 26; section 28.

The user book's glossary gains element, DOF, map type, Piola map and grid function in the same milestones.

### C. New roadmap entries

Appended to `docs/roadmap.md`, headings promoted as in part A. The degree ranges and tolerances are those of the design sections cited; "the tolerances of section 16" include its scaling above degree 4.

#### M11: Lagrange and DG elements

Depends on: none
Design: 6, 22, 26
Acceptance:
- For `Nodal(Equispaced)` Lagrange on intervals, triangles, quadrilaterals, tetrahedra and hexahedra and `Nodal(Gll)` on intervals, quadrilaterals and hexahedra, p = 1–6, values and first derivatives at the fixture points equal ndelement's to 10^−12.
- For `Nodal(Gll)` on triangles, tetrahedra and prisms and `Nodal(Equispaced)` on prisms (p = 1–6) and pyramids (p = 1–3), values and first derivatives at the fixture points equal Basix's after the A.11 conversion to 10^−12.
- Gll points on triangles, tetrahedra and prisms equal Basix's `gll_isaac` points to 10^−14 for p = 1–15.
- On every sub-entity of a triangle, tetrahedron and prism, the Gll points restricted to it equal that sub-entity's Gll points to 10^−15 for p = 1–15.
- The Lebesgue constant of Gll points on triangles and tetrahedra, on a sample grid of 10^4 points, is below that of equispaced points for every p = 3–15.
- For every Lagrange element and variant of section 26 with p ≤ 10 (`Nodal(Equispaced)`: p ≤ 6), applying its interpolation to its basis gives the identity to 10^−12.
- For `Legendre` on every cell, p = 0–10, the reference mass matrix under a `Gauss` rule of degree 2p is the identity to 10^−12.
- Every base transformation of every Lagrange element with p ≤ 10 is a permutation without negation.
- For p ≤ 6, the base transformations of Lagrange elements on intervals, triangles, quadrilaterals, tetrahedra and hexahedra equal ndelement's.
- On two cells sharing an entity, for every relative orientation and every pair of cell types that can share it, a Lagrange function with random coefficients is continuous across the entity to 10^−12 for p ≤ 6.
- On quadrilaterals and hexahedra, the product of the tensor factors reproduces the dense construction's values and first derivatives to 10^−12 for p ≤ 8.
- Degrees outside the table of section 26, and `Nodal(Gll)` on pyramids, return `ElementError::Unsupported`.
- `scripts/check-workspace-deps.sh` fails if tetraloma-element depends on cubecl.

Out of scope: N1curl and RT (M12); device kernels; element construction on the device.
Runs: none

#### M12: N1curl and RT elements

Depends on: M11
Design: 26
Acceptance:
- For N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra, p = 1–8, the number of DOFs on each sub-entity equals Symfem's.
- For the same elements and degrees, the span of the basis functions associated with each sub-entity closure equals ndelement's (Basix's where ndelement lacks the element), by a rank test at 100 random points with tolerance 10^−10.
- For the same elements with p ≤ 10, applying the interpolation to the basis gives the identity to 10^−12.
- For the same elements with p ≤ 10, interpolation reproduces every member of an orthonormal basis of the space to 10^−11.
- Every base transformation of N1curl and RT on quadrilaterals and hexahedra with p ≤ 10 is classified as a signed permutation.
- On two cells sharing an entity, for every relative orientation, N1curl functions with random coefficients have tangential components continuous across the entity to 10^−12 for p ≤ 6.
- On two cells sharing an entity, for every relative orientation, RT functions with random coefficients have normal components continuous across the entity to 10^−12 for p ≤ 6.
- On each of the four cells and for p ≤ 8, the gradients of Lagrange_p lie in N1curl_p with least-squares residual below 10^−12.
- On tetrahedra and hexahedra the curl, and on triangles and quadrilaterals the rot, of N1curl_p lies in RT_p or DG_{p−1} respectively, with residual below 10^−12 for p ≤ 8.
- On each of the four cells and for p ≤ 8, the divergence of RT_p lies in DG_{p−1} with residual below 10^−12.
- On quadrilaterals and hexahedra, the product of the tensor factors reproduces the dense construction's values to 10^−12 for p ≤ 8.

Out of scope: second-kind families; prisms and pyramids; point-evaluation DOFs.
Runs: none

#### M13: Quadrature rules and point sets

Depends on: M11
Design: 10, 27
Acceptance:
- Every rule of every `QuadratureType` integrates the orthonormal set of its degree with relative error at most 10^−14, for `Symmetric` degree 1–30, `Gauss` degree 0–40 and `GaussLobatto` 2–41 points per direction.
- Every point of every `Gauss` and `Symmetric` rule lies in the interior of its cell, and every weight is positive.
- Every `Symmetric` rule on triangles and tetrahedra maps onto itself under every vertex permutation of the cell, with points and weights equal to 10^−15.
- For every degree 4–30, the `Default` rule on triangles and on tetrahedra has fewer points than the `Gauss` rule.
- The mass matrix of the `Nodal(Gll)` Lagrange element of degree n − 1 under the n-point `GaussLobatto` rule is diagonal to 10^−14 for n = 2–17 on intervals, quadrilaterals and hexahedra.
- On meshes with randomly permuted vertex order (tetrahedra, hexahedra, the mixed box and a triangulated sphere), facet points mapped from both cells of every interior facet with `rule_permutation` coincide to 10^−12 in f64 and 10^−5 in f32, on 1–4 ranks.
- `PointSet::tabulate_element` equals the host tabulation of every element of M11 and M12 with p ≤ 6 to 10^−12 in f64 and 10^−5 in f32.
- `permute_vertices` agrees with `physical_points` of a cell whose vertices are permuted, to 10^−12, for every cell type and geometry degree 1–3.
- Every table in `tetraloma-refcell/tables/` has a source and licence entry in `tables/sources.json`.

Out of scope: singular quadrature; adaptive rules; symmetric rules on prisms and pyramids.
Runs: none

#### M14: Function spaces and dofmaps

Depends on: M12
Design: 28, 29
Acceptance:
- Global DOF counts equal closed-form counts for every family of section 26, continuous and discontinuous, with block shapes [] and [3], on boxes of every cell type and the mixed box at three refinements, p = 1–4.
- Global DOF numbers form a bijection onto [0, N) on 1–4 ranks under MPICH and Open MPI.
- N is the same on 1, 2, 3 and 4 ranks.
- Every ghost DOF's global number equals its owner's on 1–4 ranks.
- For every pair of cells sharing an entity, the global DOFs of that entity agree in number and order, after the folded permutations and signs.
- `FunctionSpace::new` returns `EntitiesNotCreated(d)` for a mesh that lacks a dimension d the space needs.
- After `create_entities_for`, `FunctionSpace::new` succeeds and no entity dimension outside `dof_dimensions` exists.
- A discontinuous space is created on a mesh without created entities.
- Every owned DOF has exactly one writer slot on its owning rank, on 1–4 ranks.
- Gather followed by accumulation satisfies ⟨Gu, e⟩ = ⟨u, Gᵀe⟩ to 10^−12 in f64 and 10^−5 in f32.
- Accumulation is bitwise identical with 1 and 7 chunks on the CPU runtime.
- The geometry space's dofmaps and IndexMap are the geometry blocks' and the node IndexMap.
- The halo exchange of a complex function equals the exchanges of its real and imaginary parts bitwise.
- A `Legendre` Lagrange element, or `map = Some(L2Piola)`, in a continuous space returns `SpaceMismatch`.

Out of scope: evaluation (M15); DOF reordering; product spaces; constraints.
Runs: none

#### M15: Evaluation of grid functions

Depends on: M13, M14
Design: 8, 29
Acceptance:
- For every family, cell and variant of section 26 with p ≤ 6, on affine meshes and on curved meshes of geometry degree 2 and 3, `evaluate` with `Op::Value` matches a host reference to the tolerances of section 16.
- Under the same conditions, `evaluate` with the family's derivative (`Grad`, `Curl` or `Div`) matches a host reference to the tolerances of section 16.
- On triangles and quadrilaterals in 3D and intervals in 2D and 3D, `evaluate` with `Value` and the family's derivative matches a host reference to the tolerances of section 16 for p ≤ 6.
- On meshes with randomly permuted vertex order, at matched facet points, Lagrange values evaluated from both cells agree to 10^−12 in f64 on 1–4 ranks for p ≤ 6.
- Under the same conditions, N1curl tangential components agree to 10^−12 in f64, including tetrahedra with dense transformations at p = 2–5.
- Under the same conditions, RT normal components agree to 10^−12 in f64, and on surface meshes the in-plane conormal components agree.
- The geometry view evaluated at a rule's points equals `physical_points` minus `origin()` to 10^−12 in f64 for geometry degree 1–4.
- `evaluate` and `integrate_basis` satisfy ⟨Bu, v⟩ = ⟨u, Bᵀv⟩ to 10^−12 in f64 for every family with p ≤ 6.
- The sum over all DOFs of `integrate_basis` of the constant 1 with `Op::Value` and Lagrange_p equals `measure` to 10^−12 in f64 for p = 1–4.
- `evaluate_at` with `CellPoints` at a rule's points equals `evaluate` with the rule bitwise on the CPU runtime.
- A complex function evaluates to its real and imaginary parts evaluated separately, bitwise on the CPU runtime.
- Evaluation is bitwise identical with 1 and 7 chunks and at vector sizes 1 and widest on the CPU runtime, for every strategy of section 29.
- The strategies loops, shared and cooperative agree with each other to the tolerances of section 16 for p = 1–8 on every cell type except pyramids (p = 1–3), and with direct for p = 1–4 (pyramids p = 1–3).
- A test checks that `tetraloma-function/degree-policy.toml` holds the four sets for FunctionGather, DofTransform, FunctionEval, FunctionEvalComplex, FunctionAccumulate and Interpolate, each following the generic decision rule on the committed records of p = 1, 4 and 8.

Out of scope: sum factorisation (M17); interpolation (M16).
Runs: the benchmark platforms of section 8 (local Mac CPU runtime and Metal, Locust Grace and H100, one Kathleen socket) for the candidate protocol at p = 1, 4 and 8.

#### M16: Interpolation and output

Depends on: M15
Design: 13, 31, 32
Acceptance:
- Interpolation from a `DeviceExpression` reproduces every polynomial of the space on affine meshes to 10^−10 in f64 for every family, cell and variant with p ≤ 6.
- The L2 interpolation error of a smooth field converges at a rate of at least p + 1 − 0.2 for Lagrange and DG of degree p = 1–4, over four refinements on meshes of geometry degree 2, in f64.
- The L2 interpolation error of a smooth field converges at a rate of at least p − 0.2 for N1curl and RT of degree p = 1–4, over four refinements on meshes of geometry degree 2, in f64.
- For u with random coefficients in Lagrange_p, p = 1–5, interpolating ∇u into N1curl_p reproduces ∇u at quadrature points to 10^−10 on affine and curved meshes.
- For u with random coefficients in N1curl_p, p = 1–5, interpolating its curl into RT_p on tetrahedra and hexahedra, and its rot into L2-Piola DG_{p−1} on triangles and quadrilaterals, reproduces it at quadrature points to 10^−10 on affine and curved meshes.
- For u with random coefficients in RT_p, p = 1–5, interpolating div u into L2-Piola DG_{p−1} reproduces it at quadrature points to 10^−10 on affine and curved meshes.
- On the boundaries of a box and a ball, the trace of an interpolated polynomial Lagrange field equals the interpolation of its trace to 10^−10 for p = 1–4.
- On the same boundaries, the tangential trace of an interpolated polynomial N1curl field into surface N1curl equals the interpolation of its tangential trace to 10^−10 for p = 1–4.
- On the same boundaries, the n× trace of an interpolated polynomial N1curl field into surface RT equals the interpolation of its rotated trace to 10^−10 for p = 1–4.
- Lifting a boundary function into the parent space and taking its trace returns the boundary function to 10^−12.
- Interpolating a DG function into continuous Lagrange gives bitwise identical coefficients in two runs and with 1 and 7 chunks on the CPU runtime.
- After interpolation, every ghost value equals its owner's on 1–4 ranks.
- `interpolate_host` and the expression path give the same coefficients to 10^−12 in f64 and 10^−5 in f32 on 1–4 ranks.
- Functions written by `VtkWriter::function` and read back by VTK's Python reader equal the device values at the output points to 10^−12 in f64 and 10^−5 in f32, for Lagrange, DG, N1curl, RT and complex functions in VTU, PVTU and VTKHDF on 1–4 ranks.
- On the same Gmsh mesh, L2 norms of interpolants of the same expressions agree with DOLFINx's to 10^−10 for Lagrange, N1curl and RT with p = 1–3.

Out of scope: non-matching interpolation; point location; Gmsh output; checkpointing; traces on edge submeshes.
Runs: none

#### M17: Sum-factorised evaluation and comparison benchmarks

Depends on: M15
Design: 8, 30
Acceptance:
- The sumfact strategy agrees with the loops strategy to the tolerances of section 16 for every family on quadrilaterals and hexahedra, every applicable `Op` and p = 1–16, in f32 and f64.
- The sumfact strategy's `evaluate` and `integrate_basis` satisfy the adjoint identity to 10^−12 in f64 for p = 1–16.
- For p = 1–16 on every target of the standing matrix, every sumfact plan stays within the device's units per cube and shared-memory limit, as reported by `LaunchPlan`.
- The sumfact strategy is bitwise identical with 1 and 7 chunks on the CPU runtime for p = 1–16.
- A test checks that `tetraloma-function/degree-policy.toml` follows the generic decision rule for FunctionEval and FunctionEvalComplex on quadrilaterals and hexahedra on the committed records of p = 1, 4, 8 and 16.
- `docs/perf/high-order.md` records, for FunctionEval on hexahedra at p = 1, 4, 8 and 16, the time per point of each strategy on every benchmark platform.
- `docs/perf/function-comparison.md` records BP1 and BP3 throughput at p = 4 and 8 for tetraloma, MFEM or libCEED on Locust's H100, and deal.II on one Kathleen socket, with versions and commands.

Out of scope: sum factorisation on simplices, prisms and pyramids; tensor-core instructions; matrix-free operators beyond `integrate_basis`.
Runs: the benchmark platforms of section 8 for p = 1, 4, 8 and 16; MFEM or libCEED on Locust; deal.II on one Kathleen socket.

## Decision

> Accept the ADR with Option 3, 2026-11-22, said to the agent in a session.

The answer names option 3, the recommendation, and does not change the numbers proposed under Recommendation, so they apply as written in parts A–C.

After the apply commit, the agent listed four passages that the ADR did not name and that disagreed with the applied text. Asked whether to fix them in this pull request or in a follow-up ADR, the maintainer answered:

> Please add as third commit, 2026-11-22, said to the agent in a session.

The third commit corrects them, each marked (ADR-0071): section 2's introduction (three topics out of scope); "nine" crates in sections 14 ("Checks"), 18 (CI matrix, `docs`) and 22 (User documentation); GLL node families in section 10 (Tabulation) and section 20 (A.5); and section 18's tetraloma-device row.

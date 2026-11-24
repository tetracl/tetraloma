# Finite elements

A finite element turns a function space on a reference cell into a concrete set of basis
functions and a rule for attaching their coefficients to vertices, edges, faces and cells of a
mesh. The crate `tetraloma-element` defines these reference elements on the host in `f64`,
independently of any compute device. Add `tetraloma-element` alongside `tetraloma` in
`Cargo.toml` (the `tetraloma` facade does not re-export it), then look up an element once
through [`Element::get`](../api/tetraloma_element/struct.Element.html) and read its metadata,
degree-of-freedom layout and tables from the cached handle.

## Elements and DOFs

In Ciarlet's definition, a finite element is a triple $(\hat K, \mathcal V, \mathcal L)$:

- a reference cell $\hat K$, one of the seven cell types in
  [Reference cell definitions](../elements/reference-cells.md);
- a finite-dimensional space $\mathcal V$ of scalar or vector functions on $\hat K$, whose
  dimension $n = \dim \mathcal V$ is returned by `Element::dim`;
- a dual basis $\mathcal L = \{l_0, \dots, l_{n-1}\}$ of linear functionals—the **degrees of
  freedom** (**DOFs**)—that determine a unique basis $\{\phi_0, \dots, \phi_{n-1}\}$ of
  $\mathcal V$ through $l_i(\phi_j) = \delta_{ij}$.

Every DOF is associated with the interior of one sub-entity of $\hat K$: a vertex (dimension $0$),
an edge (dimension $1$), a face (dimension $2$) or the cell interior (dimension $t$, where $t$ is
the cell's topological dimension). Two neighbouring cells that share a vertex, edge or face share
the DOFs attached to that sub-entity, which enforces the family's inter-element continuity (full
continuity for Lagrange, tangential continuity for N1curl, and normal continuity for
Raviart–Thomas).

[`Element`](../api/tetraloma_element/struct.Element.html) exposes this association through two
methods, indexed by dimension `d` (from vertices at `d = 0`, through edges and faces in the
numbering of [Sub-entities](../concepts/reference-cells.md#sub-entities), up to the cell itself at
`d = t`, `e = 0`) and local index `e`:

- `entity_dofs(d, e)` returns the local DOF indices belonging strictly to the interior of
  sub-entity `(d, e)`, excluding its boundary. Across all `(d, e)` with $0 \le d \le t$, these
  slices partition `0..dim()`.
- `entity_closure_dofs(d, e)` returns all local DOF indices on the closure of `(d, e)`: its
  vertices, edges, faces and interior, collected in ascending dimension and sub-entity order. For
  a cell itself (`d = t, e = 0`), the closure holds every DOF of the element.

A reference function $\hat u(\xi)$ on $\hat K$ maps to a physical cell $X(\hat K)$ according
to the element's [`MapType`](../api/tetraloma_element/enum.MapType.html) (`Element::map_type`),
using the Jacobian $J$, pseudo-inverse $K$ and determinant $\det J$ of
[Geometric factors](../concepts/geometric-factors.md) (with the pseudo-determinant when $t < g$,
where $g$ is the geometric dimension):

| `MapType` | Reference values | Push-forward $u(x)$ | Pull-back $\hat u(\xi)$ | Families |
| --- | --- | --- | --- | --- |
| `Identity` | scalar ($1$) | $\hat u(\xi)$ | $u(x)$ | Lagrange, DG |
| `CovariantPiola` | vector ($t$) | $K^{\mathsf T} \hat u(\xi)$ | $J^{\mathsf T} u(x)$ | N1curl, RTCE, NCE |
| `ContravariantPiola` | vector ($t$) | $J \hat u(\xi) / \det J$ | $(\det J)\, K u(x)$ | RT, RTCF, NCF |
| `L2Piola` | scalar ($1$) | $\hat u(\xi) / \det J$ | $(\det J)\, u(x)$ | DG in exact sequences |

`Element::reference_value_size` gives the number of components of $\hat u$: `1` for scalar
Lagrange elements and $t$ for vector elements.

The example below looks up a cubic Lagrange triangle, inspects its map type and sub-entity DOFs,
and verifies that repeated lookups share one cached allocation:

```rust
use std::sync::Arc;
use tetraloma::{CellType, NodeFamily};
use tetraloma_element::{Element, ElementKey, Family, MapType, Variant};

let key = ElementKey {
    family: Family::Lagrange,
    cell: CellType::Triangle,
    degree: 3,
    variant: Variant::Nodal(NodeFamily::Equispaced),
};
let el = Element::get(key)?;
assert!(Arc::ptr_eq(&el, &Element::get(key)?));

assert_eq!(el.dim(), 10);
assert_eq!(el.reference_value_size(), 1);
assert_eq!(el.map_type(), MapType::Identity);
assert_eq!(el.embedded_superdegree(), 3);

// Three vertices, two interior DOFs on each edge, and one interior DOF.
assert_eq!(el.entity_dofs(0, 1), &[1]);
assert_eq!(el.entity_dofs(1, 2), &[7, 8]);
assert_eq!(el.entity_dofs(2, 0), &[9]);
// Closure of edge 2 (vertices 1 and 2): its two vertices followed by its two interior DOFs.
assert_eq!(el.entity_closure_dofs(1, 2), &[1, 2, 7, 8]);
# Ok::<(), tetraloma_element::ElementError>(())
```

## Families and degrees

Elements are addressed by [`ElementKey`](../api/tetraloma_element/struct.ElementKey.html), which
combines a [`Family`](../api/tetraloma_element/enum.Family.html), a `CellType`, a polynomial
`degree` and a [`Variant`](../api/tetraloma_element/enum.Variant.html).

The `degree` field uses the **polyset superdegree** convention, matching Basix:

- degree $1$ is the lowest-order continuous Lagrange, N1curl and Raviart–Thomas element;
- degree $0$ is the lowest-order discontinuous (piecewise-constant) Lagrange element.

With this convention, elements of the same degree on neighbouring cells of different types (for
example a tetrahedron and a prism sharing a triangular face, or a hexahedron and a pyramid
sharing a quadrilateral face) have matching traces on their shared sub-entities.
`Element::embedded_superdegree` returns the smallest Lagrange degree whose space contains the
element's space; for Lagrange elements it equals `degree`.

The table below lists the families, cells and degree ranges defined in tetraloma. Any key outside
the table—as well as `Family::N1curl` and `Family::RaviartThomas`, which are declared in `Family`
but not yet constructed—returns
[`ElementError::Unsupported`](../api/tetraloma_element/enum.ElementError.html):

| Cell | `Lagrange` ($H^1$ / DG) | `N1curl` ($H(\mathrm{curl})$) | `RaviartThomas` ($H(\mathrm{div})$) | Degrees |
| --- | --- | --- | --- | --- |
| Interval | $\mathcal P_p$ | – | – | $1\text{–}20$ (DG $0\text{–}20$) |
| Triangle | $\mathcal P_p$ | $\mathrm{N1curl}_p$ | $\mathrm{RT}_p$ | $1\text{–}15$ (DG $0\text{–}15$) |
| Quadrilateral | $\mathcal Q_p$ | $\mathrm{RTCE}_p$ | $\mathrm{RTCF}_p$ | $1\text{–}20$ (DG $0\text{–}20$) |
| Tetrahedron | $\mathcal P_p$ | $\mathrm{N1curl}_p$ | $\mathrm{RT}_p$ | $1\text{–}15$ (DG $0\text{–}15$) |
| Hexahedron | $\mathcal Q_p$ | $\mathrm{NCE}_p$ | $\mathrm{NCF}_p$ | $1\text{–}20$ (DG $0\text{–}20$) |
| Prism | DefElement prism space | – | – | $1\text{–}15$ (DG $0\text{–}15$) |
| Pyramid | DefElement Definition 9 | – | – | $1\text{–}8$ (DG $0\text{–}8$) |

The example below checks that degrees beyond a cell's range and unsupported cell–variant pairs
return `ElementError::Unsupported`:

```rust
use tetraloma::{CellType, NodeFamily};
use tetraloma_element::{Element, ElementError, ElementKey, Family, Variant};

let too_high = ElementKey {
    family: Family::Lagrange,
    cell: CellType::Pyramid,
    degree: 9,
    variant: Variant::Nodal(NodeFamily::Equispaced),
};
assert!(matches!(
    Element::get(too_high),
    Err(ElementError::Unsupported { cell: CellType::Pyramid, degree: 9, .. })
));

let gll_pyramid = ElementKey {
    family: Family::Lagrange,
    cell: CellType::Pyramid,
    degree: 2,
    variant: Variant::Nodal(NodeFamily::Gll),
};
assert!(matches!(
    Element::get(gll_pyramid),
    Err(ElementError::Unsupported {
        cell: CellType::Pyramid,
        variant: Variant::Nodal(NodeFamily::Gll),
        ..
    })
));
```

## Variants

Each [`Variant`](../api/tetraloma_element/enum.Variant.html) chooses how the DOFs of the element
are defined, which fixes both the basis functions and where the DOFs sit on the cell:

- **`Variant::Nodal(NodeFamily::Equispaced)`** evaluates functions at the equispaced Lagrange
  nodes of [Lagrange nodes](../elements/lagrange.md). It is offered on all seven cell types, at
  degrees $0$ up to the cell's maximum. For $p \ge 1$, the DOFs follow the sub-entity block order
  (vertices, edges, faces, interior), so the element supports both continuous and discontinuous
  spaces (`discontinuous_only()` is `false`).
- **`Variant::Nodal(NodeFamily::Gll)`** evaluates functions at the Gauss–Lobatto–Legendre points
  of [Node families](../elements/lagrange.md#node-families): tensor GLL grids on intervals,
  quadrilaterals and hexahedra, Isaac's recursive GLL points on triangles and tetrahedra, and
  their product on prisms. It shares the same sub-entity DOF layout as equispaced nodal elements
  while remaining well conditioned at high degree. It is not offered on pyramids at any degree.
- **`Variant::Legendre`** defines DOFs as integral moments against an $L^2$-orthonormal basis on
  the reference cell. For `Family::Lagrange`, every DOF lies in the cell interior (`entity_dofs`
  is empty for every sub-entity of dimension below $t$), so the basis is $L^2$-orthonormal with
  an identity reference mass matrix and `discontinuous_only()` is `true`.

At **degree $0$**, every variant offered on a cell has dimension $1$ and places its single DOF in
the cell interior, so `discontinuous_only()` is `true` for nodal variants as well as `Legendre`.
A nodal element of degree $0$ evaluates the function at the cell's midpoint (the mean of its
vertices) and has the constant basis function $\phi_0 = 1$; `Variant::Legendre` of degree $0$ has
the normalised constant basis function $\phi_0 = 1 / \sqrt{|\hat K|}$.

```rust
use tetraloma::{CellType, NodeFamily};
use tetraloma_element::{Element, ElementKey, Family, Variant};

// Nodal degree 0: one interior DOF, discontinuous only.
let p0 = Element::get(ElementKey {
    family: Family::Lagrange,
    cell: CellType::Tetrahedron,
    degree: 0,
    variant: Variant::Nodal(NodeFamily::Gll),
})?;
assert_eq!(p0.dim(), 1);
assert!(p0.discontinuous_only());
assert!(p0.entity_dofs(0, 0).is_empty());
assert_eq!(p0.entity_dofs(3, 0), &[0]);

// Legendre degree 2: same dimension as nodal degree 2, but every DOF is interior.
let leg = Element::get(ElementKey {
    family: Family::Lagrange,
    cell: CellType::Tetrahedron,
    degree: 2,
    variant: Variant::Legendre,
})?;
assert_eq!(leg.dim(), 10);
assert!(leg.discontinuous_only());
assert!(leg.entity_closure_dofs(2, 0).is_empty());
assert_eq!(leg.entity_dofs(3, 0).len(), 10);
# Ok::<(), tetraloma_element::ElementError>(())
```

## Tabulation

Each Lagrange element expresses its scalar basis functions $\phi_0, \dots, \phi_{n-1}$ in the
cell's $L^2$-orthonormal polynomial set $\{\psi_k\}$
([Orthonormal polynomial sets](../concepts/quadrature.md#orthonormal-polynomial-sets)):
$\phi_i(\xi) = \sum_{k=0}^{n_{\mathrm{poly}}-1} C_{i, k}\, \psi_k(\xi)$. `Element::coefficients`
returns this row-major `[dim][polyset_dim * reference_value_size]` table (`[dim][polyset_dim]`
for scalar Lagrange, where `reference_value_size() == 1`), obtained once per key by inverting the
Ciarlet dual matrix $D_{ij} = l_i(\psi_j)$ in `f64` with `DenseLu` (or assembled lazily from 1D
factors on quadrilaterals and hexahedra; see [Tensor factors](#tensor-factors)).

`Element::tabulate(points, nderivs)` evaluates the basis at flat node-major reference points
(`tdim` coordinates per point) and returns an
[`ElementTabulation`](../api/tetraloma_element/struct.ElementTabulation.html) in
`[derivative][point][basis][component]` order: block `0` holds values, and for `nderivs == 1`
block `1 + d` holds $\partial \phi_i / \partial \xi_d$. Orders above `1` return
[`ElementError::RefCell`](../api/tetraloma_element/enum.ElementError.html) wrapping
`UnsupportedDerivativeOrder`, and a mis-sized point slice returns `PointsShape`.

```rust
use tetraloma::{CellType, NodeFamily};
use tetraloma_element::{Element, ElementKey, Family, Variant};

let el = Element::get(ElementKey {
    family: Family::Lagrange,
    cell: CellType::Triangle,
    degree: 2,
    variant: Variant::Nodal(NodeFamily::Gll),
})?;
let tab = el.tabulate(&[0.2, 0.3, 0.5, 0.1], 1)?;
assert_eq!((tab.nderivs, tab.num_points, tab.dim, tab.value_size), (1, 2, 6, 1));
for q in 0..tab.num_points {
    let sum: f64 = tab.values[q * tab.dim..(q + 1) * tab.dim].iter().sum();
    assert!((sum - 1.0).abs() < 1e-13);
    for d in 0..2 {
        let base = ((1 + d) * tab.num_points + q) * tab.dim;
        let dsum: f64 = tab.values[base..base + tab.dim].iter().sum();
        assert!(dsum.abs() < 1e-13);
    }
}
# Ok::<(), tetraloma_element::ElementError>(())
```

## Interpolation

Every element exposes its dual functionals $\mathcal L = \{l_0, \dots, l_{n-1}\}$ through
`Element::interpolation_points` (flat `[m][tdim]`) and `Element::interpolation_matrix`
(row-major `[dim][m * reference_value_size]`). For a scalar Lagrange function $\hat u$, the DOF
values are $l_i(\hat u) = \sum_{q=0}^{m-1} M_{i, q}\,\hat u(\xi_q)$.

For nodal Lagrange variants at $p \ge 1$, the points are the Lagrange nodes and $M = I$; at
$p = 0$, the single point is the mean of the cell's vertices and $M = [1]$. For
`Variant::Legendre`, the points come from a degree-$2p$ Gauss rule and $M_{i,q} = w_q \psi_i(\xi_q)$.
Because $l_i(\phi_j) = \delta_{ij}$, applying $M$ to the basis tabulated at
`interpolation_points()` recovers the identity to $10^{-12}$ (up to $p = 10$ for `Gll` and
`Legendre`, and $p = 6$ for `Equispaced`).

```rust
use tetraloma::CellType;
use tetraloma_element::{Element, ElementKey, Family, Variant};

let el = Element::get(ElementKey {
    family: Family::Lagrange,
    cell: CellType::Triangle,
    degree: 2,
    variant: Variant::Legendre,
})?;
let (pts, mat) = (el.interpolation_points(), el.interpolation_matrix());
let m = pts.len() / 2;
let samples: Vec<f64> = pts.chunks_exact(2).map(|p| 2.0 * p[0] + 3.0 * p[1] - 1.0).collect();
let dofs: Vec<f64> = (0..el.dim())
    .map(|i| mat[i * m..(i + 1) * m].iter().zip(&samples).map(|(a, s)| a * s).sum())
    .collect();
let tab = el.tabulate(&[0.25, 0.4], 0)?;
let approx: f64 = dofs.iter().zip(&tab.values).map(|(c, b)| c * b).sum();
assert!((approx - (2.0 * 0.25 + 3.0 * 0.4 - 1.0)).abs() < 1e-13);
# Ok::<(), tetraloma_element::ElementError>(())
```

## Tensor factors

On quadrilaterals and hexahedra, every Lagrange element is a tensor product of one-dimensional
elements along the reference coordinate axes. [`Element::tensor_factors`](../api/tetraloma_element/struct.Element.html)
returns `Some(&TensorFactors)` for these two cells and `None` for all others.

Because the element is constructed from its 1D factors, only small 1D dual matrices are ever
inverted: `Element::tabulate` combines 1D tabulations directly, and the dense `dim × dim` tables
returned by `Element::coefficients` and `Element::interpolation_matrix` are assembled from the
factors on first use rather than during `Element::get`. High-degree hexahedra (up to $p = 20$,
where `dim = 9261` and a dense table would occupy 686 MB) therefore build and tabulate quickly
without allocating dense tables unless you explicitly request them.

[`TensorFactors`](../api/tetraloma_element/struct.TensorFactors.html) exposes two public fields
and a 1D tabulator:

- `factors[component][direction]` gives the `(Factor1d, degree)` pair for each reference component
  and coordinate direction, using [`Factor1d`](../api/tetraloma_element/enum.Factor1d.html). A
  factor of degree $d$ has $d + 1$ basis functions: `NodalH1(family)` gives the 1D Lagrange basis
  in interval DOF order (endpoints first, or the constant $1$ at degree $0$), `LegendreL2` gives
  the 1D $L^2$-orthonormal Legendre basis spanning $\mathcal P_d$ on $[0, 1]$, and `MomentH1` is
  declared for tensor-product $H(\mathrm{curl})$ and $H(\mathrm{div})$ elements.
- `lexicographic[component][tensor_index]` maps each x-fastest multi-index
  $a_0 + n_0 a_1$ (on quadrilaterals) or $a_0 + n_0 (a_1 + n_1 a_2)$ (on hexahedra) to the
  element's local DOF index.
- `tabulate_1d(component, direction, points, nderivs)` returns the 1D basis values (`nderivs == 0`)
  or values and first derivatives (`nderivs == 1`) at scalar points in $[0, 1]$, laid out in
  `[derivative][point][basis]` order (panicking for `nderivs > 1`, out-of-range indices,
  `Factor1d::MomentH1`, or an unsupported interval degree).

```rust
use tetraloma::{CellType, NodeFamily};
use tetraloma_element::{Element, ElementKey, Factor1d, Family, Variant};

let el = Element::get(ElementKey {
    family: Family::Lagrange,
    cell: CellType::Quadrilateral,
    degree: 3,
    variant: Variant::Nodal(NodeFamily::Gll),
})?;
let tf = el.tensor_factors().expect("quadrilateral has tensor factors");
assert_eq!(tf.factors[0], vec![(Factor1d::NodalH1(NodeFamily::Gll), 3); 2]);

let (x, y) = (0.27, 0.71);
let tx = tf.tabulate_1d(0, 0, &[x], 1);
let ty = tf.tabulate_1d(0, 1, &[y], 1);
let n = 4; // degree + 1
let tab = el.tabulate(&[x, y], 1)?;

let (mut sum_v, mut sum_dx, mut sum_dy) = (0.0, 0.0, 0.0);
for a1 in 0..n {
    for a0 in 0..n {
        let dof = tf.lexicographic[0][a0 + n * a1];
        let (v, dx, dy) = (tx[a0] * ty[a1], tx[n + a0] * ty[a1], tx[a0] * ty[n + a1]);
        assert!((v - tab.values[dof]).abs() < 1e-14);
        assert!((dx - tab.values[el.dim() + dof]).abs() < 1e-14);
        assert!((dy - tab.values[2 * el.dim() + dof]).abs() < 1e-14);
        sum_v += v;
        sum_dx += dx;
        sum_dy += dy;
    }
}
// Partition of unity: values sum to 1 and partial derivatives sum to 0.
assert!((sum_v - 1.0).abs() < 1e-13);
assert!(sum_dx.abs() < 1e-13 && sum_dy.abs() < 1e-13);
# Ok::<(), tetraloma_element::ElementError>(())
```

# Finite elements

A finite element turns a function space on a reference cell into a concrete set of basis
functions and a rule for attaching their coefficients to vertices, edges, faces and cells of a
mesh. The crate `tetraloma-element` defines these reference elements on the host in `f64`,
independently of any compute device. Add `tetraloma-element` alongside `tetraloma` in
`Cargo.toml` (the `tetraloma` facade does not re-export it yet), then look up an element once
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
the DOFs attached to that sub-entity, which enforces continuity of the global function across cell
boundaries.

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

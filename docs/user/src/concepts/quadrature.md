# Quadrature

A quadrature rule replaces an integral by a weighted sum of point values. On a physical cell
$T = X(\hat T)$, the change of variables and a rule with points $\xi_q$ and weights $w_q$ on
the reference cell give

$$
\int_T f(x)\, dx = \int_{\hat T} f(X(\xi)) \det J(\xi)\, d\xi
\approx \sum_q f(X(\xi_q)) \det J(\xi_q)\, w_q ,
$$

with the pseudo-determinant in place of $\det J$ when the cell is a curve or surface (see
[Geometric factors](geometric-factors.md#the-determinant)). Every geometric factor in tetraloma
is evaluated at the points of such a rule. This chapter describes the rules, the rules on
facets, and the tabulated basis functions that turn node coordinates into values at the points.

## Quadrature rules

A rule of degree $k$ integrates every polynomial of degree at most $k$ on the reference cell
exactly. tetraloma's rules are Gauss rules:

- on intervals, quadrilaterals and hexahedra, tensor products of the Gauss–Legendre rule; they
  are exact for $\xi_0^a \xi_1^b \xi_2^c$ with every exponent at most $k$;
- on triangles, tetrahedra and pyramids, collapsed Gauss–Jacobi rules, which map a tensor rule
  on the unit square or cube onto the cell; they are exact for total degree $k$ (on pyramids,
  for the rational functions of degree $k$ of the pyramid's Lagrange space);
- on prisms, a triangle rule times an interval rule, exact for $\xi_0^a \xi_1^b \xi_2^c$ with
  $a + b \le k$ and $c \le k$.

Every rule uses $m = \lfloor k/2 \rfloor + 1$ points per direction, so it has $m^t$ points. All
points lie strictly inside the cell, all weights are positive, and the weights add up to the
volume of the reference cell. The points and weights are computed on the host in f64 and stay
there; only the tabulated basis functions go to the device (see [Tabulation](#tabulation)).

A mesh may mix cell types, so a rule in tetraloma
([`QuadRule`](../api/tetraloma/struct.QuadRule.html)) holds one point set per cell type.
`QuadRule::gauss(client, cells, degree)` makes one for the cell types you list, and
`QuadRule::for_grid(mesh, degree)` one for the cell types of a mesh. `get(cell)` returns the
point set of one cell type, with its `points()` (flat, $t$ coordinates per point) and
`weights()`.

The example checks the triangle rule of degree 4: it has $3^2 = 9$ points, its weights add up
to the area $1/2$, and it integrates $\xi_0^2 \xi_1^2$ exactly, to
$\int_{\hat T} \xi_0^2 \xi_1^2 \, d\xi = 2!\,2!/6! = 1/180$.

```rust
use tetraloma::prelude::*;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let client = cubecl::Device::cpu()?.client();

let rule = QuadRule::<f64, D2>::gauss(&client, &[Cell2::Triangle, Cell2::Quadrilateral], 4)?;
let tri = rule.get(Cell2::Triangle)?;
assert_eq!(tri.len(), 9);
assert!((tri.weights().iter().sum::<f64>() - 0.5).abs() < 1e-15);

let integral: f64 = tri
    .points()
    .chunks_exact(2)
    .zip(tri.weights())
    .map(|(p, w)| w * p[0] * p[0] * p[1] * p[1])
    .sum();
assert!((integral - 1.0 / 180.0).abs() < 1e-15);

// The quadrilateral's rule is the tensor product of two 3-point Gauss rules.
let quad = rule.get(Cell2::Quadrilateral)?;
assert_eq!(quad.len(), 9);
assert!((quad.weights().iter().sum::<f64>() - 1.0).abs() < 1e-15);
# Ok(()) }
```

To integrate a function over a mesh with this rule, you evaluate it at the physical points,
multiply by $\det J$ and the weights, and sum. `measure` does exactly this for $f = 1$. The
tutorial [Quadrature on a mesh](../tutorials/quadrature.md#integrating-over-a-mesh) integrates
other functions.

## Facet rules

Boundary integrals and fluxes between cells need points on the facets of a cell. A facet rule
([`FacetQuadRule`](../api/tetraloma/struct.FacetQuadRule.html)) takes a Gauss rule on each
facet's own reference cell and maps its points into the coordinates of the cell, facet by facet.
For each cell type it holds a [`FacetPointSet`](../api/tetraloma/struct.FacetPointSet.html)
with:

- `points()`: all facet points as one point set, in cell coordinates, facet-major (the points
  of facet 0 first, then those of facet 1, and so on), with the weights of the facet rules;
- `facet_offsets()`: where each facet's points start, one entry per facet plus one;
- `reference_normal(q)`: the outward unit normal of the reference cell on the facet of point
  $q$.

The facets of a prism or pyramid have different types, so their point counts differ. A field
on facet points therefore has $\sum_f n_f$ points per cell, where $n_f$ is the point count of
facet $f$.

The weights are those of the facet's reference cell. They add up to 1 on an edge and $1/2$ on
a triangular face, whatever the facet's size in the reference cell. To integrate over a
physical facet, you multiply them by the facet's own volume factor, the pseudo-determinant of
the facet's map, which is not part of the rule.

```rust
use tetraloma::prelude::*;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let client = cubecl::Device::cpu()?.client();

// Two Gauss points on each of the triangle's three edges.
let rule = FacetQuadRule::<f64, D2>::gauss(&client, &[Cell2::Triangle], 3)?;
let fps = rule.get(Cell2::Triangle)?;
assert_eq!(fps.facet_offsets(), &[0, 2, 4, 6]);

// Edge 2 runs from (1, 0) to (0, 1): its points satisfy ξ0 + ξ1 = 1, its weights add up to 1
// (the length of the reference interval), and its outward normal is (1, 1)/√2.
let pts = fps.points().points();
let w = fps.points().weights();
let r = 0.5f64.sqrt();
for q in 4..6 {
    assert!((pts[2 * q] + pts[2 * q + 1] - 1.0).abs() < 1e-15);
    assert!((fps.reference_normal(q)[0] - r).abs() < 1e-15);
    assert!((fps.reference_normal(q)[1] - r).abs() < 1e-15);
}
assert!((w[4] + w[5] - 1.0).abs() < 1e-15);
# Ok(()) }
```

## Tabulation

To evaluate the map $X(\xi) = \sum_k x_k \phi_k(\xi)$ and its Jacobian at the points of a
rule, a kernel needs the basis functions and their derivatives at those points. These numbers
depend only on the coordinate element and the rule, not on the cell, so tetraloma computes them
once, in a table:

- values $\phi_k(\xi_q)$, stored as $[q][k]$;
- first derivatives $\partial \phi_k / \partial \xi_d\, (\xi_q)$, stored as $[q][k][d]$.

A kernel then evaluates the physical points and Jacobians of all cells as small matrix
products, $X(\xi_q) = \sum_k x_k\, \phi_k(\xi_q)$ and
$J_{\gamma d}(\xi_q) = \sum_k x_{k,\gamma}\, \partial_d \phi_k(\xi_q)$, reading the table from
fast memory.

tetraloma tabulates on the host in f64, narrows the table to the mesh's scalar type and uploads
it the first time a point set is used with a coordinate element. The point set keeps it, so
later calls with the same rule reuse it. `tabulate(element)` and
`tabulate_derivatives(element)` on a point set return these device tables.

The example tabulates the linear triangle at the centroid. There every basis function has the
value $1/3$, and the derivatives are those of $\phi_0 = 1 - \xi_0 - \xi_1$, $\phi_1 = \xi_0$ and
$\phi_2 = \xi_1$. The host reads are there for inspection only.

```rust
use tetraloma::prelude::*;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let client = cubecl::Device::cpu()?.client();

let p1 = CoordinateElement::<D2> {
    cell: Cell2::Triangle,
    degree: 1,
    family: NodeFamily::Equispaced,
};
// Degree 1: one point, the centroid (1/3, 1/3).
let rule = QuadRule::<f64, D2>::gauss(&client, &[Cell2::Triangle], 1)?;
let pts = rule.get(Cell2::Triangle)?;

let values = pts.tabulate(&p1)?.to_host()?;
assert_eq!(values.len(), 3);
assert!(values.iter().all(|v| (v - 1.0 / 3.0).abs() < 1e-14));

// [q][k][d]: (∂0 φk, ∂1 φk) for k = 0, 1, 2.
let derivs = pts.tabulate_derivatives(&p1)?.to_host()?;
let want = [-1.0, -1.0, 1.0, 0.0, 0.0, 1.0];
for (d, w) in derivs.iter().zip(want) {
    assert!((d - w).abs() < 1e-14);
}
# Ok(()) }
```

### Orthonormal polynomial sets

Every reference cell also has a host-side $L^2$-orthonormal polynomial set spanning its
degree-$p$ space, exposed in `tetraloma_refcell` as
[`polyset`](../api/tetraloma_refcell/polyset/index.html):

- on intervals, normalised Legendre polynomials on $[0, 1]$;
- on quadrilaterals and hexahedra, tensor products of those Legendre polynomials;
- on triangles and tetrahedra, Dubiner's collapsed-coordinate Jacobi bases;
- on prisms, the triangle Dubiner basis times the interval Legendre basis;
- on pyramids, orthonormal rational functions spanning the pyramid's Lagrange space.

`polyset::dim(cell, degree)` returns the number of basis functions (1 at degree 0 and on the
point cell), and `polyset::tabulate(cell, degree, points, nderivs)` tabulates values
(`nderivs == 0`) or values and first derivatives (`nderivs == 1`) in
`[derivative][basis][point]` order: block 0 holds the values $\varphi_k(\xi_q)$ and block
$1 + d$ holds $\partial \varphi_k / \partial \xi_d(\xi_q)$. Passing `nderivs > 1` or a point
array whose length is not a multiple of the cell's topological dimension panics.

[`LagrangeElement`](../api/tetraloma_refcell/struct.LagrangeElement.html) and downstream finite
elements expand their bases in these orthonormal sets and solve the small dense Vandermonde or
dual system on the host in `f64` with
[`DenseLu`](../api/tetraloma_refcell/struct.DenseLu.html), which factors a square matrix with
partial pivoting (`DenseLu::factor`) and solves against any number of right-hand sides in place
(`DenseLu::solve`).

```rust
use tetraloma::CellType;
use tetraloma_refcell::{DenseLu, gauss_rule, lagrange_points, polyset, NodeFamily};
# fn main() -> Result<(), Box<dyn std::error::Error>> {
// Tabulate the degree-2 orthonormal set on the reference triangle at its Gauss points.
let rule = gauss_rule(CellType::Triangle, 4);
let nb = polyset::dim(CellType::Triangle, 2);
let npts = rule.weights.len();
let table = polyset::tabulate(CellType::Triangle, 2, &rule.points, 0);
assert_eq!((nb, table.len()), (6, 6 * npts));

// Integrate φ_0^2 over the reference triangle: L^2-orthonormal means 1.
let norm_sq: f64 = (0..npts)
    .map(|q| rule.weights[q] * table[q] * table[q])
    .sum();
assert!((norm_sq - 1.0).abs() < 1e-14);

// Build the P2 Vandermonde matrix at the equispaced nodes and invert it with DenseLu.
let nodes = lagrange_points(CellType::Triangle, 2, NodeFamily::Equispaced)?;
let at_nodes = polyset::tabulate(CellType::Triangle, 2, &nodes, 0);
let mut vandermonde = vec![0.0; nb * nb];
for i in 0..nb {
    for k in 0..nb {
        vandermonde[i * nb + k] = at_nodes[k * nb + i];
    }
}
let mut coeffs = vec![0.0; nb * nb];
for i in 0..nb {
    coeffs[i * nb + i] = 1.0;
}
DenseLu::factor(vandermonde, nb)?.solve(&mut coeffs, nb);
assert_eq!(coeffs.len(), 36);
# Ok(()) }
```

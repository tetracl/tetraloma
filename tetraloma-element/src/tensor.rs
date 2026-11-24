// Design: section 26 ("Finite elements", "Tensor factors"), Listing E1; ADR-0071, ADR-0072.
//! Tensor-product factorisation of finite elements on quadrilaterals and hexahedra.

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    NodeFamily, gauss_rule, polyset, sub_entities, sub_entity_types, tdim, vertices,
};

use crate::{Element, ElementError, ElementKey, ElementTabulation, Family, Variant, ciarlet};

// Design: section 26 ("Finite elements", "Tensor factors"), Listing E1; ADR-0072.
/// One-dimensional basis factor along a single coordinate direction of a quadrilateral or
/// hexahedron.
///
/// Each reference component of a tensor-cell element is the tensor product of one `Factor1d` per
/// coordinate direction; see [Tensor factors].
///
/// [Tensor factors]: https://tetracl.github.io/tetraloma/fem/elements.html#tensor-factors
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Factor1d {
    /// One-dimensional Lagrange basis at a `tetraloma-refcell` point family (`Equispaced` or
    /// `Gll`), in interval DOF order (endpoints `0` and `1` first, then interior nodes). At
    /// degree 0 this is the single constant function `1`.
    NodalH1(NodeFamily),
    /// Endpoint evaluations plus interior Legendre moments up to degree `p − 2`, used in the
    /// continuous directions of tensor-product H(curl) and H(div) elements.
    ///
    /// No currently supported element constructs this variant.
    MomentH1,
    /// Orthonormal Legendre basis on `[0, 1]` with integral-moment DOFs.
    LegendreL2,
}

// Design: section 26 ("Finite elements", "Tensor factors"), Listing E1.
/// Tensor-product decomposition of a quadrilateral or hexahedron element into one-dimensional
/// factors.
///
/// Returned by [`Element::tensor_factors`]. For each reference component `c`, `factors[c]` lists
/// the `(Factor1d, degree)` pair along each coordinate direction `0..tdim`, and
/// `lexicographic[c]` maps each x-fastest tensor multi-index
/// `a₀ + n₀ a₁ (+ n₀ n₁ a₂)` (where `n_d` is the basis count of direction `d`) to the element's
/// local DOF index. See [Tensor factors].
///
/// # Example
///
/// ```
/// use tetraloma_dims::CellType;
/// use tetraloma_element::{Element, ElementKey, Factor1d, Family, Variant};
/// use tetraloma_refcell::NodeFamily;
///
/// let el = Element::get(ElementKey {
///     family: Family::Lagrange,
///     cell: CellType::Quadrilateral,
///     degree: 2,
///     variant: Variant::Nodal(NodeFamily::Gll),
/// })?;
/// let tf = el.tensor_factors().expect("quadrilateral has tensor factors");
/// assert_eq!(
///     tf.factors,
///     vec![vec![
///         (Factor1d::NodalH1(NodeFamily::Gll), 2),
///         (Factor1d::NodalH1(NodeFamily::Gll), 2),
///     ]]
/// );
/// assert_eq!(tf.lexicographic[0].len(), el.dim());
/// # Ok::<(), tetraloma_element::ElementError>(())
/// ```
///
/// [Tensor factors]: https://tetracl.github.io/tetraloma/fem/elements.html#tensor-factors
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorFactors {
    /// Per-component, per-direction `(kind, degree)` descriptors: `[component][direction]`.
    pub factors: Vec<Vec<(Factor1d, u32)>>,
    /// Per-component map from x-fastest tensor index to local element DOF:
    /// `[component][tensor_index] -> dof`.
    pub lexicographic: Vec<Vec<usize>>,
}

impl TensorFactors {
    // Design: section 26 ("Finite elements", "Tensor factors", "Derivative orders"), Listing E1;
    // ADR-0072.
    /// Tabulates one 1D factor at scalar points in `[0, 1]`, returning entries in
    /// `[derivative][point][basis]` order.
    ///
    /// Block `0` holds values; when `nderivs == 1`, block `1` holds first derivatives. For an
    /// empty `points` slice the returned vector is empty. See [Tensor factors].
    ///
    /// # Panics
    ///
    /// - If `nderivs > 1`.
    /// - If `component` or `direction` is out of range for `self.factors`.
    /// - If the factor kind or degree is unsupported (for example [`Factor1d::MomentH1`] or a
    ///   degree outside the supported interval range).
    /// - If the output table size overflows `usize`.
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange,
    ///     cell: CellType::Quadrilateral,
    ///     degree: 1,
    ///     variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// let tf = el.tensor_factors().unwrap();
    /// let t = tf.tabulate_1d(0, 0, &[0.25], 1);
    /// // Values: φ₀(0.25) = 0.75, φ₁(0.25) = 0.25; derivatives: −1, +1.
    /// assert_eq!(t.len(), 4);
    /// assert!((t[0] - 0.75).abs() < 1e-14 && (t[1] - 0.25).abs() < 1e-14);
    /// assert!((t[2] + 1.0).abs() < 1e-14 && (t[3] - 1.0).abs() < 1e-14);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Tensor factors]: https://tetracl.github.io/tetraloma/fem/elements.html#tensor-factors
    pub fn tabulate_1d(
        &self,
        component: usize,
        direction: usize,
        points: &[f64],
        nderivs: u32,
    ) -> Vec<f64> {
        assert!(
            nderivs <= 1,
            "TensorFactors::tabulate_1d: derivative order {nderivs} is not supported (0 or 1)"
        );
        let dirs = self.factors.get(component).unwrap_or_else(|| {
            panic!(
                "TensorFactors::tabulate_1d: component {component} out of range (num components is {})",
                self.factors.len()
            )
        });
        let &(kind, degree) = dirs.get(direction).unwrap_or_else(|| {
            panic!(
                "TensorFactors::tabulate_1d: direction {direction} out of range for component {component} (num directions is {})",
                dirs.len()
            )
        });
        let npts = points.len();
        let num_derivs: usize = if nderivs == 0 { 1 } else { 2 };

        match kind {
            Factor1d::NodalH1(family) => {
                let el = Element::get(ElementKey {
                    family: Family::Lagrange,
                    cell: CellType::Interval,
                    degree,
                    variant: Variant::Nodal(family),
                })
                .unwrap_or_else(|e| {
                    panic!(
                        "TensorFactors::tabulate_1d: unsupported 1D factor ({kind:?}, {degree}): {e}"
                    )
                });
                let _ = num_derivs
                    .checked_mul(npts)
                    .and_then(|v| v.checked_mul(el.dim()))
                    .expect("TensorFactors::tabulate_1d: table size overflows usize");
                el.tabulate(points, nderivs)
                    .expect("validated 1D tabulate cannot fail")
                    .values
            }
            Factor1d::LegendreL2 => {
                let nb = polyset::dim(CellType::Interval, degree);
                let len = num_derivs
                    .checked_mul(npts)
                    .and_then(|v| v.checked_mul(nb))
                    .expect("TensorFactors::tabulate_1d: table size overflows usize");
                if npts == 0 {
                    return Vec::new();
                }
                let poly = polyset::tabulate(CellType::Interval, degree, points, nderivs);
                let mut out = vec![0.0; len];
                for d in 0..num_derivs {
                    let src = &poly[d * nb * npts..(d + 1) * nb * npts];
                    let dst = &mut out[d * npts * nb..(d + 1) * npts * nb];
                    for b in 0..nb {
                        for q in 0..npts {
                            dst[q * nb + b] = src[b * npts + q];
                        }
                    }
                }
                out
            }
            Factor1d::MomentH1 => {
                unimplemented!(
                    "TensorFactors::tabulate_1d: Factor1d::MomentH1 is not yet supported"
                )
            }
        }
    }
}

/// Whether `cell` is a tensor-product cell (quadrilateral or hexahedron).
pub(crate) fn is_tensor_cell(cell: CellType) -> bool {
    matches!(cell, CellType::Quadrilateral | CellType::Hexahedron)
}

/// Builds the x-fastest tensor-index-to-DOF map for a nodal Lagrange element on a quadrilateral
/// or hexahedron, with each 1D direction indexed in interval DOF order (endpoints `0` and `1`,
/// followed by interior nodes `2..=degree`).
fn nodal_lexicographic(cell: CellType, degree: u32) -> Vec<usize> {
    debug_assert!(is_tensor_cell(cell));
    if degree == 0 {
        return vec![0];
    }
    let (td, p) = (tdim(cell), degree as usize);
    let (n, verts) = (p + 1, vertices(cell));
    let dim = n.pow(td as u32);
    let mut lex_to_dof = vec![usize::MAX; dim];
    let mut dof = 0usize;

    for d in 0..=td {
        for (entity, &ety) in sub_entities(cell, d).iter().zip(sub_entity_types(cell, d)) {
            let v0 = verts[entity[0]];
            let mut base = [0usize; 3];
            for c in 0..td {
                base[c] = usize::from(v0[c] > 0.5);
            }
            let axis_of = |v_idx: usize| -> usize {
                (0..td)
                    .find(|&c| (verts[v_idx][c] - v0[c]).abs() > 0.5)
                    .expect("tensor-cell sub-entity edge is axis-aligned")
            };
            let free: &[usize] = match ety {
                CellType::Point => &[],
                CellType::Interval => &[axis_of(entity[1])],
                CellType::Quadrilateral => &[axis_of(entity[1]), axis_of(entity[2])],
                CellType::Hexahedron => {
                    &[axis_of(entity[1]), axis_of(entity[2]), axis_of(entity[4])]
                }
                _ => unreachable!("unexpected sub-entity {ety:?} on {cell:?}"),
            };
            let m = p - 1;
            for flat in 0..m.pow(free.len() as u32) {
                let mut idx = base;
                let mut rem = flat;
                for &ax in free {
                    idx[ax] = 2 + (rem % m);
                    rem /= m;
                }
                let lex = (0..td).rev().fold(0usize, |acc, c| acc * n + idx[c]);
                debug_assert_eq!(lex_to_dof[lex], usize::MAX);
                lex_to_dof[lex] = dof;
                dof += 1;
            }
        }
    }
    debug_assert_eq!(dof, dim);
    lex_to_dof
}

/// Builds the `TensorFactors` of a validated Lagrange element on a quadrilateral or hexahedron,
/// warming the 1D interval element in the cache for nodal variants.
pub(crate) fn build_lagrange_tensor_factors(
    key: ElementKey,
    dim: usize,
) -> Result<TensorFactors, ElementError> {
    debug_assert!(key.family == Family::Lagrange && is_tensor_cell(key.cell));
    let td = tdim(key.cell);
    let (factor1d, lexicographic) = match key.variant {
        Variant::Nodal(family) => {
            let _ = Element::get(ElementKey {
                family: Family::Lagrange,
                cell: CellType::Interval,
                degree: key.degree,
                variant: Variant::Nodal(family),
            })?;
            (
                Factor1d::NodalH1(family),
                nodal_lexicographic(key.cell, key.degree),
            )
        }
        Variant::Legendre => (Factor1d::LegendreL2, (0..dim).collect()),
    };
    Ok(TensorFactors {
        factors: vec![vec![(factor1d, key.degree); td]],
        lexicographic: vec![lexicographic],
    })
}

/// Writes the outer product of 2 or 3 length-`n` 1D factor slices into `out` via `lex_map`.
fn write_tensor_product(out: &mut [f64], lex_map: &[usize], f: &[&[f64]]) {
    let n = f[0].len();
    match f {
        [f0, f1] => {
            for (a1, &y) in f1.iter().enumerate() {
                for (a0, &x) in f0.iter().enumerate() {
                    out[lex_map[a0 + n * a1]] = x * y;
                }
            }
        }
        [f0, f1, f2] => {
            for (a2, &z) in f2.iter().enumerate() {
                for (a1, &y) in f1.iter().enumerate() {
                    let yz = y * z;
                    let base = n * (a1 + n * a2);
                    for (a0, &x) in f0.iter().enumerate() {
                        out[lex_map[base + a0]] = x * yz;
                    }
                }
            }
        }
        _ => unreachable!("write_tensor_product only handles 2 or 3 directions"),
    }
}

/// Tabulates a scalar Lagrange element on a quadrilateral or hexahedron from its 1D tensor
/// factors into `[derivative][point][basis][component=1]` order.
pub(crate) fn tabulate_lagrange(
    cell: CellType,
    dim: usize,
    tf: &TensorFactors,
    points: &[f64],
    nderivs: u32,
) -> Result<ElementTabulation, ElementError> {
    let num_points = ciarlet::validate_tabulate_args(cell, points, nderivs)?;
    let td = tdim(cell);
    let num_derivs = if nderivs == 0 { 1 } else { 1 + td };
    if num_points == 0 {
        return Ok(ElementTabulation {
            nderivs,
            num_points: 0,
            dim,
            value_size: 1,
            values: Vec::new(),
        });
    }

    let t1d: Vec<Vec<f64>> = (0..td)
        .map(|d| {
            let coords: Vec<f64> = (0..num_points).map(|q| points[q * td + d]).collect();
            tf.tabulate_1d(0, d, &coords, nderivs)
        })
        .collect();
    let n = tf.factors[0][0].1 as usize + 1;
    let lex_map = &tf.lexicographic[0];
    let block = num_points * dim;
    let mut values = vec![0.0; num_derivs * block];

    for q in 0..num_points {
        let vals: Vec<&[f64]> = (0..td).map(|d| &t1d[d][q * n..(q + 1) * n]).collect();
        write_tensor_product(&mut values[q * dim..(q + 1) * dim], lex_map, &vals);
        if nderivs == 1 {
            for deriv_dir in 0..td {
                let mut f = vals.clone();
                f[deriv_dir] = &t1d[deriv_dir][(num_points + q) * n..(num_points + q + 1) * n];
                let base = (1 + deriv_dir) * block + q * dim;
                write_tensor_product(&mut values[base..base + dim], lex_map, &f);
            }
        }
    }

    Ok(ElementTabulation {
        nderivs,
        num_points,
        dim,
        value_size: 1,
        values,
    })
}

fn identity_matrix(dim: usize) -> Vec<f64> {
    let mut id = vec![0.0; dim * dim];
    for i in 0..dim {
        id[i * dim + i] = 1.0;
    }
    id
}

/// Kronecker product `outer ⊗ inner` (row-major square matrices of sizes `n_out` and `n_in`),
/// optionally permuting output rows via `row_map[lex] -> dof`.
fn kronecker(
    outer: &[f64],
    n_out: usize,
    inner: &[f64],
    n_in: usize,
    row_map: Option<&[usize]>,
) -> Vec<f64> {
    let dim = n_out * n_in;
    let mut out = vec![0.0; dim * dim];
    for i_o in 0..n_out {
        let ro = &outer[i_o * n_out..(i_o + 1) * n_out];
        for i_i in 0..n_in {
            let ri = &inner[i_i * n_in..(i_i + 1) * n_in];
            let lex = i_o * n_in + i_i;
            let dof = row_map.map_or(lex, |m| m[lex]);
            let dst = &mut out[dof * dim..(dof + 1) * dim];
            for (j_o, &vo) in ro.iter().enumerate() {
                let col = j_o * n_in;
                for (j_i, &vi) in ri.iter().enumerate() {
                    dst[col + j_i] = vo * vi;
                }
            }
        }
    }
    out
}

fn kronecker_power(a: &[f64], n: usize, td: usize, row_map: Option<&[usize]>) -> Vec<f64> {
    match td {
        2 => kronecker(a, n, a, n, row_map),
        3 => {
            let sq = kronecker(a, n, a, n, None);
            kronecker(a, n, &sq, n * n, row_map)
        }
        _ => unreachable!("kronecker_power only handles td = 2 or 3"),
    }
}

/// Assembles the lazy `coefficients` table `[dim][dim]` for a tensor-cell Lagrange element from
/// its 1D factors.
pub(crate) fn assemble_lagrange_coefficients(key: ElementKey, tf: &TensorFactors) -> Vec<f64> {
    let td = tdim(key.cell);
    let n = key.degree as usize + 1;
    match key.variant {
        Variant::Legendre => identity_matrix(n.pow(td as u32)),
        Variant::Nodal(family) => {
            let el_1d = Element::get(ElementKey {
                family: Family::Lagrange,
                cell: CellType::Interval,
                degree: key.degree,
                variant: Variant::Nodal(family),
            })
            .expect("1D interval element already validated");
            kronecker_power(el_1d.coefficients(), n, td, Some(&tf.lexicographic[0]))
        }
    }
}

/// Assembles the lazy `interpolation_matrix` table `[dim][dim]` for a tensor-cell Lagrange
/// element from its 1D factors.
pub(crate) fn assemble_lagrange_interpolation_matrix(key: ElementKey, dim: usize) -> Vec<f64> {
    match key.variant {
        Variant::Nodal(_) => identity_matrix(dim),
        Variant::Legendre => {
            let td = tdim(key.cell);
            let rule = gauss_rule(CellType::Interval, 2 * key.degree);
            let n = key.degree as usize + 1;
            let phi = polyset::tabulate(CellType::Interval, key.degree, &rule.points, 0);
            let m1: Vec<f64> = phi
                .chunks_exact(n)
                .flat_map(|row| row.iter().zip(&rule.weights).map(|(&p, &w)| w * p))
                .collect();
            kronecker_power(&m1, n, td, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use tetraloma_dims::CellType;
    use tetraloma_refcell::{NodeFamily, RefCellError, lagrange_points, tdim};

    use super::*;
    use crate::{Element, ElementKey, ElementTabulation, Family, Generator, Variant};

    const TOL: f64 = 1e-12;

    fn lagrange(cell: CellType, degree: u32, variant: Variant) -> ElementKey {
        ElementKey {
            family: Family::Lagrange,
            cell,
            degree,
            variant,
        }
    }

    /// Reference dense construction for tensor cells (section 26: "used there only by tests, for
    /// p <= 8").
    fn dense_tabulate(key: ElementKey, points: &[f64], nderivs: u32) -> ElementTabulation {
        let dim = crate::layout::build_lagrange_layout(key).dim;
        let data = crate::ciarlet::build_lagrange_dense(key, dim).unwrap();
        crate::ciarlet::tabulate_from_coefficients(
            key.cell,
            key.degree,
            dim,
            &data.coefficients,
            points,
            nderivs,
        )
        .unwrap()
    }

    fn tensor_grid(coords: &[f64], td: usize) -> Vec<f64> {
        let m = coords.len();
        let mut pts = Vec::with_capacity(m.pow(td as u32) * td);
        for idx in 0..m.pow(td as u32) {
            let mut rem = idx;
            for _ in 0..td {
                pts.push(coords[rem % m]);
                rem /= m;
            }
        }
        pts
    }

    /// M11 acceptance A11: on quadrilaterals and hexahedra, the product of the tensor factors
    /// reproduces the dense construction's values and first derivatives to 1e-12 for p <= 8.
    #[test]
    fn factors_reproduce_dense_construction() {
        let coords_1d = [0.0, 0.19, 0.63, 1.0];
        let variants = [
            Variant::Nodal(NodeFamily::Equispaced),
            Variant::Nodal(NodeFamily::Gll),
            Variant::Legendre,
        ];

        for cell in [CellType::Quadrilateral, CellType::Hexahedron] {
            let td = tdim(cell);
            let points = tensor_grid(&coords_1d, td);
            let npts = points.len() / td;
            let per_dir: Vec<Vec<f64>> = (0..td)
                .map(|d| (0..npts).map(|q| points[q * td + d]).collect())
                .collect();

            for variant in variants {
                for degree in 0..=8 {
                    let key = lagrange(cell, degree, variant);
                    let el = Element::get(key).unwrap();
                    let tf = el.tensor_factors().expect("tensor cell has tensor factors");
                    assert_eq!((tf.factors.len(), tf.factors[0].len()), (1, td));

                    let t1d: Vec<Vec<f64>> = (0..td)
                        .map(|d| tf.tabulate_1d(0, d, &per_dir[d], 1))
                        .collect();
                    let n_dir: Vec<usize> = t1d.iter().map(|t| t.len() / (2 * npts)).collect();
                    let dim = el.dim();
                    assert_eq!(n_dir.iter().product::<usize>(), dim);
                    assert_eq!(tf.lexicographic[0].len(), dim);

                    let dense = dense_tabulate(key, &points, 1);
                    assert_eq!(
                        (dense.nderivs, dense.num_points, dense.dim, dense.value_size),
                        (1, npts, dim, 1)
                    );

                    let mut got = vec![0.0; (1 + td) * npts * dim];
                    for q in 0..npts {
                        for lex in 0..dim {
                            let dof = tf.lexicographic[0][lex];
                            let mut rem = lex;
                            let mut a = [0usize; 3];
                            for d in 0..td {
                                a[d] = rem % n_dir[d];
                                rem /= n_dir[d];
                            }
                            let val_at = |d: usize, deriv: usize| {
                                t1d[d][(deriv * npts + q) * n_dir[d] + a[d]]
                            };
                            got[q * dim + dof] = (0..td).map(|d| val_at(d, 0)).product();
                            for deriv_dir in 0..td {
                                let prod: f64 = (0..td)
                                    .map(|d| val_at(d, usize::from(d == deriv_dir)))
                                    .product();
                                got[((1 + deriv_dir) * npts + q) * dim + dof] = prod;
                            }
                        }
                    }

                    let block = npts * dim;
                    for d in 0..=td {
                        let want = &dense.values[d * block..(d + 1) * block];
                        let actual = &got[d * block..(d + 1) * block];
                        let scale = want.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                        for (idx, (&g, &w)) in actual.iter().zip(want).enumerate() {
                            let err = (g - w).abs() / scale;
                            assert!(
                                err <= TOL,
                                "{key:?} deriv {d} entry {idx}: {g} vs {w} (rel err {err:e})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn non_tensor_cells_and_lexicographic_node_map() {
        for cell in [
            CellType::Interval,
            CellType::Triangle,
            CellType::Tetrahedron,
            CellType::Prism,
            CellType::Pyramid,
        ] {
            let el =
                Element::get(lagrange(cell, 2, Variant::Nodal(NodeFamily::Equispaced))).unwrap();
            assert!(el.tensor_factors().is_none(), "{cell:?}");
        }
        for cell in [CellType::Quadrilateral, CellType::Hexahedron] {
            let td = tdim(cell);
            for family in [NodeFamily::Equispaced, NodeFamily::Gll] {
                for degree in [1u32, 2, 3, 5, 10, 20] {
                    let line = lagrange_points(CellType::Interval, degree, family).unwrap();
                    let cell_pts = lagrange_points(cell, degree, family).unwrap();
                    let n = degree as usize + 1;
                    let lex = nodal_lexicographic(cell, degree);
                    assert_eq!(lex.len(), n.pow(td as u32));
                    for (idx, &dof) in lex.iter().enumerate() {
                        let mut rem = idx;
                        for d in 0..td {
                            assert!((cell_pts[dof * td + d] - line[rem % n]).abs() < 1e-15);
                            rem /= n;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn high_degree_hexahedron_stays_lazy_and_matches_dense_tables() {
        for variant in [
            Variant::Nodal(NodeFamily::Gll),
            Variant::Nodal(NodeFamily::Equispaced),
            Variant::Legendre,
        ] {
            let el = Element::get(lagrange(CellType::Hexahedron, 20, variant)).unwrap();
            assert_eq!(el.dim(), 9261);
            let tab = el.tabulate(&[0.23, 0.41, 0.79], 1).unwrap();
            assert_eq!(
                (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
                (1, 1, 9261, 1)
            );
            if matches!(variant, Variant::Nodal(_)) {
                assert!((tab.values[..9261].iter().sum::<f64>() - 1.0).abs() < 1e-10);
                assert!(
                    el.base_transformation(CellType::Interval, Generator::Reversal)
                        .is_some()
                );
                assert!(
                    el.base_transformation(CellType::Quadrilateral, Generator::Rotation)
                        .is_some()
                );
            }
            assert_eq!(el.lazy_tables_initialised(), (false, false));

            for cell in [CellType::Quadrilateral, CellType::Hexahedron] {
                for degree in [0u32, 1, 2, 4] {
                    let key = lagrange(cell, degree, variant);
                    let e = Element::get(key).unwrap();
                    let dense = crate::ciarlet::build_lagrange_dense(key, e.dim()).unwrap();
                    let c_scale = dense
                        .coefficients
                        .iter()
                        .fold(1.0_f64, |m, v| m.max(v.abs()));
                    assert!(
                        e.coefficients()
                            .iter()
                            .zip(&dense.coefficients)
                            .all(|(&g, &w)| (g - w).abs() / c_scale <= TOL)
                    );
                    assert_eq!(e.interpolation_points(), dense.interpolation_points);
                    let m_scale = dense
                        .interpolation_matrix
                        .iter()
                        .fold(1.0_f64, |m, v| m.max(v.abs()));
                    assert!(
                        e.interpolation_matrix()
                            .iter()
                            .zip(&dense.interpolation_matrix)
                            .all(|(&g, &w)| (g - w).abs() / m_scale <= TOL)
                    );
                    assert_eq!(e.lazy_tables_initialised(), (true, true));
                }
            }
        }
    }

    #[test]
    fn degree_zero_empty_points_and_tensor_tabulate_errors() {
        let q0 = Element::get(lagrange(
            CellType::Quadrilateral,
            0,
            Variant::Nodal(NodeFamily::Gll),
        ))
        .unwrap();
        let tf = q0.tensor_factors().unwrap();
        assert_eq!(
            tf.factors,
            vec![vec![(Factor1d::NodalH1(NodeFamily::Gll), 0); 2]]
        );
        assert_eq!(tf.lexicographic, vec![vec![0]]);
        assert!(tf.tabulate_1d(0, 0, &[], 0).is_empty() && tf.tabulate_1d(0, 0, &[], 1).is_empty());
        assert_eq!(
            tf.tabulate_1d(0, 0, &[0.1, 0.9], 1),
            vec![1.0, 1.0, 0.0, 0.0]
        );

        let h2 = Element::get(lagrange(CellType::Hexahedron, 2, Variant::Legendre)).unwrap();
        let ltf = h2.tensor_factors().unwrap();
        assert!(
            ltf.tabulate_1d(0, 0, &[], 0).is_empty() && ltf.tabulate_1d(0, 0, &[], 1).is_empty()
        );
        assert_eq!(
            h2.tabulate(&[0.1], 2),
            Err(ElementError::RefCell(
                RefCellError::UnsupportedDerivativeOrder(2)
            ))
        );
        assert_eq!(
            h2.tabulate(&[0.1, 0.2], 0),
            Err(ElementError::RefCell(RefCellError::PointsShape {
                len: 2,
                tdim: 3
            }))
        );
        for nd in [0, 1] {
            let empty = h2.tabulate(&[], nd).unwrap();
            assert_eq!(
                (empty.nderivs, empty.num_points, empty.dim, empty.value_size),
                (nd, 0, 27, 1)
            );
            assert!(empty.values.is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "MomentH1")]
    fn tabulate_1d_panics_on_moment_h1() {
        let tf = TensorFactors {
            factors: vec![vec![(Factor1d::MomentH1, 1)]],
            lexicographic: vec![vec![0, 1]],
        };
        let _ = tf.tabulate_1d(0, 0, &[0.5], 0);
    }

    #[test]
    #[should_panic(expected = "unsupported 1D factor")]
    fn tabulate_1d_panics_on_unsupported_nodal_degree() {
        let tf = TensorFactors {
            factors: vec![vec![(Factor1d::NodalH1(NodeFamily::Gll), 21)]],
            lexicographic: vec![Vec::new()],
        };
        let _ = tf.tabulate_1d(0, 0, &[0.5], 0);
    }

    #[test]
    #[should_panic(expected = "derivative order 2 is not supported")]
    fn tabulate_1d_panics_above_order_1() {
        let el = Element::get(lagrange(
            CellType::Quadrilateral,
            2,
            Variant::Nodal(NodeFamily::Gll),
        ))
        .unwrap();
        let _ = el.tensor_factors().unwrap().tabulate_1d(0, 0, &[0.5], 2);
    }

    #[test]
    #[should_panic(expected = "component 1 out of range")]
    fn tabulate_1d_panics_on_bad_component() {
        let el = Element::get(lagrange(CellType::Quadrilateral, 1, Variant::Legendre)).unwrap();
        let _ = el.tensor_factors().unwrap().tabulate_1d(1, 0, &[0.5], 0);
    }

    #[test]
    #[should_panic(expected = "direction 2 out of range")]
    fn tabulate_1d_panics_on_bad_direction() {
        let el = Element::get(lagrange(CellType::Quadrilateral, 1, Variant::Legendre)).unwrap();
        let _ = el.tensor_factors().unwrap().tabulate_1d(0, 2, &[0.5], 0);
    }
}

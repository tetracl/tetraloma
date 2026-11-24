// Design: section 26 ("Finite elements"), Listing E1, "Construction", "Derivative orders",
// "Variants"; ADR-0071, ADR-0072.
//! Ciarlet element construction and tabulation over `tetraloma-refcell`'s orthonormal polynomial
//! sets.

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    DenseLu, RefCellError, gauss_rule, lagrange_points, polyset, tdim, vertices,
};

use crate::{ElementError, ElementKey, ElementTabulation, Family, Variant};

/// Interpolation data and orthonormal-basis expansion coefficients from Ciarlet construction.
pub(crate) struct CiarletData {
    /// `[dim][polyset_dim * value_size]`.
    pub(crate) coefficients: Vec<f64>,
    /// `[m][tdim]`.
    pub(crate) interpolation_points: Vec<f64>,
    /// `[dim][m * value_size]`.
    pub(crate) interpolation_matrix: Vec<f64>,
}

/// Transposes a row-major `rows × cols` block into `cols × rows` in `dst`.
fn transpose_block(src: &[f64], rows: usize, cols: usize, dst: &mut [f64]) {
    for r in 0..rows {
        for (c, &v) in src[r * cols..(r + 1) * cols].iter().enumerate() {
            dst[c * rows + r] = v;
        }
    }
}

/// Mean of the reference cell's vertices (the single interpolation point of nodal degree 0).
fn cell_midpoint(cell: CellType) -> Vec<f64> {
    let verts = vertices(cell);
    let inv_n = 1.0 / (verts.len() as f64);
    (0..tdim(cell))
        .map(|d| verts.iter().map(|v| v[d]).sum::<f64>() * inv_n)
        .collect()
}

/// Builds `(interpolation_points, interpolation_matrix)` for a validated Lagrange element key.
fn lagrange_interpolation(
    cell: CellType,
    degree: u32,
    variant: Variant,
    dim: usize,
) -> Result<(Vec<f64>, Vec<f64>), ElementError> {
    match variant {
        Variant::Nodal(family) => {
            let points = if degree == 0 {
                cell_midpoint(cell)
            } else {
                lagrange_points(cell, degree, family)?
            };
            let mut matrix = vec![0.0; dim * dim];
            for i in 0..dim {
                matrix[i * dim + i] = 1.0;
            }
            Ok((points, matrix))
        }
        Variant::Legendre => {
            let rule = gauss_rule(cell, 2 * degree);
            let m = rule.weights.len();
            let phi = polyset::tabulate(cell, degree, &rule.points, 0);
            let mut matrix = vec![0.0; dim * m];
            for (m_row, phi_row) in matrix.chunks_exact_mut(m).zip(phi.chunks_exact(m)) {
                for ((dst, &p), &w) in m_row.iter_mut().zip(phi_row).zip(&rule.weights) {
                    *dst = w * p;
                }
            }
            Ok((rule.points, matrix))
        }
    }
}

/// Constructs Ciarlet basis coefficients in `tetraloma-refcell`'s orthonormal set from
/// interpolation points (`[m][tdim]`) and matrix (`[dim][m * value_size]`).
///
/// Forms `D[i, k * v + c] = l_i(e_c ψ_k)`, solves `D Y = I` with `DenseLu`, and transposes `Y`
/// into `[dim][polyset_dim * value_size]` order so `φ_j = Σ_k C[j, k * v + c] e_c ψ_k`.
pub(crate) fn solve_ciarlet_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
    value_size: usize,
    interpolation_points: &[f64],
    interpolation_matrix: &[f64],
) -> Result<Vec<f64>, ElementError> {
    let m = interpolation_points.len() / tdim(cell);
    let n_poly = polyset::dim(cell, degree);
    debug_assert_eq!(dim, n_poly * value_size);
    debug_assert_eq!(interpolation_matrix.len(), dim * m * value_size);

    let poly = polyset::tabulate(cell, degree, interpolation_points, 0);
    let mut poly_t = vec![0.0; m * n_poly];
    transpose_block(&poly, n_poly, m, &mut poly_t);

    let mut dual = vec![0.0; dim * dim];
    for (d_row, m_row) in dual
        .chunks_exact_mut(dim)
        .zip(interpolation_matrix.chunks_exact(m * value_size))
    {
        for (p_row, m_q) in poly_t
            .chunks_exact(n_poly)
            .zip(m_row.chunks_exact(value_size))
        {
            for (c, &a) in m_q.iter().enumerate() {
                if a == 0.0 {
                    continue;
                }
                if value_size == 1 {
                    for (dv, &pv) in d_row.iter_mut().zip(p_row) {
                        *dv += a * pv;
                    }
                } else {
                    for (k, &pv) in p_row.iter().enumerate() {
                        d_row[k * value_size + c] += a * pv;
                    }
                }
            }
        }
    }

    let lu = DenseLu::factor(dual, dim)?;
    let mut inv = vec![0.0; dim * dim];
    for i in 0..dim {
        inv[i * dim + i] = 1.0;
    }
    lu.solve(&mut inv, dim);

    let mut coefficients = vec![0.0; dim * dim];
    transpose_block(&inv, dim, dim, &mut coefficients);
    Ok(coefficients)
}

/// Runs the dense Ciarlet construction for a validated Lagrange element key.
pub(crate) fn build_lagrange_dense(
    key: ElementKey,
    dim: usize,
) -> Result<CiarletData, ElementError> {
    debug_assert_eq!(key.family, Family::Lagrange);
    let (interpolation_points, interpolation_matrix) =
        lagrange_interpolation(key.cell, key.degree, key.variant, dim)?;
    let coefficients = solve_ciarlet_coefficients(
        key.cell,
        key.degree,
        dim,
        1,
        &interpolation_points,
        &interpolation_matrix,
    )?;
    Ok(CiarletData {
        coefficients,
        interpolation_points,
        interpolation_matrix,
    })
}

/// Tabulates an element from its `polyset` expansion `coefficients` (`[dim][n_poly * value_size]`)
/// into `[derivative][point][basis][component]` order.
pub(crate) fn tabulate_from_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
    value_size: usize,
    coefficients: &[f64],
    points: &[f64],
    nderivs: u32,
) -> Result<ElementTabulation, ElementError> {
    if nderivs > 1 {
        return Err(ElementError::RefCell(
            RefCellError::UnsupportedDerivativeOrder(nderivs),
        ));
    }
    let td = tdim(cell);
    let shape_ok = if td == 0 {
        points.is_empty()
    } else {
        points.len().is_multiple_of(td)
    };
    if !shape_ok {
        return Err(ElementError::RefCell(RefCellError::PointsShape {
            len: points.len(),
            tdim: td,
        }));
    }
    let num_points = points.len().checked_div(td).unwrap_or(1);
    let num_derivs = if nderivs == 0 { 1 } else { 1 + td };
    if num_points == 0 {
        return Ok(ElementTabulation {
            nderivs,
            num_points: 0,
            dim,
            value_size,
            values: Vec::new(),
        });
    }

    let n_poly = polyset::dim(cell, degree);
    let poly = polyset::tabulate(cell, degree, points, nderivs);
    let mut values = vec![0.0; num_derivs * num_points * dim * value_size];
    let mut poly_t = vec![0.0; num_points * n_poly];
    let row_stride = n_poly * value_size;

    for (d, out_block) in values
        .chunks_exact_mut(num_points * dim * value_size)
        .enumerate()
    {
        transpose_block(
            &poly[d * n_poly * num_points..(d + 1) * n_poly * num_points],
            n_poly,
            num_points,
            &mut poly_t,
        );
        for (p_row, out_q) in poly_t
            .chunks_exact(n_poly)
            .zip(out_block.chunks_exact_mut(dim * value_size))
        {
            if value_size == 1 {
                for (dst, c_row) in out_q.iter_mut().zip(coefficients.chunks_exact(n_poly)) {
                    *dst = c_row.iter().zip(p_row).map(|(&c, &p)| c * p).sum();
                }
            } else {
                for (out_ic, c_i) in out_q
                    .chunks_exact_mut(value_size)
                    .zip(coefficients.chunks_exact(row_stride))
                {
                    for (&pv, c_k) in p_row.iter().zip(c_i.chunks_exact(value_size)) {
                        if pv != 0.0 {
                            for (o, &cv) in out_ic.iter_mut().zip(c_k) {
                                *o += cv * pv;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ElementTabulation {
        nderivs,
        num_points,
        dim,
        value_size,
        values,
    })
}

#[cfg(test)]
mod tests {
    use tetraloma_dims::CellType;
    use tetraloma_refcell::{LagrangeElement, NodeFamily, RefCellError, gauss_rule, polyset, tdim};

    use super::*;
    use crate::{Element, ElementError, ElementKey, Family, Variant};

    const ALL_CELLS: [CellType; 7] = [
        CellType::Interval,
        CellType::Triangle,
        CellType::Quadrilateral,
        CellType::Tetrahedron,
        CellType::Hexahedron,
        CellType::Prism,
        CellType::Pyramid,
    ];

    fn cell_volume(cell: CellType) -> f64 {
        match cell {
            CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron => 1.0,
            CellType::Triangle | CellType::Prism => 0.5,
            CellType::Tetrahedron => 1.0 / 6.0,
            CellType::Pyramid => 1.0 / 3.0,
            CellType::Point => unreachable!(),
        }
    }

    fn lagrange(cell: CellType, degree: u32, variant: Variant) -> ElementKey {
        ElementKey {
            family: Family::Lagrange,
            cell,
            degree,
            variant,
        }
    }

    #[test]
    fn degree_zero_nodal_and_legendre_bases() {
        for cell in ALL_CELLS {
            let (td, vol, mid) = (tdim(cell), cell_volume(cell), cell_midpoint(cell));
            let pts = gauss_rule(cell, 3).points;
            let npts = pts.len() / td;
            let nodal: &[Variant] = if cell == CellType::Pyramid {
                &[Variant::Nodal(NodeFamily::Equispaced)]
            } else {
                &[
                    Variant::Nodal(NodeFamily::Equispaced),
                    Variant::Nodal(NodeFamily::Gll),
                ]
            };
            for &variant in nodal {
                let el = Element::get(lagrange(cell, 0, variant)).unwrap();
                assert_eq!(el.interpolation_points(), mid.as_slice());
                assert_eq!(el.interpolation_matrix(), &[1.0]);
                assert!((el.coefficients()[0] - vol.sqrt()).abs() < 1e-14);
                let tab = el.tabulate(&pts, 1).unwrap();
                assert_eq!(
                    (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
                    (1, npts, 1, 1)
                );
                assert!(tab.values[..npts].iter().all(|&v| (v - 1.0).abs() < 1e-14));
                assert!(tab.values[npts..].iter().all(|&v| v.abs() < 1e-14));
            }
            let leg = Element::get(lagrange(cell, 0, Variant::Legendre)).unwrap();
            assert_eq!(leg.interpolation_points().len(), td);
            assert!((leg.interpolation_matrix()[0] - vol.sqrt()).abs() < 1e-14);
            assert!((leg.coefficients()[0] - 1.0).abs() < 1e-14);
            let tab = leg.tabulate(&pts, 1).unwrap();
            let want = 1.0 / vol.sqrt();
            assert!(tab.values[..npts].iter().all(|&v| (v - want).abs() < 1e-14));
            assert!(tab.values[npts..].iter().all(|&v| v.abs() < 1e-14));
        }
    }

    #[test]
    fn nodal_matches_refcell_lagrange_element_and_transposed_coefficients() {
        for cell in ALL_CELLS {
            let td = tdim(cell);
            let fams: &[NodeFamily] = if cell == CellType::Pyramid {
                &[NodeFamily::Equispaced]
            } else {
                &[NodeFamily::Equispaced, NodeFamily::Gll]
            };
            let pts = gauss_rule(cell, 4).points;
            let npts = pts.len() / td;
            for &nf in fams {
                for p in 1..=5 {
                    let ref_el = LagrangeElement::new(cell, p, nf).unwrap();
                    let el = Element::get(lagrange(cell, p, Variant::Nodal(nf))).unwrap();
                    let n = el.dim();
                    assert_eq!(el.interpolation_points(), ref_el.points());
                    let (ce, cr) = (el.coefficients(), ref_el.coefficients());
                    let cs = cr.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                    for j in 0..n {
                        for k in 0..n {
                            assert!((ce[j * n + k] - cr[k * n + j]).abs() < 1e-13 * cs);
                        }
                    }
                    let tab = el.tabulate(&pts, 1).unwrap();
                    let rtab = ref_el.tabulate(&pts, 1).unwrap();
                    for d in 0..=td {
                        let base = d * npts * n;
                        let s = rtab.data[base..base + npts * n]
                            .iter()
                            .fold(1.0_f64, |m, v| m.max(v.abs()));
                        for q in 0..npts {
                            for j in 0..n {
                                assert!(
                                    (tab.values[base + q * n + j] - rtab.get(d, q, j)).abs()
                                        < 1e-12 * s
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn legendre_derivatives_match_central_differences_and_polyset() {
        let h = 1e-6;
        for cell in ALL_CELLS {
            let (td, pt) = (tdim(cell), cell_midpoint(cell));
            for p in 1..=5 {
                let el = Element::get(lagrange(cell, p, Variant::Legendre)).unwrap();
                let (n, tab) = (el.dim(), el.tabulate(&pt, 1).unwrap());
                let poly = polyset::tabulate(cell, p, &pt, 1);
                for d in 0..=td {
                    let s = poly[d * n..(d + 1) * n]
                        .iter()
                        .fold(1.0_f64, |m, v| m.max(v.abs()));
                    for j in 0..n {
                        assert!((tab.values[d * n + j] - poly[d * n + j]).abs() <= 1e-12 * s);
                    }
                }
                for d in 0..td {
                    let (mut xp, mut xm) = (pt.clone(), pt.clone());
                    xp[d] += h;
                    xm[d] -= h;
                    let (tp, tm) = (el.tabulate(&xp, 0).unwrap(), el.tabulate(&xm, 0).unwrap());
                    let s = tab.values[(1 + d) * n..(2 + d) * n]
                        .iter()
                        .fold(1.0_f64, |m, v| m.max(v.abs()));
                    for j in 0..n {
                        let fd = (tp.values[j] - tm.values[j]) / (2.0 * h);
                        assert!(
                            (fd - tab.values[(1 + d) * n + j]).abs() < 1e-6 * s * ((p * p) as f64)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn tabulate_errors_empty_points_and_singular_dual() {
        let el = Element::get(lagrange(
            CellType::Tetrahedron,
            2,
            Variant::Nodal(NodeFamily::Gll),
        ))
        .unwrap();
        let bad_ord = Err(ElementError::RefCell(
            RefCellError::UnsupportedDerivativeOrder(2),
        ));
        assert_eq!(el.tabulate(&[0.25, 0.25, 0.25], 2), bad_ord);
        assert_eq!(el.tabulate(&[], 2), bad_ord);
        assert_eq!(
            el.tabulate(&[0.1, 0.2], 0),
            Err(ElementError::RefCell(RefCellError::PointsShape {
                len: 2,
                tdim: 3
            }))
        );
        for nderivs in [0, 1] {
            let empty = el.tabulate(&[], nderivs).unwrap();
            assert_eq!(
                (empty.nderivs, empty.num_points, empty.dim, empty.value_size),
                (nderivs, 0, 10, 1)
            );
            assert!(empty.values.is_empty());
        }
        let err = solve_ciarlet_coefficients(
            CellType::Interval,
            1,
            2,
            1,
            &[0.5, 0.5],
            &[1.0, 0.0, 0.0, 1.0],
        )
        .unwrap_err();
        assert_eq!(err, ElementError::RefCell(RefCellError::Singular));

        // Vector-valued Ciarlet path (value_size = 2): P_1^2 on the interval, point evaluations
        // of each component at the two endpoints.
        let mut eye = vec![0.0; 16];
        for i in 0..4 {
            eye[i * 4 + i] = 1.0;
        }
        let coeffs =
            solve_ciarlet_coefficients(CellType::Interval, 1, 4, 2, &[0.0, 1.0], &eye).unwrap();
        let tab =
            tabulate_from_coefficients(CellType::Interval, 1, 4, 2, &coeffs, &[0.25], 1).unwrap();
        assert_eq!(
            (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
            (1, 1, 4, 2)
        );
        let want_vals = [0.75, 0.0, 0.0, 0.75, 0.25, 0.0, 0.0, 0.25];
        let want_derivs = [-1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0];
        for (&g, w) in tab.values[..8].iter().zip(want_vals) {
            assert!((g - w).abs() < 1e-14);
        }
        for (&g, w) in tab.values[8..16].iter().zip(want_derivs) {
            assert!((g - w).abs() < 1e-14);
        }
    }
}

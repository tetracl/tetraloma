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

/// Constructs scalar (`reference_value_size == 1`) Ciarlet basis coefficients in
/// `tetraloma-refcell`'s orthonormal set from interpolation points (`[m][tdim]`) and matrix
/// (`[dim][m]`). M12 extends this with subspace span coefficients and the vector layout.
///
/// Forms `D[i, k] = l_i(ψ_k)`, solves `D Y = I` with `DenseLu`, and transposes `Y` into
/// `[dim][polyset_dim]` order so `φ_j = Σ_k C[j, k] ψ_k`.
pub(crate) fn solve_ciarlet_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
    interpolation_points: &[f64],
    interpolation_matrix: &[f64],
) -> Result<Vec<f64>, ElementError> {
    let td = tdim(cell);
    assert!(
        td > 0 && interpolation_points.len().is_multiple_of(td),
        "solve_ciarlet_coefficients: invalid point array length {} for tdim {td}",
        interpolation_points.len()
    );
    let m = interpolation_points.len() / td;
    let n_poly = polyset::dim(cell, degree);
    assert_eq!(
        dim, n_poly,
        "solve_ciarlet_coefficients: full scalar polyset expects dim == {n_poly}, got {dim}"
    );
    assert_eq!(
        interpolation_matrix.len(),
        dim * m,
        "solve_ciarlet_coefficients: expected {dim} x {m} matrix, got {}",
        interpolation_matrix.len()
    );

    let poly = polyset::tabulate(cell, degree, interpolation_points, 0);
    let mut poly_t = vec![0.0; m * n_poly];
    transpose_block(&poly, n_poly, m, &mut poly_t);

    let mut dual = vec![0.0; dim * dim];
    for (d_row, m_row) in dual
        .chunks_exact_mut(dim)
        .zip(interpolation_matrix.chunks_exact(m))
    {
        for (p_row, &a) in poly_t.chunks_exact(n_poly).zip(m_row) {
            if a != 0.0 {
                for (dv, &pv) in d_row.iter_mut().zip(p_row) {
                    *dv += a * pv;
                }
            }
        }
    }
    drop((poly, poly_t));

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
        &interpolation_points,
        &interpolation_matrix,
    )?;
    Ok(CiarletData {
        coefficients,
        interpolation_points,
        interpolation_matrix,
    })
}

/// Tabulates a scalar (`reference_value_size == 1`) element from its `polyset` expansion
/// `coefficients` (`[dim][n_poly]`) into `[derivative][point][basis][component]` order.
pub(crate) fn tabulate_from_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
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
            value_size: 1,
            values: Vec::new(),
        });
    }

    let n_poly = polyset::dim(cell, degree);
    assert_eq!(
        coefficients.len(),
        dim * n_poly,
        "tabulate_from_coefficients: expected {dim} x {n_poly} coefficients, got {}",
        coefficients.len()
    );
    let poly = polyset::tabulate(cell, degree, points, nderivs);
    let mut values = vec![0.0; num_derivs * num_points * dim];
    let mut poly_t = vec![0.0; num_points * n_poly];

    for (d, out_block) in values.chunks_exact_mut(num_points * dim).enumerate() {
        transpose_block(
            &poly[d * n_poly * num_points..(d + 1) * n_poly * num_points],
            n_poly,
            num_points,
            &mut poly_t,
        );
        for (p_row, out_q) in poly_t
            .chunks_exact(n_poly)
            .zip(out_block.chunks_exact_mut(dim))
        {
            for (dst, c_row) in out_q.iter_mut().zip(coefficients.chunks_exact(n_poly)) {
                *dst = c_row.iter().zip(p_row).map(|(&c, &p)| c * p).sum();
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

    /// Independent literal vertex midpoints and volumes for each reference cell (Appendix A.1).
    const CELL_MIDPOINTS_AND_VOLUMES: [(CellType, &[f64], f64); 7] = [
        (CellType::Interval, &[0.5], 1.0),
        (CellType::Triangle, &[1.0 / 3.0, 1.0 / 3.0], 0.5),
        (CellType::Quadrilateral, &[0.5, 0.5], 1.0),
        (CellType::Tetrahedron, &[0.25, 0.25, 0.25], 1.0 / 6.0),
        (CellType::Hexahedron, &[0.5, 0.5, 0.5], 1.0),
        (CellType::Prism, &[1.0 / 3.0, 1.0 / 3.0, 0.5], 0.5),
        (CellType::Pyramid, &[0.4, 0.4, 0.2], 1.0 / 3.0),
    ];

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
        for (cell, want_mid, vol) in CELL_MIDPOINTS_AND_VOLUMES {
            let td = tdim(cell);
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
                let got_mid = el.interpolation_points();
                assert_eq!(got_mid.len(), want_mid.len());
                for (&g, &w) in got_mid.iter().zip(want_mid) {
                    assert!((g - w).abs() < 1e-15, "{cell:?} {variant:?}: {g} vs {w}");
                }
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
        for (cell, pt, _) in CELL_MIDPOINTS_AND_VOLUMES {
            let td = tdim(cell);
            for p in 1..=5 {
                let el = Element::get(lagrange(cell, p, Variant::Legendre)).unwrap();
                let (n, tab) = (el.dim(), el.tabulate(pt, 1).unwrap());
                let poly = polyset::tabulate(cell, p, pt, 1);
                for d in 0..=td {
                    let s = poly[d * n..(d + 1) * n]
                        .iter()
                        .fold(1.0_f64, |m, v| m.max(v.abs()));
                    for j in 0..n {
                        assert!((tab.values[d * n + j] - poly[d * n + j]).abs() <= 1e-12 * s);
                    }
                }
                for d in 0..td {
                    let (mut xp, mut xm) = (pt.to_vec(), pt.to_vec());
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
            &[0.5, 0.5],
            &[1.0, 0.0, 0.0, 1.0],
        )
        .unwrap_err();
        assert_eq!(err, ElementError::RefCell(RefCellError::Singular));
    }
}

// Design: section 26 ("Finite elements"), Listing E1, "Construction", "Derivative orders",
// "Variants"; ADR-0071, ADR-0072, ADR-0080.
//! Ciarlet element construction and tabulation over `tetraloma-refcell`'s orthonormal polynomial
//! sets.

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    DenseLu, QuadratureRule, RefCellError, gauss_rule, lagrange_points, polyset, tdim, vertices,
};

use crate::{ElementError, ElementKey, ElementTabulation, Family, Variant};

/// Interpolation data and orthonormal-basis expansion coefficients from Ciarlet construction.
pub(crate) struct CiarletData {
    /// `[dim][component][polyset]` (row-major `[dim][value_size * polyset_dim]`).
    pub(crate) coefficients: Vec<f64>,
    /// `[m][tdim]`.
    pub(crate) interpolation_points: Vec<f64>,
    /// `[dim][point][component]` (row-major `[dim][m * value_size]`).
    pub(crate) interpolation_matrix: Vec<f64>,
}

/// One contiguous block of DOF functionals sharing a single set of reference sample points.
pub(crate) struct FunctionalBlock<'a> {
    /// Reference sample points in `[m_b][tdim]` order.
    pub(crate) points: &'a [f64],
    /// Functional weights in `[n_b][m_b][value_size]` order.
    pub(crate) weights: &'a [f64],
    /// Number of DOF rows (`n_b`) in this block.
    pub(crate) num_dofs: usize,
}

/// Builds the row-major `n × n` identity matrix.
pub(crate) fn identity(n: usize) -> Vec<f64> {
    let mut id = vec![0.0; n * n];
    for i in 0..n {
        id[i * n + i] = 1.0;
    }
    id
}

/// Transposes a row-major `rows × cols` block into `cols × rows` in `dst`.
pub(crate) fn transpose_block(src: &[f64], rows: usize, cols: usize, dst: &mut [f64]) {
    debug_assert_eq!(src.len(), rows * cols);
    debug_assert_eq!(dst.len(), rows * cols);
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

/// Builds `interpolation_points` for a validated Lagrange element key without assembling the
/// `dim × m` interpolation matrix.
pub(crate) fn lagrange_interpolation_points(
    cell: CellType,
    degree: u32,
    variant: Variant,
) -> Result<Vec<f64>, ElementError> {
    match variant {
        Variant::Nodal(family) => {
            if degree == 0 {
                Ok(cell_midpoint(cell))
            } else {
                Ok(lagrange_points(cell, degree, family)?)
            }
        }
        Variant::Legendre => Ok(gauss_rule(cell, 2 * degree).points),
    }
}

/// Builds `(interpolation_points, interpolation_matrix)` for a validated Lagrange element key.
pub(crate) fn lagrange_interpolation(
    cell: CellType,
    degree: u32,
    variant: Variant,
    dim: usize,
) -> Result<(Vec<f64>, Vec<f64>), ElementError> {
    match variant {
        Variant::Nodal(_) => {
            let points = lagrange_interpolation_points(cell, degree, variant)?;
            Ok((points, identity(dim)))
        }
        Variant::Legendre => {
            let rule = gauss_rule(cell, 2 * degree);
            let m = rule.weights.len();
            let phi = polyset::tabulate(cell, degree, &rule.points, 0);
            debug_assert_eq!(phi.len(), dim * m);
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

/// Orthonormalises a sequence of candidate vector polynomials over `polyset(cell, degree)` in
/// `[basis][component][polyset]` layout using two-pass modified Gram–Schmidt under an exact
/// quadrature `rule` (for example `gauss_rule(cell, 2 * degree)`).
///
/// `eval_candidate(i, &mut vals)` evaluates candidate `i` (`0..num_candidates`) at `rule.points`
/// in `[point][component]` order (`vals.len() == rule.weights.len() * value_size`). `vals` is
/// zero-initialised before each call, so callbacks need only write nonzero components. Candidates
/// whose residual norm after projection or orthogonalisation is at most `1e-10 * original_norm`
/// (or whose original norm is zero) are dropped as linearly dependent.
///
/// Returns `(num_basis, span)` with `span` in `[num_basis][value_size][n_poly]` layout.
#[cfg_attr(not(test), expect(dead_code))] // wired in M12.T7
pub(crate) fn orthonormalise_vector_basis(
    cell: CellType,
    degree: u32,
    value_size: usize,
    rule: &QuadratureRule,
    num_candidates: usize,
    mut eval_candidate: impl FnMut(usize, &mut [f64]),
) -> (usize, Vec<f64>) {
    const DROP_TOL: f64 = 1e-10;
    let td = tdim(cell);
    let nq = rule.weights.len();
    assert_eq!(
        rule.cell, cell,
        "orthonormalise_vector_basis: rule cell {:?} does not match {cell:?}",
        rule.cell
    );
    let min_rule_degree = degree
        .checked_mul(2)
        .expect("orthonormalise_vector_basis: 2 * degree overflows u32");
    assert!(
        rule.degree >= min_rule_degree,
        "orthonormalise_vector_basis: rule degree {} is below 2 * {degree} ({min_rule_degree})",
        rule.degree
    );
    assert!(
        td > 0 && value_size > 0 && nq > 0 && rule.points.len() == nq * td,
        "orthonormalise_vector_basis: invalid rule or value_size ({value_size})"
    );
    if num_candidates == 0 {
        return (0, Vec::new());
    }

    let n_poly = polyset::dim(cell, degree);
    let stride = value_size * n_poly;
    let mut w_poly = polyset::tabulate(cell, degree, &rule.points, 0);
    for row in w_poly.chunks_exact_mut(nq) {
        for (v, &w) in row.iter_mut().zip(&rule.weights) {
            *v *= w;
        }
    }

    let mut vals = vec![0.0; nq * value_size];
    let mut vals_t = vec![0.0; value_size * nq];
    let mut cand = vec![0.0; stride];
    let mut span = Vec::new();
    let mut num_kept = 0usize;

    for i in 0..num_candidates {
        vals.fill(0.0);
        eval_candidate(i, &mut vals);

        let orig_norm_sq: f64 = vals
            .chunks_exact(value_size)
            .zip(&rule.weights)
            .map(|(v, &w)| w * v.iter().map(|&x| x * x).sum::<f64>())
            .sum();
        let orig_norm = orig_norm_sq.sqrt();
        if orig_norm == 0.0 {
            continue;
        }

        transpose_block(&vals, nq, value_size, &mut vals_t);
        for c in 0..value_size {
            let vc = &vals_t[c * nq..(c + 1) * nq];
            let dst = &mut cand[c * n_poly..(c + 1) * n_poly];
            if vc.iter().all(|&v| v == 0.0) {
                dst.fill(0.0);
                continue;
            }
            for (d, wp_row) in dst.iter_mut().zip(w_poly.chunks_exact(nq)) {
                *d = wp_row.iter().zip(vc).map(|(&wp, &v)| wp * v).sum();
            }
        }

        let mut norm = cand.iter().map(|&x| x * x).sum::<f64>().sqrt();
        if norm <= DROP_TOL * orig_norm {
            continue;
        }

        let mut dependent = false;
        for _ in 0..2 {
            for qj in span.chunks_exact(stride) {
                let h: f64 = cand.iter().zip(qj).map(|(&a, &b)| a * b).sum();
                if h != 0.0 {
                    for (a, &b) in cand.iter_mut().zip(qj) {
                        *a -= h * b;
                    }
                }
            }
            norm = cand.iter().map(|&x| x * x).sum::<f64>().sqrt();
            if norm <= DROP_TOL * orig_norm {
                dependent = true;
                break;
            }
        }
        if dependent {
            continue;
        }

        let inv = 1.0 / norm;
        cand.iter_mut().for_each(|x| *x *= inv);
        span.extend_from_slice(&cand);
        num_kept += 1;
    }

    (num_kept, span)
}

/// Constructs Ciarlet basis coefficients in `[dim][component][polyset]` layout from per-entity
/// functional blocks.
///
/// `span` supplies an orthonormal subspace basis `[dim][value_size][n_poly]`; `None` is the
/// full-polyset scalar identity fast path (`value_size == 1`, `dim == n_poly`). Forms
/// `D[i, j] = l_i(w_j)`, solves `D Y = I` with `DenseLu` (keeping the dual residual
/// `l_i(φ_m) − δ_{i,m}` small even for ill-conditioned equispaced bases), and forms `C = Yᵀ S`
/// (`Yᵀ` when `span` is `None`) so `φ_m = Σ_k C[m, c, k] ψ_k e_c`.
pub(crate) fn solve_ciarlet_coefficients_from_blocks(
    cell: CellType,
    degree: u32,
    dim: usize,
    value_size: usize,
    span: Option<&[f64]>,
    blocks: &[FunctionalBlock<'_>],
) -> Result<Vec<f64>, ElementError> {
    let td = tdim(cell);
    assert!(
        td > 0 && value_size > 0 && dim > 0,
        "solve_ciarlet_coefficients: invalid td={td}, value_size={value_size}, dim={dim}"
    );
    let n_poly = polyset::dim(cell, degree);
    let stride = value_size * n_poly;
    if let Some(s) = span {
        assert_eq!(
            s.len(),
            dim * stride,
            "solve_ciarlet_coefficients: expected {dim} x {value_size} x {n_poly} span, got {}",
            s.len()
        );
    } else {
        assert!(
            value_size == 1 && dim == n_poly,
            "solve_ciarlet_coefficients: span=None requires scalar full polyset (value_size=1, dim={n_poly}), got value_size={value_size}, dim={dim}"
        );
    }
    let total_dofs: usize = blocks.iter().map(|b| b.num_dofs).sum();
    assert_eq!(
        total_dofs, dim,
        "solve_ciarlet_coefficients: functional blocks sum to {total_dofs} DOFs, expected {dim}"
    );

    let mut dual = vec![0.0; dim * dim];
    let mut l_row = vec![0.0; stride];
    let mut row_offset = 0usize;

    for block in blocks {
        if block.num_dofs == 0 {
            continue;
        }
        assert!(
            !block.points.is_empty() && block.points.len().is_multiple_of(td),
            "solve_ciarlet_coefficients: invalid point array length {} for tdim {td}",
            block.points.len()
        );
        let m_b = block.points.len() / td;
        let row_len = m_b * value_size;
        assert_eq!(
            block.weights.len(),
            block.num_dofs * row_len,
            "solve_ciarlet_coefficients: expected {} x {m_b} x {value_size} weights, got {}",
            block.num_dofs,
            block.weights.len()
        );

        let poly = polyset::tabulate(cell, degree, block.points, 0);
        let mut poly_t = vec![0.0; m_b * n_poly];
        transpose_block(&poly, n_poly, m_b, &mut poly_t);
        drop(poly);

        for (r, w_row) in block.weights.chunks_exact(row_len).enumerate() {
            let d_row = &mut dual[(row_offset + r) * dim..(row_offset + r + 1) * dim];
            l_row.fill(0.0);
            for (p_row, w_q) in poly_t
                .chunks_exact(n_poly)
                .zip(w_row.chunks_exact(value_size))
            {
                for (c, &a) in w_q.iter().enumerate() {
                    if a != 0.0 {
                        for (dst, &pv) in l_row[c * n_poly..(c + 1) * n_poly].iter_mut().zip(p_row)
                        {
                            *dst += a * pv;
                        }
                    }
                }
            }
            if let Some(s) = span {
                for (dv, s_row) in d_row.iter_mut().zip(s.chunks_exact(stride)) {
                    *dv = l_row.iter().zip(s_row).map(|(&a, &b)| a * b).sum();
                }
            } else {
                d_row.copy_from_slice(&l_row);
            }
        }
        row_offset += block.num_dofs;
    }

    let lu = DenseLu::factor(dual, dim)?;
    let mut inv = identity(dim);
    lu.solve(&mut inv, dim);
    drop(lu);

    let mut coefficients = vec![0.0; dim * stride];
    if let Some(s) = span {
        for (j, s_row) in s.chunks_exact(stride).enumerate() {
            let inv_row = &inv[j * dim..(j + 1) * dim];
            for (c_row, &y) in coefficients.chunks_exact_mut(stride).zip(inv_row) {
                if y != 0.0 {
                    for (dst, &sv) in c_row.iter_mut().zip(s_row) {
                        *dst += y * sv;
                    }
                }
            }
        }
    } else {
        transpose_block(&inv, dim, dim, &mut coefficients);
    }
    Ok(coefficients)
}

/// Constructs Ciarlet basis coefficients in `tetraloma-refcell`'s orthonormal set
/// (`[dim][component][polyset]`) from flat interpolation points (`[m][tdim]`) and matrix
/// (`[dim][point][component]`).
pub(crate) fn solve_ciarlet_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
    value_size: usize,
    span: Option<&[f64]>,
    interpolation_points: &[f64],
    interpolation_matrix: &[f64],
) -> Result<Vec<f64>, ElementError> {
    let block = FunctionalBlock {
        points: interpolation_points,
        weights: interpolation_matrix,
        num_dofs: dim,
    };
    solve_ciarlet_coefficients_from_blocks(cell, degree, dim, value_size, span, &[block])
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
        None,
        &interpolation_points,
        &interpolation_matrix,
    )?;
    Ok(CiarletData {
        coefficients,
        interpolation_points,
        interpolation_matrix,
    })
}

/// Validates `nderivs` (0 or 1) and `points` shape for `cell`, returning the number of points.
pub(crate) fn validate_tabulate_args(
    cell: CellType,
    points: &[f64],
    nderivs: u32,
) -> Result<usize, ElementError> {
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
    Ok(points.len().checked_div(td).unwrap_or(1))
}

/// Tabulates an element with `value_size` reference components from its `polyset` expansion
/// `coefficients` (`[dim][component][polyset]`) into `[derivative][point][basis][component]` order.
pub(crate) fn tabulate_from_coefficients(
    cell: CellType,
    degree: u32,
    dim: usize,
    value_size: usize,
    coefficients: &[f64],
    points: &[f64],
    nderivs: u32,
) -> Result<ElementTabulation, ElementError> {
    let num_points = validate_tabulate_args(cell, points, nderivs)?;
    assert!(
        value_size > 0,
        "tabulate_from_coefficients: value_size must be positive"
    );
    let n_poly = polyset::dim(cell, degree);
    let stride = value_size * n_poly;
    assert_eq!(
        coefficients.len(),
        dim * stride,
        "tabulate_from_coefficients: expected {dim} x {value_size} x {n_poly} coefficients, got {}",
        coefficients.len()
    );
    let td = tdim(cell);
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

    let poly = polyset::tabulate(cell, degree, points, nderivs);
    let point_stride = dim * value_size;
    let deriv_stride = num_points * point_stride;
    let mut values = vec![0.0; num_derivs * deriv_stride];
    let mut poly_t = vec![0.0; num_points * n_poly];

    for (d, out_block) in values.chunks_exact_mut(deriv_stride).enumerate() {
        transpose_block(
            &poly[d * n_poly * num_points..(d + 1) * n_poly * num_points],
            n_poly,
            num_points,
            &mut poly_t,
        );
        for (p_row, out_q) in poly_t
            .chunks_exact(n_poly)
            .zip(out_block.chunks_exact_mut(point_stride))
        {
            for (out_basis, c_basis) in out_q
                .chunks_exact_mut(value_size)
                .zip(coefficients.chunks_exact(stride))
            {
                for (dst, c_comp) in out_basis.iter_mut().zip(c_basis.chunks_exact(n_poly)) {
                    *dst = c_comp.iter().zip(p_row).map(|(&c, &p)| c * p).sum();
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
            1,
            None,
            &[0.5, 0.5],
            &[1.0, 0.0, 0.0, 1.0],
        )
        .unwrap_err();
        assert_eq!(err, ElementError::RefCell(RefCellError::Singular));
    }

    #[test]
    fn identity_and_transpose_block_helpers() {
        assert_eq!(identity(0), Vec::<f64>::new());
        assert_eq!(identity(1), vec![1.0]);
        assert_eq!(
            identity(3),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );

        let src = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut dst = [0.0; 6];
        transpose_block(&src, 2, 3, &mut dst);
        assert_eq!(dst, [1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);

        let mut empty = [];
        transpose_block(&[], 0, 3, &mut empty);
        transpose_block(&[], 3, 0, &mut empty);
    }

    fn assert_span_orthonormal(span: &[f64], dim: usize, stride: usize) {
        for i in 0..dim {
            for j in 0..dim {
                let dot: f64 = span[i * stride..(i + 1) * stride]
                    .iter()
                    .zip(&span[j * stride..(j + 1) * stride])
                    .map(|(&a, &b)| a * b)
                    .sum();
                let want = f64::from(u8::from(i == j));
                assert!((dot - want).abs() < 1e-13, "({i},{j}): {dot}");
            }
        }
    }

    #[test]
    fn orthonormalise_vector_basis_drops_dependent_and_is_l2_orthonormal() {
        let tri_rule = gauss_rule(CellType::Triangle, 4);
        assert_eq!(
            orthonormalise_vector_basis(CellType::Triangle, 2, 2, &tri_rule, 0, |_, _| {}),
            (0, Vec::new())
        );

        // Triangle P_2^2 with monomials plus zero, duplicate, near-collinear (two-pass MGS) and
        // linear-combination candidates.
        let nq = tri_rule.weights.len();
        let mut cands = Vec::new();
        for c in 0..2 {
            for i in 0..=2usize {
                for j in 0..=(2 - i) {
                    let mut v = vec![0.0; nq * 2];
                    for q in 0..nq {
                        let (x, y) = (tri_rule.points[2 * q], tri_rule.points[2 * q + 1]);
                        v[2 * q + c] = x.powi(i as i32) * y.powi(j as i32);
                    }
                    cands.push(v);
                }
            }
        }
        let lin_comb: Vec<f64> = cands[1]
            .iter()
            .zip(&cands[4])
            .map(|(&a, &b)| 2.5 * a - 1.75 * b)
            .collect();
        let near: Vec<f64> = cands[0]
            .iter()
            .zip(&cands[1])
            .map(|(&a, &b)| a + 1e-6 * b)
            .collect();
        let mut all = vec![cands[0].clone(), vec![0.0; nq * 2], cands[0].clone(), near];
        all.extend(cands.into_iter().skip(1));
        all.push(lin_comb);

        let (dim, span) =
            orthonormalise_vector_basis(CellType::Triangle, 2, 2, &tri_rule, all.len(), |i, d| {
                d.copy_from_slice(&all[i]);
            });
        let n_poly = polyset::dim(CellType::Triangle, 2);
        assert_eq!((dim, span.len()), (2 * n_poly, 4 * n_poly * n_poly));
        assert_span_orthonormal(&span, dim, 2 * n_poly);

        // Cross-check orthonormality and exact candidate reproduction by quadrature of tabulated
        // basis functions (including the dropped linear combination).
        let tab =
            tabulate_from_coefficients(CellType::Triangle, 2, dim, 2, &span, &tri_rule.points, 0)
                .unwrap();
        for i in 0..dim {
            for j in 0..dim {
                let q_dot: f64 = (0..nq)
                    .map(|q| {
                        let (bi, bj) = ((q * dim + i) * 2, (q * dim + j) * 2);
                        tri_rule.weights[q]
                            * (tab.values[bi] * tab.values[bj]
                                + tab.values[bi + 1] * tab.values[bj + 1])
                    })
                    .sum();
                assert!(
                    (q_dot - f64::from(u8::from(i == j))).abs() < 1e-13,
                    "({i},{j}): {q_dot}"
                );
            }
        }
        for (idx, cand) in all.iter().enumerate() {
            let proj: Vec<f64> = (0..dim)
                .map(|j| {
                    (0..nq)
                        .map(|q| {
                            let bj = (q * dim + j) * 2;
                            tri_rule.weights[q]
                                * (cand[2 * q] * tab.values[bj]
                                    + cand[2 * q + 1] * tab.values[bj + 1])
                        })
                        .sum()
                })
                .collect();
            for q in 0..nq {
                for c in 0..2 {
                    let recon: f64 = (0..dim)
                        .map(|j| proj[j] * tab.values[(q * dim + j) * 2 + c])
                        .sum();
                    assert!(
                        (recon - cand[2 * q + c]).abs() < 1e-12,
                        "cand {idx} q={q} c={c}: {recon} vs {}",
                        cand[2 * q + c]
                    );
                }
            }
        }

        // 3D strict subspace: Tetrahedron N1curl_2 = P_1^3 ⊕ (x × P_1^3), dim = 20 (with x × x = 0).
        let tet_rule = gauss_rule(CellType::Tetrahedron, 4);
        let (nq3, n_p1) = (
            tet_rule.weights.len(),
            polyset::dim(CellType::Tetrahedron, 1),
        );
        let p1 = polyset::tabulate(CellType::Tetrahedron, 1, &tet_rule.points, 0);
        let eval_tet_cand = |idx: usize, dst: &mut [f64]| {
            let (m, c) = ((idx / 3) % n_p1, idx % 3);
            if idx < 3 * n_p1 {
                for q in 0..nq3 {
                    dst[3 * q + c] = p1[m * nq3 + q];
                }
            } else {
                let (a, b) = ((c + 1) % 3, (c + 2) % 3);
                for q in 0..nq3 {
                    let (x, pm) = (&tet_rule.points[3 * q..3 * (q + 1)], p1[m * nq3 + q]);
                    (dst[3 * q + a], dst[3 * q + b]) = (x[b] * pm, -x[a] * pm);
                }
            }
        };
        let (dim3, span3) = orthonormalise_vector_basis(
            CellType::Tetrahedron,
            2,
            3,
            &tet_rule,
            6 * n_p1,
            eval_tet_cand,
        );
        assert_eq!(dim3, 20);
        assert_span_orthonormal(&span3, dim3, 3 * polyset::dim(CellType::Tetrahedron, 2));
        // Verify span3 reproduces its 3D cross-product candidates by L² projection.
        let tab3 = tabulate_from_coefficients(
            CellType::Tetrahedron,
            2,
            dim3,
            3,
            &span3,
            &tet_rule.points,
            0,
        )
        .unwrap();
        let mut sample_cand = vec![0.0; nq3 * 3];
        for idx in [0, 3 * n_p1, 3 * n_p1 + 4, 6 * n_p1 - 1] {
            sample_cand.fill(0.0);
            eval_tet_cand(idx, &mut sample_cand);
            let proj: Vec<f64> = (0..dim3)
                .map(|j| {
                    (0..nq3)
                        .map(|q| {
                            let bj = (q * dim3 + j) * 3;
                            tet_rule.weights[q]
                                * (0..3)
                                    .map(|c| sample_cand[3 * q + c] * tab3.values[bj + c])
                                    .sum::<f64>()
                        })
                        .sum()
                })
                .collect();
            for q in 0..nq3 {
                for c in 0..3 {
                    let recon: f64 = (0..dim3)
                        .map(|j| proj[j] * tab3.values[(q * dim3 + j) * 3 + c])
                        .sum();
                    assert!(
                        (recon - sample_cand[3 * q + c]).abs() < 1e-12,
                        "tet cand {idx} q={q} c={c}: {recon} vs {}",
                        sample_cand[3 * q + c]
                    );
                }
            }
        }
    }

    #[test]
    fn generalised_ciarlet_solve_and_tabulate_on_synthetic_vector_spaces() {
        // 1. P_p^2 on Triangle (p = 1..=3) and P_p^3 on Tetrahedron (p = 1..=2): flat vs
        //    multi-block FunctionalBlock solve, M*B = I, and vector polynomial reproduction.
        let tri_test = [0.11, 0.23, 0.57, 0.19, 0.31, 0.43];
        let tet_test = [0.11, 0.17, 0.23, 0.37, 0.19, 0.21];
        for (cell, max_p, test_pts) in [
            (CellType::Triangle, 3u32, &tri_test[..]),
            (CellType::Tetrahedron, 2u32, &tet_test[..]),
        ] {
            let td = tdim(cell);
            for p in 1..=max_p {
                let pts = lagrange_points(cell, p, NodeFamily::Gll).unwrap();
                let m = pts.len() / td;
                let (dim, span) = (td * m, identity(td * polyset::dim(cell, p)));
                let mut mat = vec![0.0; dim * m * td];
                for q in 0..m {
                    for c in 0..td {
                        mat[((q * td + c) * m + q) * td + c] = 1.0;
                    }
                }
                let flat =
                    solve_ciarlet_coefficients(cell, p, dim, td, Some(&span), &pts, &mat).unwrap();
                let id_td = identity(td);
                let mut blocks = vec![FunctionalBlock {
                    points: &[],
                    weights: &[],
                    num_dofs: 0,
                }];
                blocks.extend((0..m).map(|q| FunctionalBlock {
                    points: &pts[td * q..td * (q + 1)],
                    weights: &id_td,
                    num_dofs: td,
                }));
                let blocked =
                    solve_ciarlet_coefficients_from_blocks(cell, p, dim, td, Some(&span), &blocks)
                        .unwrap();
                assert!(
                    flat.iter()
                        .zip(&blocked)
                        .all(|(&a, &b)| (a - b).abs() < 1e-13)
                );

                let tab = tabulate_from_coefficients(cell, p, dim, td, &flat, &pts, 0).unwrap();
                for i in 0..dim {
                    for j in 0..dim {
                        let s: f64 = (0..m)
                            .flat_map(|q| (0..td).map(move |c| (q, c)))
                            .map(|(q, c)| {
                                mat[(i * m + q) * td + c] * tab.values[(q * dim + j) * td + c]
                            })
                            .sum();
                        assert!((s - f64::from(u8::from(i == j))).abs() < 1e-12);
                    }
                }
                let eval_u = |x: &[f64]| -> Vec<f64> {
                    (0..td)
                        .map(|c| {
                            x[c].powi(p as i32) + (c as f64 + 1.0) * x[(c + 1) % td]
                                - 0.5 * (c as f64)
                        })
                        .collect()
                };
                let dofs: Vec<f64> = (0..m)
                    .flat_map(|q| eval_u(&pts[td * q..td * (q + 1)]))
                    .collect();
                let ev = tabulate_from_coefficients(cell, p, dim, td, &flat, test_pts, 0).unwrap();
                for q in 0..(test_pts.len() / td) {
                    for (c, &wc) in eval_u(&test_pts[td * q..td * (q + 1)]).iter().enumerate() {
                        let got: f64 = (0..dim)
                            .map(|j| dofs[j] * ev.values[(q * dim + j) * td + c])
                            .sum();
                        assert!(
                            (got - wc).abs() < 1e-12,
                            "{cell:?} p={p} q={q} c={c}: {got} vs {wc}"
                        );
                    }
                }
            }
        }

        // 2. P_2^3 on Tetrahedron via L²-moment functionals (coefficients must recover identity).
        let rule3 = gauss_rule(CellType::Tetrahedron, 4);
        let (nq3, np3) = (rule3.weights.len(), polyset::dim(CellType::Tetrahedron, 2));
        let (dim3, span3) = (3 * np3, identity(3 * np3));
        let poly3 = polyset::tabulate(CellType::Tetrahedron, 2, &rule3.points, 0);
        let mut mat3 = vec![0.0; dim3 * nq3 * 3];
        for c in 0..3 {
            for k in 0..np3 {
                for q in 0..nq3 {
                    mat3[((c * np3 + k) * nq3 + q) * 3 + c] = rule3.weights[q] * poly3[k * nq3 + q];
                }
            }
        }
        let c3 = solve_ciarlet_coefficients(
            CellType::Tetrahedron,
            2,
            dim3,
            3,
            Some(&span3),
            &rule3.points,
            &mat3,
        )
        .unwrap();
        assert!(c3.iter().zip(&span3).all(|(&g, &w)| (g - w).abs() < 1e-13));

        // 3. Strict subspaces (dim < value_size * n_poly): RT_1 = P_0^d ⊕ x P_0 on Triangle
        //    (dim = 3 < 6) and Tetrahedron (dim = 4 < 12) via facet-midpoint normal-flux blocks,
        //    checking both l_i(φ_j) = δ_{ij} and reproduction of a + b·x.
        let tri_mids: &[&[f64]] = &[&[0.5, 0.5], &[0.0, 0.5], &[0.5, 0.0]];
        let tri_norms: &[&[f64]] = &[&[1.0, 1.0], &[-1.0, 0.0], &[0.0, -1.0]];
        let tet_mids: &[&[f64]] = &[
            &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
            &[0.0, 1.0 / 3.0, 1.0 / 3.0],
            &[1.0 / 3.0, 0.0, 1.0 / 3.0],
            &[1.0 / 3.0, 1.0 / 3.0, 0.0],
        ];
        let tet_norms: &[&[f64]] = &[
            &[0.5, 0.5, 0.5],
            &[-0.5, 0.0, 0.0],
            &[0.0, -0.5, 0.0],
            &[0.0, 0.0, -0.5],
        ];
        for (cell, mids, norms, eval_pts) in [
            (CellType::Triangle, tri_mids, tri_norms, &tri_test[..]),
            (CellType::Tetrahedron, tet_mids, tet_norms, &tet_test[..]),
        ] {
            let td = tdim(cell);
            let rt_dim_want = td + 1;
            let rule = gauss_rule(cell, 2);
            let nq = rule.weights.len();
            let (rt_dim, rt_span) =
                orthonormalise_vector_basis(cell, 1, td, &rule, rt_dim_want, |idx, dst| {
                    for q in 0..nq {
                        if idx < td {
                            dst[td * q + idx] = 1.0;
                        } else {
                            for c in 0..td {
                                dst[td * q + c] = rule.points[td * q + c];
                            }
                        }
                    }
                });
            assert_eq!(rt_dim, rt_dim_want);
            let blocks: Vec<_> = (0..rt_dim)
                .map(|f| FunctionalBlock {
                    points: mids[f],
                    weights: norms[f],
                    num_dofs: 1,
                })
                .collect();
            let rt_c = solve_ciarlet_coefficients_from_blocks(
                cell,
                1,
                rt_dim,
                td,
                Some(&rt_span),
                &blocks,
            )
            .unwrap();
            let flat_mids: Vec<f64> = mids.iter().flat_map(|m| m.iter().copied()).collect();
            let rt_tab =
                tabulate_from_coefficients(cell, 1, rt_dim, td, &rt_c, &flat_mids, 0).unwrap();
            for (i, n) in norms.iter().enumerate() {
                for j in 0..rt_dim {
                    let flux: f64 = (0..td)
                        .map(|c| n[c] * rt_tab.values[(i * rt_dim + j) * td + c])
                        .sum();
                    assert!((flux - f64::from(u8::from(i == j))).abs() < 1e-13);
                }
            }
            // Reproduce u(x) = a + b * x (an arbitrary RT_1 field) at interior points.
            let a_vec = [1.25, -0.75, 0.5];
            let b_scale = 2.5;
            let dofs: Vec<f64> = (0..rt_dim)
                .map(|f| {
                    (0..td)
                        .map(|c| norms[f][c] * (a_vec[c] + b_scale * mids[f][c]))
                        .sum()
                })
                .collect();
            let ev = tabulate_from_coefficients(cell, 1, rt_dim, td, &rt_c, eval_pts, 0).unwrap();
            for q in 0..(eval_pts.len() / td) {
                for c in 0..td {
                    let want = a_vec[c] + b_scale * eval_pts[td * q + c];
                    let got: f64 = (0..rt_dim)
                        .map(|j| dofs[j] * ev.values[(q * rt_dim + j) * td + c])
                        .sum();
                    assert!(
                        (got - want).abs() < 1e-12,
                        "{cell:?} RT_1 q={q} c={c}: {got} vs {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn vector_tabulate_derivatives_match_central_differences_and_errors() {
        let h = 1e-6;
        for (cell, pt, p) in [
            (CellType::Triangle, &[0.27, 0.31][..], 3u32),
            (CellType::Tetrahedron, &[0.21, 0.23, 0.19][..], 2u32),
        ] {
            let (td, n_poly) = (tdim(cell), polyset::dim(cell, p));
            let (dim, coeffs) = (td * n_poly, identity(td * n_poly));
            let tab = tabulate_from_coefficients(cell, p, dim, td, &coeffs, pt, 1).unwrap();
            assert_eq!(
                (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
                (1, 1, dim, td)
            );
            let block = dim * td;
            for d in 0..td {
                let (mut xp, mut xm) = (pt.to_vec(), pt.to_vec());
                (xp[d], xm[d]) = (xp[d] + h, xm[d] - h);
                let tp = tabulate_from_coefficients(cell, p, dim, td, &coeffs, &xp, 0).unwrap();
                let tm = tabulate_from_coefficients(cell, p, dim, td, &coeffs, &xm, 0).unwrap();
                let deriv = &tab.values[(1 + d) * block..(2 + d) * block];
                let scale = deriv.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                for (idx, &dv) in deriv.iter().enumerate() {
                    let fd = (tp.values[idx] - tm.values[idx]) / (2.0 * h);
                    assert!((fd - dv).abs() < 1e-6 * scale * ((p * p) as f64));
                }
            }
            for nd in [0, 1] {
                let e = tabulate_from_coefficients(cell, p, dim, td, &coeffs, &[], nd).unwrap();
                assert_eq!(
                    (e.nderivs, e.num_points, e.dim, e.value_size),
                    (nd, 0, dim, td)
                );
                assert!(e.values.is_empty());
            }
            assert_eq!(
                tabulate_from_coefficients(cell, p, dim, td, &coeffs, &[], 2),
                Err(ElementError::RefCell(
                    RefCellError::UnsupportedDerivativeOrder(2)
                ))
            );
            assert_eq!(
                tabulate_from_coefficients(cell, p, dim, td, &coeffs, &[0.1], 0),
                Err(ElementError::RefCell(RefCellError::PointsShape {
                    len: 1,
                    tdim: td
                }))
            );
        }
        let span = identity(6);
        assert_eq!(
            solve_ciarlet_coefficients(
                CellType::Triangle,
                1,
                6,
                2,
                Some(&span),
                &[0.25, 0.25],
                &[1.0; 12]
            )
            .unwrap_err(),
            ElementError::RefCell(RefCellError::Singular)
        );
    }

    #[test]
    #[should_panic(expected = "below 2 * 2")]
    fn orthonormalise_vector_basis_rejects_under_integrating_rule() {
        let rule = gauss_rule(CellType::Triangle, 3);
        let _ = orthonormalise_vector_basis(CellType::Triangle, 2, 2, &rule, 0, |_, _| {});
    }
}

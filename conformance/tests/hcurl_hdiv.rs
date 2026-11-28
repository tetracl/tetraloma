//! M12 acceptance A2: for N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra,
//! p = 1–8, the span of the basis functions associated with each sub-entity closure equals
//! ndelement's, by a rank and projection test at 100 random points (`max(100, ceil(dim / tdim) + 20)`
//! for the cell closure when `dim >= 100 * tdim`) with tolerance 1e-10 (docs/design.md, A.10;
//! ADR-0080; docs/plans/M12.md, A2).

use std::sync::atomic::{AtomicUsize, Ordering};

use ndelement::ciarlet::{nedelec, raviart_thomas};
use ndelement::traits::FiniteElement;
use ndelement::types::Continuity;
use rlst::DynArray;
use rlst::dense::linalg::lapack::interface::geqp3::Geqp3;
use rlst::dense::linalg::lapack::interface::mqr::{Mqr, MqrSide, MqrTransMode};
use tetraloma_conformance::reference_cell_type;
use tetraloma_dims::CellType;
use tetraloma_element::{Element, ElementKey, Family, Variant};
use tetraloma_refcell::{num_sub_entities, tdim};

const SPAN_TOL: f64 = 1e-10;
const RANK_TOL: f64 = 1e-10;
const MIN_POINTS: usize = 100;
const CELL_MARGIN_POINTS: usize = 20;
const MAX_DEGREE: u32 = 8;

/// Stack for worker threads. With OpenBLAS (the hosted Linux job), ndelement's element
/// construction can overflow the test harness's 2 MiB thread stack; matching `lagrange.rs`.
const STACK_BYTES: usize = 64 << 20;

const CELLS: [CellType; 4] = [
    CellType::Triangle,
    CellType::Quadrilateral,
    CellType::Tetrahedron,
    CellType::Hexahedron,
];

/// splitmix64 (Steele, Lea and Flood 2014), so the test needs no random-number crate.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform `f64` strictly inside `(0, 1)` (52 bits + 0.5, exact in IEEE-754 f64).
    fn next_unit(&mut self) -> f64 {
        ((self.next_u64() >> 12) as f64 + 0.5) / ((1u64 << 52) as f64)
    }
}

/// Deterministic per-case RNG seed so results are independent of worker scheduling.
fn case_seed(family: Family, cell: CellType, degree: u32) -> u64 {
    let f = match family {
        Family::N1curl => 1u64,
        Family::RaviartThomas => 2u64,
        _ => unreachable!("unexpected family {family:?}"),
    };
    let c = match cell {
        CellType::Triangle => 1u64,
        CellType::Quadrilateral => 2u64,
        CellType::Tetrahedron => 3u64,
        CellType::Hexahedron => 4u64,
        _ => unreachable!("unexpected cell {cell:?}"),
    };
    0x7465_7472_614d_3132 ^ (f << 40) ^ (c << 24) ^ u64::from(degree)
}

/// Deterministic pseudo-random interior reference points of `cell`, flat node-major `[n][tdim]`.
fn random_interior_points(cell: CellType, n: usize, rng: &mut SplitMix64) -> Vec<f64> {
    let td = tdim(cell);
    let mut pts = Vec::with_capacity(n * td);
    for _ in 0..n {
        match cell {
            CellType::Triangle => loop {
                let (u, v) = (rng.next_unit(), rng.next_unit());
                let (x, y) = if u + v > 1.0 {
                    (1.0 - u, 1.0 - v)
                } else {
                    (u, v)
                };
                if x > 0.0 && y > 0.0 && x + y < 1.0 {
                    pts.extend_from_slice(&[x, y]);
                    break;
                }
            },
            CellType::Quadrilateral => pts.extend_from_slice(&[rng.next_unit(), rng.next_unit()]),
            CellType::Tetrahedron => loop {
                let p = [rng.next_unit(), rng.next_unit(), rng.next_unit()];
                if p[0] + p[1] + p[2] < 1.0 {
                    pts.extend_from_slice(&p);
                    break;
                }
            },
            CellType::Hexahedron => {
                pts.extend_from_slice(&[rng.next_unit(), rng.next_unit(), rng.next_unit()]);
            }
            other => panic!("unexpected cell type {other:?} for A2"),
        }
    }
    pts
}

fn col_norm(col: &[f64]) -> f64 {
    col.iter().map(|&x| x * x).sum::<f64>().sqrt()
}

/// Column-pivoted Householder QR (`dgeqp3`) of a column-major `rows x cols` matrix (`rows >= cols`)
/// after scaling each column to unit 2-norm so `r_ratio = min_i |R[i,i]| / |R[0,0]|` measures
/// linear dependence independently of column scaling (`0.0` if any column is zero or non-finite).
fn pivoted_qr(mat: &[f64], rows: usize, cols: usize) -> (Vec<f64>, Vec<f64>, f64) {
    assert!(rows >= cols && cols >= 1, "invalid shape {rows} x {cols}");
    assert_eq!(mat.len(), rows * cols);
    let mut a = mat.to_vec();
    for j in 0..cols {
        let col = &mut a[j * rows..(j + 1) * rows];
        let norm = col_norm(col);
        if !norm.is_finite() || norm == 0.0 {
            return (a, vec![0.0; cols], 0.0);
        }
        for x in col {
            *x /= norm;
        }
    }
    let mut jpvt = vec![0i32; cols];
    let mut tau = vec![0.0_f64; cols];
    f64::geqp3(rows, cols, &mut a, rows, &mut jpvt, &mut tau).expect("LAPACK dgeqp3 failed");

    if !a.iter().all(|x| x.is_finite()) {
        return (a, tau, 0.0);
    }
    let r0 = a[0].abs();
    if r0 == 0.0 {
        return (a, tau, 0.0);
    }
    let r_min = (0..cols)
        .map(|i| a[i * rows + i].abs())
        .fold(f64::INFINITY, f64::min);
    (a, tau, r_min / r0)
}

/// Maximum relative residual `max_j ||(I - Q_1 Q_1^T) v_j||_2 / ||v_j||_2`, computed directly from
/// `Q^T V` via LAPACK `dormqr` without forming `Q` explicitly.
fn max_projection_residual(
    qr_a: &[f64],
    tau: &[f64],
    target: &[f64],
    rows: usize,
    cols: usize,
) -> Result<f64, String> {
    assert!(rows > cols && cols >= 1 && target.len() == rows * cols);
    let mut w = target.to_vec();
    f64::mqr(
        MqrSide::Left,
        MqrTransMode::ConjugateTranspose,
        rows,
        cols,
        cols,
        qr_a,
        rows,
        tau,
        &mut w,
        rows,
    )
    .expect("LAPACK dormqr failed");

    let mut worst = 0.0_f64;
    for j in 0..cols {
        let v_norm = col_norm(&target[j * rows..(j + 1) * rows]);
        if !v_norm.is_finite() || v_norm == 0.0 {
            return Err(format!("column {j} of target has invalid norm {v_norm:e}"));
        }
        let rel = col_norm(&w[j * rows + cols..(j + 1) * rows]) / v_norm;
        if !rel.is_finite() {
            return Err(format!("column {j} produced non-finite residual {rel}"));
        }
        worst = worst.max(rel);
    }
    Ok(worst)
}

fn check_same_span(
    u: &[f64],
    v: &[f64],
    rows: usize,
    cols: usize,
    rank_tol: f64,
    span_tol: f64,
) -> Result<(f64, f64), String> {
    if rows <= cols {
        return Err(format!("non-overdetermined span test ({rows} <= {cols})"));
    }
    let (qr_u, tau_u, ru) = pivoted_qr(u, rows, cols);
    let (qr_v, tau_v, rv) = pivoted_qr(v, rows, cols);
    if ru <= rank_tol || rv <= rank_tol {
        return Err(format!(
            "rank-deficient ({rows}x{cols}): U={ru:e}, V={rv:e} <= {rank_tol:e}"
        ));
    }
    let res_v = max_projection_residual(&qr_u, &tau_u, v, rows, cols)?;
    let res_u = max_projection_residual(&qr_v, &tau_v, u, rows, cols)?;
    if res_v > span_tol || res_u > span_tol {
        return Err(format!(
            "span mismatch ({rows}x{cols}): V_on_U={res_v:e}, U_on_V={res_u:e} > {span_tol:e}"
        ));
    }
    Ok((res_v, res_u))
}

fn tabulate_ndelement<E: FiniteElement<T = f64, TGeo = f64>>(
    nd_el: &E,
    pts: &[f64],
    td: usize,
) -> DynArray<f64, 4> {
    let m = pts.len() / td;
    let mut nd_pts = DynArray::<f64, 2>::from_shape([td, m]);
    for q in 0..m {
        for c in 0..td {
            nd_pts[[c, q]] = pts[q * td + c];
        }
    }
    let shape = nd_el.tabulate_array_shape(0, m);
    assert_eq!(shape, [m, td, 1, nd_el.dim()], "ndelement shape");
    let mut data = DynArray::<f64, 4>::from_shape(shape);
    nd_el.tabulate(&nd_pts, 0, &mut data);
    data
}

fn pack_nd_cols(nd_data: &DynArray<f64, 4>, dofs: &[usize], m: usize, td: usize) -> Vec<f64> {
    let rows = m * td;
    let mut out = vec![0.0_f64; rows * dofs.len()];
    for (j, &b) in dofs.iter().enumerate() {
        for q in 0..m {
            for c in 0..td {
                out[j * rows + q * td + c] = nd_data[[q, c, 0, b]];
            }
        }
    }
    out
}

fn check_element_spans<E: FiniteElement<T = f64, TGeo = f64>>(
    family: Family,
    cell: CellType,
    degree: u32,
    nd_el: &E,
) {
    let td = tdim(cell);
    let key = ElementKey {
        family,
        cell,
        degree,
        variant: Variant::Legendre,
    };
    let el = Element::get(key).unwrap_or_else(|e| panic!("Element::get({key:?}): {e}"));
    let dim = el.dim();
    assert_eq!(dim, nd_el.dim(), "{key:?}: dim mismatch");
    assert_eq!(
        (el.reference_value_size(), nd_el.value_size()),
        (td, td),
        "{key:?}"
    );

    let m_cell = if dim >= MIN_POINTS * td {
        dim.div_ceil(td) + CELL_MARGIN_POINTS
    } else {
        MIN_POINTS
    };
    let mut rng = SplitMix64(case_seed(family, cell, degree));
    let pts = random_interior_points(cell, m_cell, &mut rng);

    let tab = el
        .tabulate(&pts, 0)
        .unwrap_or_else(|e| panic!("tabulate({key:?}): {e}"));
    assert_eq!(
        (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
        (0, m_cell, dim, td)
    );
    let nd_data = tabulate_ndelement(nd_el, &pts, td);

    for d in 0..=td {
        let m = if d < td { MIN_POINTS } else { m_cell };
        let rows = m * td;
        for e in 0..num_sub_entities(cell, d) {
            let nd_ent = nd_el.entity_dofs(d, e).expect("ndelement entity_dofs");
            assert_eq!(
                el.entity_dofs(d, e).len(),
                nd_ent.len(),
                "{key:?} ({d}, {e})"
            );
            let our_dofs = el.entity_closure_dofs(d, e);
            let nd_dofs = nd_el.entity_closure_dofs(d, e).expect("ndelement closure");
            assert_eq!(our_dofs.len(), nd_dofs.len(), "{key:?} closure ({d}, {e})");
            let k = our_dofs.len();
            if k == 0 {
                continue;
            }

            let mut u = vec![0.0_f64; rows * k];
            for (j, &b) in our_dofs.iter().enumerate() {
                for q in 0..m {
                    for c in 0..td {
                        u[j * rows + q * td + c] = tab.values[(q * dim + b) * td + c];
                    }
                }
            }
            let v = pack_nd_cols(&nd_data, nd_dofs, m, td);
            check_same_span(&u, &v, rows, k, RANK_TOL, SPAN_TOL)
                .unwrap_or_else(|msg| panic!("{key:?} closure ({d}, {e}) (m={m}, k={k}): {msg}"));
        }
    }
}

fn run_case(family: Family, cell: CellType, degree: u32) {
    let nd_cell = reference_cell_type(cell);
    match family {
        Family::N1curl => {
            let nd = nedelec::create::<f64, f64>(nd_cell, degree as usize, Continuity::Standard);
            check_element_spans(family, cell, degree, &nd);
        }
        Family::RaviartThomas => {
            let nd =
                raviart_thomas::create::<f64, f64>(nd_cell, degree as usize, Continuity::Standard);
            check_element_spans(family, cell, degree, &nd);
        }
        other => unreachable!("unexpected family {other:?}"),
    }
}

/// M12 acceptance A2: for N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra,
/// p = 1–8, the span of the basis functions associated with each sub-entity closure equals
/// ndelement's to 1e-10.
#[test]
#[ignore = "pending M12.T7"]
fn closure_spans_match_ndelement() {
    // Schedule heaviest cases (Hexahedron high degree) first so bounded parallelism stays balanced.
    let mut cases = Vec::with_capacity(CELLS.len() * 2 * MAX_DEGREE as usize);
    for degree in (1..=MAX_DEGREE).rev() {
        for cell in CELLS.into_iter().rev() {
            cases.push((Family::N1curl, cell, degree));
            cases.push((Family::RaviartThomas, cell, degree));
        }
    }
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().min(4))
        .unwrap_or(1)
        .min(cases.len());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (cases, next) = (&cases, &next);
                std::thread::Builder::new()
                    .stack_size(STACK_BYTES)
                    .spawn_scoped(s, move || {
                        while let Some(&(f, c, p)) = cases.get(next.fetch_add(1, Ordering::Relaxed))
                        {
                            run_case(f, c, p);
                        }
                    })
                    .unwrap()
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
}

#[test]
fn qr_span_helper_accepts_same_span_and_rejects_mismatches() {
    let mut rng = SplitMix64(0x5152_5445_5354_0001);
    let (rows, cols) = (40, 6);
    let u: Vec<f64> = (0..rows * cols)
        .map(|_| 2.0 * rng.next_unit() - 1.0)
        .collect();

    // Invertible basis change V = U * A (diagonally dominant A).
    let mut a = vec![0.0; cols * cols];
    for c in 0..cols {
        for r in 0..cols {
            a[c * cols + r] = 0.2 * (2.0 * rng.next_unit() - 1.0);
        }
        a[c * cols + c] += 3.0;
    }
    let mut v = vec![0.0; rows * cols];
    for j in 0..cols {
        for i in 0..rows {
            v[j * rows + i] = (0..cols).map(|k| u[k * rows + i] * a[j * cols + k]).sum();
        }
    }
    let (r1, r2) = check_same_span(&u, &v, rows, cols, RANK_TOL, SPAN_TOL).unwrap();
    assert!(r1 < 1e-13 && r2 < 1e-13, "residuals {r1:e}, {r2:e}");

    // Extreme column scaling must not trigger a false rank-deficiency failure.
    let mut v_scaled = v.clone();
    for x in &mut v_scaled[0..rows] {
        *x *= 1e-12;
    }
    let (rs1, rs2) = check_same_span(&u, &v_scaled, rows, cols, RANK_TOL, SPAN_TOL).unwrap();
    assert!(
        rs1 < 1e-13 && rs2 < 1e-13,
        "scaled residuals {rs1:e}, {rs2:e}"
    );

    let mut v_bad = v;
    v_bad[0] += 1e-6;
    assert!(check_same_span(&u, &v_bad, rows, cols, RANK_TOL, SPAN_TOL).is_err());

    let mut u_deg = u;
    for i in 0..rows {
        u_deg[(cols - 1) * rows + i] = u_deg[i];
    }
    assert!(check_same_span(&u_deg, &u_deg, rows, cols, RANK_TOL, SPAN_TOL).is_err());
}

#[test]
fn random_interior_points_lie_strictly_inside_each_cell() {
    let mut rng = SplitMix64(0x504f_494e_5453_0002);
    for cell in CELLS {
        let td = tdim(cell);
        let pts = random_interior_points(cell, 250, &mut rng);
        assert_eq!(pts.len(), 250 * td);
        for p in pts.chunks_exact(td) {
            assert!(p.iter().all(|&x| x > 0.0 && x < 1.0), "{cell:?}: {p:?}");
            if matches!(cell, CellType::Triangle | CellType::Tetrahedron) {
                assert!(p.iter().sum::<f64>() < 1.0, "{cell:?}: {p:?}");
            }
        }
    }
}

fn check_nd_counts_and_rank<E: FiniteElement<T = f64, TGeo = f64>>(
    family: Family,
    cell: CellType,
    degree: u32,
    nd_el: &E,
    counts: &serde_json::Value,
) {
    let td = tdim(cell);
    let rows_json = counts.as_array().unwrap();
    assert_eq!(rows_json.len(), td + 1);
    for (d, row) in rows_json.iter().enumerate() {
        let want: Vec<usize> = row
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        let got: Vec<usize> = (0..num_sub_entities(cell, d))
            .map(|e| nd_el.entity_dofs(d, e).unwrap().len())
            .collect();
        assert_eq!(got, want, "{family:?} {cell:?} p={degree} dim {d}");
    }
    let pts = random_interior_points(
        cell,
        MIN_POINTS,
        &mut SplitMix64(case_seed(family, cell, degree)),
    );
    let nd_data = tabulate_ndelement(nd_el, &pts, td);
    let cell_dofs = nd_el.entity_closure_dofs(td, 0).unwrap();
    let v = pack_nd_cols(&nd_data, cell_dofs, MIN_POINTS, td);
    let (_, _, ratio) = pivoted_qr(&v, MIN_POINTS * td, cell_dofs.len());
    assert!(
        ratio > RANK_TOL,
        "{family:?} {cell:?} p={degree}: ratio {ratio:e}"
    );
}

/// Verifies the Symfem DOF-count fixture agrees with ndelement on p = 1..=2 and that ndelement's
/// cell basis has full numerical rank at the sampled interior points.
#[test]
fn symfem_fixture_and_ndelement_rank_low_degree() {
    let path = format!(
        "{}/../tetraloma-element/tests/fixtures/hcurl_hdiv_dofs_symfem.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let symfem: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(move || {
            for cell in CELLS {
                let (nd_cell, cname) = (reference_cell_type(cell), format!("{cell:?}"));
                for degree in 1..=2u32 {
                    let n1 =
                        nedelec::create::<f64, f64>(nd_cell, degree as usize, Continuity::Standard);
                    check_nd_counts_and_rank(
                        Family::N1curl,
                        cell,
                        degree,
                        &n1,
                        &symfem["N1curl"][&cname][&degree.to_string()],
                    );
                    let rt = raviart_thomas::create::<f64, f64>(
                        nd_cell,
                        degree as usize,
                        Continuity::Standard,
                    );
                    check_nd_counts_and_rank(
                        Family::RaviartThomas,
                        cell,
                        degree,
                        &rt,
                        &symfem["RaviartThomas"][&cname][&degree.to_string()],
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

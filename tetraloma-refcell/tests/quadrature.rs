//! M1b acceptance: quadrature integrates monomials (rationomials on pyramids) up to its degree
//! to 1e-13, relative to the exact integral (docs/plans/M1b.md, questions 10 and 11).

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    QuadratureRule, QuadratureType, gauss_rule, polyset, quadrature_rule, tdim, vertices,
};

const CELLS: [CellType; 7] = [
    CellType::Interval,
    CellType::Triangle,
    CellType::Quadrilateral,
    CellType::Tetrahedron,
    CellType::Hexahedron,
    CellType::Prism,
    CellType::Pyramid,
];

const TOL: f64 = 1e-13;

fn factorial(n: u32) -> f64 {
    (1..=n).map(f64::from).product()
}

/// ∫ x^a y^b over the reference triangle: a! b! / (a + b + 2)!.
fn triangle_integral(a: u32, b: u32) -> f64 {
    factorial(a) * factorial(b) / factorial(a + b + 2)
}

/// A function of a point in reference coordinates.
type PointFn = Box<dyn Fn(&[f64]) -> f64>;

/// One test function on a cell: its values at a point and its exact integral.
struct Integrand {
    label: String,
    f: PointFn,
    exact: f64,
}

fn monomial(label: String, e: [u32; 3], exact: f64) -> Integrand {
    let f = move |x: &[f64]| x.iter().zip(e).map(|(xi, ei)| xi.powi(ei as i32)).product();
    Integrand {
        label,
        f: Box::new(f),
        exact,
    }
}

/// The integrands a rule of degree n must integrate exactly on `cell`:
/// - interval, quadrilateral, hexahedron: x^a y^b z^c with every exponent ≤ n (tensor rules);
/// - triangle, tetrahedron: total degree ≤ n;
/// - prism: a + b ≤ n and c ≤ n (triangle rule times interval rule);
/// - pyramid: the rationomials ξ^a η^b (1−z)^max(a,b) z^c with max(a,b) + c ≤ n, where
///   ξ = x/(1−z) and η = y/(1−z), i.e. x^a y^b z^c (1−z)^(max(a,b) − a − b) (question 11).
fn integrands(cell: CellType, n: u32) -> Vec<Integrand> {
    let mut out = Vec::new();
    match cell {
        CellType::Interval => {
            for a in 0..=n {
                out.push(monomial(
                    format!("x^{a}"),
                    [a, 0, 0],
                    1.0 / f64::from(a + 1),
                ));
            }
        }
        CellType::Quadrilateral => {
            for b in 0..=n {
                for a in 0..=n {
                    let exact = 1.0 / f64::from((a + 1) * (b + 1));
                    out.push(monomial(format!("x^{a} y^{b}"), [a, b, 0], exact));
                }
            }
        }
        CellType::Hexahedron => {
            for c in 0..=n {
                for b in 0..=n {
                    for a in 0..=n {
                        let exact = 1.0 / f64::from((a + 1) * (b + 1) * (c + 1));
                        out.push(monomial(format!("x^{a} y^{b} z^{c}"), [a, b, c], exact));
                    }
                }
            }
        }
        CellType::Triangle => {
            for b in 0..=n {
                for a in 0..=n - b {
                    out.push(monomial(
                        format!("x^{a} y^{b}"),
                        [a, b, 0],
                        triangle_integral(a, b),
                    ));
                }
            }
        }
        CellType::Tetrahedron => {
            for c in 0..=n {
                for b in 0..=n - c {
                    for a in 0..=n - c - b {
                        // a! b! c! / (a + b + c + 3)!
                        let exact =
                            factorial(a) * factorial(b) * factorial(c) / factorial(a + b + c + 3);
                        out.push(monomial(format!("x^{a} y^{b} z^{c}"), [a, b, c], exact));
                    }
                }
            }
        }
        CellType::Prism => {
            for c in 0..=n {
                for b in 0..=n {
                    for a in 0..=n - b {
                        let exact = triangle_integral(a, b) / f64::from(c + 1);
                        out.push(monomial(format!("x^{a} y^{b} z^{c}"), [a, b, c], exact));
                    }
                }
            }
        }
        CellType::Pyramid => {
            for c in 0..=n {
                for b in 0..=n - c {
                    for a in 0..=n - c {
                        let m = a.max(b);
                        if m + c > n {
                            continue;
                        }
                        // ∫∫∫ ξ^a η^b (1−z)^m z^c (1−z)^2 dξ dη dz
                        //   = 1/((a+1)(b+1)) · c! (m+2)! / (c+m+3)!
                        let exact = factorial(c) * factorial(m + 2)
                            / factorial(c + m + 3)
                            / f64::from((a + 1) * (b + 1));
                        let f = move |x: &[f64]| {
                            let s = 1.0 - x[2];
                            let (xi, eta) = (x[0] / s, x[1] / s);
                            xi.powi(a as i32)
                                * eta.powi(b as i32)
                                * s.powi(m as i32)
                                * x[2].powi(c as i32)
                        };
                        out.push(Integrand {
                            label: format!("xi^{a} eta^{b} (1-z)^{m} z^{c}"),
                            f: Box::new(f),
                            exact,
                        });
                    }
                }
            }
        }
        CellType::Point => unreachable!("the point cell has no monomials"),
    }
    out
}

#[test]
fn integrates_monomials() {
    for cell in CELLS {
        let td = tdim(cell);
        for n in 0..=20 {
            let rule = gauss_rule(cell, n);
            assert_eq!(rule.cell, cell);
            assert_eq!(
                rule.points.len(),
                rule.weights.len() * td,
                "{cell:?}, degree {n}"
            );
            for g in integrands(cell, n) {
                let approx: f64 = rule
                    .points
                    .chunks_exact(td)
                    .zip(&rule.weights)
                    .map(|(x, w)| w * (g.f)(x))
                    .sum();
                let err = (approx - g.exact).abs() / g.exact;
                assert!(
                    err <= TOL,
                    "{cell:?}, degree {n}, {}: {approx} vs {} (relative error {err:e})",
                    g.label,
                    g.exact
                );
            }
        }
    }
}

const ALL_CELLS: [CellType; 8] = [
    CellType::Point,
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
        CellType::Point | CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron => {
            1.0
        }
        CellType::Triangle | CellType::Prism => 0.5,
        CellType::Tetrahedron => 1.0 / 6.0,
        CellType::Pyramid => 1.0 / 3.0,
    }
}

fn strictly_inside(cell: CellType, p: &[f64]) -> bool {
    let unit = |t: f64| 0.0 < t && t < 1.0;
    match cell {
        CellType::Point => p.is_empty(),
        CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron => {
            p.iter().all(|&t| unit(t))
        }
        CellType::Triangle => p[0] > 0.0 && p[1] > 0.0 && p[0] + p[1] < 1.0,
        CellType::Tetrahedron => p.iter().all(|&t| t > 0.0) && p[0] + p[1] + p[2] < 1.0,
        CellType::Prism => p[0] > 0.0 && p[1] > 0.0 && p[0] + p[1] < 1.0 && unit(p[2]),
        CellType::Pyramid => p[0] > 0.0 && p[1] > 0.0 && p[2] > 0.0 && p[0].max(p[1]) + p[2] < 1.0,
    }
}

fn check_polyset_direct(cell: CellType, degree: u32, kind: QuadratureType, rule: &QuadratureRule) {
    let td = tdim(cell);
    assert_eq!((rule.cell, rule.degree), (cell, degree));
    assert_eq!(rule.points.len(), rule.weights.len() * td);
    let nb = polyset::dim(cell, degree);
    let sqrt_vol = cell_volume(cell).sqrt();

    if td == 0 {
        assert_eq!(rule.weights.len(), 1);
        let table = polyset::tabulate(cell, degree, &[], 0);
        assert_eq!(table.len(), nb);
        let err = (rule.weights[0] * table[0] - sqrt_vol).abs() / sqrt_vol;
        assert!(err <= 1e-14, "{kind:?} Point d={degree}: rel err {err:e}");
        return;
    }

    let npts = rule.weights.len();
    let chunk_pts = (1_000_000 / nb.max(1)).clamp(1, 512);
    let mut integrals = vec![0.0_f64; nb];
    for start in (0..npts).step_by(chunk_pts) {
        let end = (start + chunk_pts).min(npts);
        let m = end - start;
        let table = polyset::tabulate(cell, degree, &rule.points[start * td..end * td], 0);
        assert_eq!(table.len(), nb * m);
        for (k, acc) in integrals.iter_mut().enumerate() {
            let row = &table[k * m..(k + 1) * m];
            *acc += row
                .iter()
                .zip(&rule.weights[start..end])
                .map(|(&v, &w)| w * v)
                .sum::<f64>();
        }
    }

    for (k, &approx) in integrals.iter().enumerate() {
        let err = if k == 0 {
            (approx - sqrt_vol).abs()
        } else {
            approx.abs()
        } / sqrt_vol;
        assert!(
            err <= 1e-14,
            "{kind:?} {cell:?} d={degree} basis {k}: {approx} (rel {err:e})"
        );
    }
}

fn check_tensor_product(cell: CellType, degree: u32, kind: QuadratureType, rule: &QuadratureRule) {
    let td = tdim(cell);
    let r1 = quadrature_rule(CellType::Interval, degree, kind)
        .unwrap_or_else(|e| panic!("Interval {kind:?} d={degree}: {e}"));
    let n1 = r1.weights.len();
    let total = n1.pow(td as u32);
    assert_eq!(rule.weights.len(), total, "{kind:?} {cell:?} d={degree}");
    assert_eq!(
        rule.points.len(),
        total * td,
        "{kind:?} {cell:?} d={degree}"
    );

    let mut idx = vec![0usize; td];
    for q in 0..total {
        let mut w_want = 1.0;
        for (d, &i) in idx.iter().enumerate() {
            assert_eq!(
                rule.points[q * td + d].to_bits(),
                r1.points[i].to_bits(),
                "{kind:?} {cell:?} d={degree} q={q} axis={d}"
            );
            w_want *= r1.weights[i];
        }
        assert!(
            (rule.weights[q] - w_want).abs() <= 1e-15,
            "{kind:?} {cell:?} d={degree} q={q}: w={} vs {w_want}",
            rule.weights[q]
        );
        for i in &mut idx {
            *i += 1;
            if *i < n1 {
                break;
            }
            *i = 0;
        }
    }
}

fn check_rule_integration(
    cell: CellType,
    degree: u32,
    kind: QuadratureType,
    rule: &QuadratureRule,
) {
    match cell {
        CellType::Quadrilateral => {
            check_tensor_product(cell, degree, kind, rule);
            if degree <= 20 {
                check_polyset_direct(cell, degree, kind, rule);
            }
        }
        CellType::Hexahedron => {
            check_tensor_product(cell, degree, kind, rule);
            if degree <= 10 {
                check_polyset_direct(cell, degree, kind, rule);
            }
        }
        _ => check_polyset_direct(cell, degree, kind, rule),
    }
}

/// M13 acceptance A1: every rule of every `QuadratureType` integrates the orthonormal set of its
/// degree with relative error at most 10^-14 (`Symmetric` 1-30, `Gauss`/`Default` 0-40,
/// `GaussLobatto` 2-41 points per direction).
#[test]
#[ignore = "pending M13.T2"]
fn every_rule_integrates_polyset() {
    for cell in [CellType::Point, CellType::Triangle, CellType::Tetrahedron] {
        for degree in 1..=30 {
            let rule = quadrature_rule(cell, degree, QuadratureType::Symmetric)
                .unwrap_or_else(|e| panic!("Symmetric {cell:?} d={degree}: {e}"));
            check_polyset_direct(cell, degree, QuadratureType::Symmetric, &rule);
        }
    }

    for kind in [QuadratureType::Gauss, QuadratureType::Default] {
        for cell in ALL_CELLS {
            for degree in 0..=40 {
                let rule = quadrature_rule(cell, degree, kind)
                    .unwrap_or_else(|e| panic!("{kind:?} {cell:?} d={degree}: {e}"));
                check_rule_integration(cell, degree, kind, &rule);
            }
        }
    }

    let gll_degrees: Vec<u32> = std::iter::once(0)
        .chain((2u32..=41).map(|n| 2 * n - 3))
        .collect();
    for cell in [
        CellType::Point,
        CellType::Interval,
        CellType::Quadrilateral,
        CellType::Hexahedron,
    ] {
        for &degree in &gll_degrees {
            let rule = quadrature_rule(cell, degree, QuadratureType::GaussLobatto)
                .unwrap_or_else(|e| panic!("GaussLobatto {cell:?} d={degree}: {e}"));
            let n1 = (degree + 3).div_ceil(2) as usize;
            assert_eq!(
                rule.weights.len(),
                n1.pow(tdim(cell) as u32),
                "GaussLobatto {cell:?} d={degree}"
            );
            check_rule_integration(cell, degree, QuadratureType::GaussLobatto, &rule);
        }
    }
}

/// M13 acceptance A2: every point of every `Gauss`, `Symmetric` and `Default` rule lies strictly
/// inside its cell, and every weight is positive.
#[test]
#[ignore = "pending M13.T2"]
fn gauss_and_symmetric_points_interior_and_weights_positive() {
    let check_rule = |cell: CellType, degree: u32, kind: QuadratureType| {
        let rule = quadrature_rule(cell, degree, kind)
            .unwrap_or_else(|e| panic!("{kind:?} {cell:?} d={degree}: {e}"));
        let td = tdim(cell);
        assert_eq!((rule.cell, rule.degree), (cell, degree));
        assert!(!rule.weights.is_empty(), "{kind:?} {cell:?} d={degree}");
        assert_eq!(rule.points.len(), rule.weights.len() * td);
        for (q, &w) in rule.weights.iter().enumerate() {
            assert!(
                w.is_finite() && w > 0.0,
                "{kind:?} {cell:?} d={degree} q={q}: w={w}"
            );
        }
        if td == 0 {
            assert!(rule.points.is_empty() && rule.weights == [1.0]);
        } else {
            for (q, p) in rule.points.chunks_exact(td).enumerate() {
                assert!(
                    p.iter().all(|c| c.is_finite()) && strictly_inside(cell, p),
                    "{kind:?} {cell:?} d={degree} q={q}: {p:?} not strictly interior"
                );
            }
        }
    };

    for cell in ALL_CELLS {
        for degree in 0..=60 {
            check_rule(cell, degree, QuadratureType::Gauss);
            check_rule(cell, degree, QuadratureType::Default);
        }
    }
    for cell in [CellType::Point, CellType::Triangle, CellType::Tetrahedron] {
        for degree in 1..=30 {
            check_rule(cell, degree, QuadratureType::Symmetric);
        }
    }
}

fn all_permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for rest in all_permutations(n - 1) {
        for pos in 0..n {
            let mut p = rest.clone();
            p.insert(pos, n - 1);
            out.push(p);
        }
    }
    out
}

/// M13 acceptance A3: every `Symmetric` rule on triangles and tetrahedra maps onto itself under
/// every vertex permutation of the cell, with points and weights equal to 10^-15.
#[test]
#[ignore = "pending M13.T2"]
fn symmetric_rules_invariant_under_vertex_permutations() {
    for cell in [CellType::Triangle, CellType::Tetrahedron] {
        let (td, verts) = (tdim(cell), vertices(cell));
        let nv = verts.len();
        let perms = all_permutations(nv);
        assert_eq!(perms.len(), if td == 2 { 6 } else { 24 });

        for degree in 1..=30 {
            let rule = quadrature_rule(cell, degree, QuadratureType::Symmetric)
                .unwrap_or_else(|e| panic!("Symmetric {cell:?} d={degree}: {e}"));
            let npts = rule.weights.len();
            let (mut used, mut bary, mut mapped) =
                (vec![false; npts], vec![0.0; nv], vec![0.0; td]);

            for perm in &perms {
                used.fill(false);
                for q in 0..npts {
                    let x = &rule.points[q * td..(q + 1) * td];
                    bary[0] = 1.0 - x.iter().sum::<f64>();
                    bary[1..nv].copy_from_slice(x);
                    mapped.fill(0.0);
                    for (k, &lam) in bary.iter().enumerate() {
                        for c in 0..td {
                            mapped[c] += lam * verts[perm[k]][c];
                        }
                    }

                    let (m, best_dist) = used
                        .iter()
                        .enumerate()
                        .filter(|&(_, &u)| !u)
                        .map(|(m, _)| {
                            let y = &rule.points[m * td..(m + 1) * td];
                            let d = mapped
                                .iter()
                                .zip(y)
                                .map(|(a, b)| (a - b).abs())
                                .fold(0.0, f64::max);
                            (m, d)
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .expect("unused rule point");
                    assert!(
                        best_dist <= 1e-15,
                        "{cell:?} d={degree} perm={perm:?} q={q}: pt err {best_dist:e}"
                    );
                    let w_err = (rule.weights[q] - rule.weights[m]).abs();
                    assert!(
                        w_err <= 1e-15,
                        "{cell:?} d={degree} perm={perm:?} q={q}->{m}: w err {w_err:e}"
                    );
                    used[m] = true;
                }
            }
        }
    }
}

/// M13 acceptance A4: for every degree 4-30, the `Default` rule on triangles and on tetrahedra
/// has strictly fewer points than the `Gauss` rule.
#[test]
#[ignore = "pending M13.T2"]
fn default_rule_has_fewer_points_than_gauss() {
    for cell in [CellType::Triangle, CellType::Tetrahedron] {
        for degree in 4..=30 {
            let def = quadrature_rule(cell, degree, QuadratureType::Default)
                .unwrap_or_else(|e| panic!("Default {cell:?} d={degree}: {e}"));
            let gauss = quadrature_rule(cell, degree, QuadratureType::Gauss)
                .unwrap_or_else(|e| panic!("Gauss {cell:?} d={degree}: {e}"));
            assert!(
                def.weights.len() < gauss.weights.len(),
                "{cell:?} d={degree}: Default {} vs Gauss {}",
                def.weights.len(),
                gauss.weights.len()
            );
        }
    }
}

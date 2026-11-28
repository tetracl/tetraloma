//! M13 acceptance A5: the mass matrix of the `Nodal(Gll)` Lagrange element of degree n - 1
//! under the n-point `GaussLobatto` rule is diagonal to 10^-14 for n = 2..=17 on intervals,
//! quadrilaterals and hexahedra.

use tetraloma_dims::CellType;
use tetraloma_element::{Element, ElementKey, Family, Variant};
use tetraloma_refcell::{NodeFamily, QuadratureType, quadrature_rule, tdim};

const TOL: f64 = 1e-14;

/// Maps each DOF `i` (at `interp_pts[i]`) to the unique quadrature point index `q_of[i]` in
/// `rule_pts`, asserting a 1-to-1 coordinate match within `1e-13`.
fn match_dofs_to_rule_points(
    cell: CellType,
    n: u32,
    interp_pts: &[f64],
    rule_pts: &[f64],
) -> Vec<usize> {
    let td = tdim(cell);
    let dim = interp_pts.len() / td;
    assert_eq!(rule_pts.len(), dim * td);
    let mut used = vec![false; dim];
    (0..dim)
        .map(|i| {
            let node = &interp_pts[i * td..(i + 1) * td];
            let (q, best_dist) = used
                .iter()
                .enumerate()
                .filter(|&(_, &u)| !u)
                .map(|(q, _)| {
                    let qp = &rule_pts[q * td..(q + 1) * td];
                    let d = node
                        .iter()
                        .zip(qp)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max);
                    (q, d)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("every GLL node matches a unique GLL quadrature point");
            assert!(
                best_dist <= 1e-13,
                "{cell:?} n={n} dof {i}: nearest pt {q} dist {best_dist:e}"
            );
            used[q] = true;
            q
        })
        .collect()
}

/// M13 acceptance A5: the mass matrix of `Nodal(Gll)` Lagrange of degree n - 1 under the
/// n-point `GaussLobatto` rule is diagonal to 10^-14 for n = 2..=17 on intervals,
/// quadrilaterals and hexahedra.
#[test]
#[ignore = "pending M13.T2"]
fn gll_mass_matrix_is_diagonal() {
    for (cell, direct_max_n) in [
        (CellType::Interval, 17u32),
        (CellType::Quadrilateral, 17u32),
        (CellType::Hexahedron, 8u32),
    ] {
        let td = tdim(cell);
        for n in 2..=17u32 {
            let el = Element::get(ElementKey {
                family: Family::Lagrange,
                cell,
                degree: n - 1,
                variant: Variant::Nodal(NodeFamily::Gll),
            })
            .unwrap_or_else(|e| panic!("Element::get({cell:?}, p={}): {e}", n - 1));
            let rule = quadrature_rule(cell, 2 * n - 3, QuadratureType::GaussLobatto)
                .unwrap_or_else(|e| panic!("quadrature_rule({cell:?}, n={n}): {e}"));

            let dim = el.dim();
            assert_eq!(dim, (n as usize).pow(td as u32), "{cell:?} n={n}");
            assert_eq!(rule.weights.len(), dim, "{cell:?} n={n}");

            let q_of = match_dofs_to_rule_points(cell, n, el.interpolation_points(), &rule.points);
            let tab = el
                .tabulate(&rule.points, 0)
                .unwrap_or_else(|e| panic!("tabulate({cell:?}, n={n}): {e}"));
            assert_eq!(tab.values.len(), dim * dim);

            let (mut diag, mut off_norm) = (vec![0.0_f64; dim], vec![0.0_f64; dim]);
            for i in 0..dim {
                let qi = q_of[i];
                let (mut d_ii, mut r_sq) = (0.0, 0.0);
                for (q, &w) in rule.weights.iter().enumerate() {
                    let v = tab.values[q * dim + i];
                    let term = w * v * v;
                    d_ii += term;
                    if q != qi {
                        r_sq += term;
                    }
                }
                (diag[i], off_norm[i]) = (d_ii, r_sq.sqrt());
                let w_i = rule.weights[qi];
                let err = (d_ii - w_i).abs();
                assert!(
                    err <= TOL,
                    "{cell:?} n={n} diag[{i}]: {d_ii} vs {w_i} (abs {err:e})"
                );
            }

            for i in 0..dim {
                let (qi, ri) = (q_of[i], off_norm[i]);
                let wi_phi_i = rule.weights[qi] * tab.values[qi * dim + i];
                for j in (i + 1)..dim {
                    let bound = if n <= direct_max_n {
                        rule.weights
                            .iter()
                            .enumerate()
                            .map(|(q, &w)| w * tab.values[q * dim + i] * tab.values[q * dim + j])
                            .sum::<f64>()
                            .abs()
                    } else {
                        // Rigorous O(N^2) Cauchy-Schwarz upper bound on |M_{ij}| for Hex n=9..=17.
                        let qj = q_of[j];
                        let t_i = (wi_phi_i * tab.values[qi * dim + j]).abs();
                        let t_j = (rule.weights[qj]
                            * tab.values[qj * dim + i]
                            * tab.values[qj * dim + j])
                            .abs();
                        t_i + t_j + ri * off_norm[j]
                    };
                    let rel = bound / diag[i].max(diag[j]);
                    assert!(
                        bound <= TOL && rel <= TOL,
                        "{cell:?} n={n} |M[{i},{j}]| = {bound:e} (rel {rel:e})"
                    );
                }
            }
        }
    }
}

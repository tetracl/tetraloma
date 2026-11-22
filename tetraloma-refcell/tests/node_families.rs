//! M11 acceptance tests for GLL node families on triangles, tetrahedra and prisms
//! (docs/plans/M11.md: A4 and A5).

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    LagrangeElement, NodeFamily, lagrange_entity_nodes, lagrange_points, sub_entities,
    sub_entity_types, tdim, vertices,
};

fn map_sub_entity_points(cell: CellType, ety: CellType, everts: &[usize], pts: &[f64]) -> Vec<f64> {
    let (td, ed) = (tdim(cell), tdim(ety));
    let verts = vertices(cell);
    let v0 = verts[everts[0]];
    if ed == 0 {
        return v0.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() / ed * td);
    for t in pts.chunks_exact(ed) {
        for i in 0..td {
            let delta: f64 = (0..ed)
                .map(|k| t[k] * (verts[everts[k + 1]][i] - v0[i]))
                .sum();
            out.push(v0[i] + delta);
        }
    }
    out
}

/// M11 acceptance A4: on every sub-entity of a triangle, tetrahedron and prism, the GLL points
/// restricted to it equal that sub-entity's GLL points to 1e-15 for p = 1–15.
#[test]
#[ignore = "pending M11.T2"]
fn gll_restricts_to_sub_entities() {
    for cell in [CellType::Triangle, CellType::Tetrahedron, CellType::Prism] {
        let td = tdim(cell);
        for p in 1..=15 {
            let cell_pts = lagrange_points(cell, p, NodeFamily::Gll).unwrap();
            for (v, &vcoords) in vertices(cell).iter().enumerate() {
                for c in 0..td {
                    assert!((cell_pts[v * td + c] - vcoords[c]).abs() <= 1e-15);
                }
            }
            for dim in 1..td {
                for (e, (&everts, &ety)) in sub_entities(cell, dim)
                    .iter()
                    .zip(sub_entity_types(cell, dim))
                    .enumerate()
                {
                    let sub_pts = lagrange_points(ety, p, NodeFamily::Gll).unwrap();
                    let mapped = map_sub_entity_points(cell, ety, everts, &sub_pts);

                    // Interior nodes of `(dim, e)` on `cell` match `ety`'s interior nodes in order.
                    let cell_range = lagrange_entity_nodes(cell, p, dim, e);
                    let sub_range = lagrange_entity_nodes(ety, p, dim, 0);
                    assert_eq!(
                        cell_range.len(),
                        sub_range.len(),
                        "{cell:?} p={p} dim={dim} e={e}"
                    );
                    for (ck, sk) in cell_range.zip(sub_range) {
                        for c in 0..td {
                            let (a, b) = (cell_pts[ck * td + c], mapped[sk * td + c]);
                            assert!(
                                (a - b).abs() <= 1e-15,
                                "{cell:?} p={p} dim={dim} e={e} node ({ck}, {sk}) coord {c}: {a} vs {b}"
                            );
                        }
                    }
                    // Every point of `ety`'s full GLL point set matches a GLL node of `cell`.
                    for (sk, mp) in mapped.chunks_exact(td).enumerate() {
                        let min_dist = cell_pts
                            .chunks_exact(td)
                            .map(|cp| {
                                cp.iter()
                                    .zip(mp)
                                    .map(|(a, b)| (a - b).abs())
                                    .fold(0.0_f64, f64::max)
                            })
                            .fold(f64::INFINITY, f64::min);
                        assert!(
                            min_dist <= 1e-15,
                            "{cell:?} p={p} dim={dim} e={e} closure pt {sk}: min dist {min_dist:e}"
                        );
                    }
                }
            }
        }
    }
}

/// Deterministic sample grid of at least 10^4 points on the reference triangle (10 011 points)
/// or tetrahedron (10 660 points).
fn sample_grid(cell: CellType) -> Vec<f64> {
    let mut pts = Vec::new();
    match cell {
        CellType::Triangle => {
            let m = 140usize;
            for j in 0..=m {
                for i in 0..=(m - j) {
                    pts.extend_from_slice(&[i as f64 / m as f64, j as f64 / m as f64]);
                }
            }
        }
        CellType::Tetrahedron => {
            let m = 38usize;
            for k in 0..=m {
                for j in 0..=(m - k) {
                    for i in 0..=(m - j - k) {
                        pts.extend_from_slice(&[
                            i as f64 / m as f64,
                            j as f64 / m as f64,
                            k as f64 / m as f64,
                        ]);
                    }
                }
            }
        }
        other => panic!("sample_grid: unexpected cell {other:?}"),
    }
    assert!(pts.len() / tdim(cell) >= 10_000);
    pts
}

fn lebesgue_constant(el: &LagrangeElement, points: &[f64]) -> f64 {
    let td = tdim(el.cell());
    let mut max_sum = 0.0_f64;
    for chunk in points.chunks(1024 * td) {
        let tab = el.tabulate(chunk, 0).unwrap();
        for q in 0..tab.num_points {
            let sum: f64 = (0..tab.num_basis).map(|k| tab.get(0, q, k).abs()).sum();
            max_sum = max_sum.max(sum);
        }
    }
    max_sum
}

/// M11 acceptance A5: the Lebesgue constant of GLL points on triangles and tetrahedra, on a
/// sample grid of 10^4 points, is below that of equispaced points for every p = 3–15.
#[test]
#[ignore = "pending M11.T2"]
fn gll_lebesgue_constant_below_equispaced() {
    for cell in [CellType::Triangle, CellType::Tetrahedron] {
        let grid = sample_grid(cell);
        for p in 3..=15 {
            let gll = LagrangeElement::new(cell, p, NodeFamily::Gll).unwrap();
            let eq = LagrangeElement::new(cell, p, NodeFamily::Equispaced).unwrap();
            let lg = lebesgue_constant(&gll, &grid);
            let le = lebesgue_constant(&eq, &grid);
            assert!(
                lg < le,
                "{cell:?} p={p}: Lebesgue GLL {lg:.6} >= Equispaced {le:.6}"
            );
        }
    }
}

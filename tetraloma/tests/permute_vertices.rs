//! M13 acceptance A8: `permute_vertices` agrees with `physical_points_block` of a cell whose
//! vertices (and corresponding Lagrange nodes) are permuted, for every cell type of dimension
//! 1–3 and geometry degrees 1–3, in `f32` (`10^-5`) and, where supported, `f64` (`10^-12`).

use crate::common::{client, tol};

use cubecl::prelude::Client;
use tetraloma::{PointSet, physical_points_block, prelude::*};
use tetraloma_refcell::{lagrange_points, permute_vertices, vertices};
use tetraloma_testing::facet_rule::{cell_symmetries, interior_points};

fn curved_map(x: &[f64]) -> Vec<f64> {
    let c = [0.15, 0.07, 0.03];
    (0..x.len())
        .map(|i| {
            let (xi, xj) = (x[i], x[(i + 1) % x.len()]);
            (1.0 + c[i]) * xi
                + 0.06 * xj
                + 0.08 * xi * (1.0 - xi)
                + 0.03 * xj * xj
                + 0.02 * xi.powi(3)
        })
        .collect()
}

/// Independent affine map `Φ(v̂_k) = v̂_{perm[k]}` for a combinatorial symmetry `perm` of `cell`,
/// using the origin vertex `0` and the axis vertices along e₀, …, e_{d−1}.
fn affine_symmetry(cell: CellType, perm: &[usize], xi: &[f64]) -> Vec<f64> {
    let verts = vertices(cell);
    let axes: &[usize] = match cell {
        CellType::Hexahedron | CellType::Pyramid => &[1, 2, 4],
        _ => &[1, 2, 3][..xi.len()],
    };
    let v0 = verts[perm[0]];
    (0..xi.len())
        .map(|i| {
            v0[i]
                + axes
                    .iter()
                    .enumerate()
                    .map(|(k, &a)| xi[k] * (verts[perm[a]][i] - v0[i]))
                    .sum::<f64>()
        })
        .collect()
}

fn check_cell<F: Real, TD: TopoDim + EmbeddedIn<TD>>(client: &Client, cell: TD::Cell) {
    let ct: CellType = cell.into();
    let (td, syms, pts, t) = (TD::N, cell_symmetries(ct), interior_points(ct), tol::<F>());
    let nq = pts.len() / td;

    for degree in 1..=3u32 {
        let ref_nodes = lagrange_points(ct, degree, NodeFamily::Equispaced).unwrap();
        let n_nodes = ref_nodes.len() / td;
        let phys_nodes: Vec<f64> = ref_nodes.chunks_exact(td).flat_map(curved_map).collect();
        let orig_conn: Vec<u32> = (0..n_nodes as u32).collect();
        let perm_conn: Vec<u32> = syms
            .iter()
            .flat_map(|sym| {
                ref_nodes.chunks_exact(td).map(|node| {
                    let m = affine_symmetry(ct, sym, node);
                    ref_nodes
                        .chunks_exact(td)
                        .position(|c| c.iter().zip(&m).all(|(&a, &b)| (a - b).abs() < 1e-12))
                        .unwrap() as u32
                })
            })
            .collect();

        let elem = CoordinateElement::<TD> {
            cell,
            degree,
            family: NodeFamily::Equispaced,
        };
        let build = |conn: &[u32]| -> Mesh<F, TD, TD> {
            MeshBuilder::new(client)
                .nodes(&phys_nodes, None)
                .cell_block(elem, conn, None)
                .build()
                .unwrap()
        };
        let (mesh_orig, mesh_perm) = (build(&orig_conn), build(&perm_conn));

        let pts_orig =
            PointSet::<F, TD>::from_points(client, cell, pts.clone(), Vec::new()).unwrap();
        let mapped: Vec<f64> = syms
            .iter()
            .flat_map(|sym| permute_vertices(ct, sym, &pts).unwrap())
            .collect();
        let pts_mapped = PointSet::<F, TD>::from_points(client, cell, mapped, Vec::new()).unwrap();

        let got_perm = physical_points_block(&mesh_perm, 0, &pts_orig)
            .unwrap()
            .to_host_cell_major()
            .unwrap();
        let got_orig = physical_points_block(&mesh_orig, 0, &pts_mapped)
            .unwrap()
            .to_host_cell_major()
            .unwrap();
        assert_eq!(
            (got_perm.len(), got_orig.len()),
            (syms.len() * nq, syms.len() * nq)
        );

        for (s, sym) in syms.iter().enumerate() {
            for q in 0..nq {
                let (p, o) = (got_perm[s * nq + q], got_orig[s * nq + q]);
                for (g, (&a, &b)) in p.as_ref().iter().zip(o.as_ref()).enumerate() {
                    let (a, b) = (a.widen(), b.widen());
                    assert!(
                        (a - b).abs() <= t * a.abs().max(b.abs()).max(1.0),
                        "{ct:?} p={degree} {sym:?} q={q} g={g}: {a} vs {b}"
                    );
                }
            }
        }
    }
}

fn check_all<F: Real>(client: &Client) {
    check_cell::<F, D1>(client, Cell1::Interval);
    check_cell::<F, D2>(client, Cell2::Triangle);
    check_cell::<F, D2>(client, Cell2::Quadrilateral);
    check_cell::<F, D3>(client, Cell3::Tetrahedron);
    check_cell::<F, D3>(client, Cell3::Hexahedron);
    check_cell::<F, D3>(client, Cell3::Prism);
    check_cell::<F, D3>(client, Cell3::Pyramid);
}

#[test]
#[ignore = "pending M13.T5"]
fn matches_physical_points_of_permuted_cell() {
    for_each_precision!(client(), check_all);
}

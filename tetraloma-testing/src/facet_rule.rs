//! M13 acceptance A6: interior-facet point matching with `FacetQuadRule::new` and
//! `rule_permutation` on meshes with randomly permuted vertex orderings, generic over `Comm`.
//!
//! `tetraloma/tests/facet_rule_permutation.rs` runs [`check`] on [`ThreadComm`](crate::ThreadComm)
//! (1–4 ranks); `tetraloma-mpi-tests` runs it on `Mpi` (registered in M13.T4).

use cubecl::prelude::Client;
use tetraloma::{
    Cell3, Comm, CommExt, ConnKey, Connectivity, D2, D3, DeviceGrid, Dim, EmbeddedIn,
    FacetQuadRule, GhostMode, QuadratureType, Real, TopoDim, physical_points_block,
};
use tetraloma_dims::CellType;
use tetraloma_refcell::{num_sub_entities, quadrature_rule, rule_permutation, sub_entities};

use crate::{HostMesh, TOL_F32, TOL_F64, supports};

/// Three asymmetric interior reference points per cell, for M13 A7 and A8 tests.
pub fn interior_points(cell: CellType) -> Vec<f64> {
    use CellType::*;
    match cell {
        Point => Vec::new(),
        Interval => vec![0.13, 0.47, 0.89],
        Triangle => vec![0.11, 0.17, 0.53, 0.23, 0.19, 0.61],
        Quadrilateral => vec![0.13, 0.29, 0.71, 0.37, 0.43, 0.83],
        Tetrahedron => vec![0.11, 0.17, 0.23, 0.41, 0.19, 0.13, 0.15, 0.37, 0.29],
        Hexahedron => vec![0.13, 0.29, 0.41, 0.71, 0.37, 0.19, 0.43, 0.83, 0.67],
        Prism => vec![0.11, 0.17, 0.23, 0.53, 0.23, 0.61, 0.19, 0.47, 0.83],
        Pyramid => vec![0.11, 0.17, 0.23, 0.31, 0.29, 0.41, 0.13, 0.19, 0.67],
    }
}

/// Every combinatorial vertex symmetry (1-skeleton automorphism) of `cell`, in gather order
/// (`perm[k]` is the original vertex moved to local slot `k`).
pub fn cell_symmetries(cell: CellType) -> Vec<Vec<usize>> {
    if cell == CellType::Hexahedron {
        let mut out = Vec::with_capacity(48);
        for ax in cell_symmetries(CellType::Triangle) {
            for f in 0..8usize {
                let p = (0..8)
                    .map(|v| {
                        let b = [v & 1, (v >> 1) & 1, (v >> 2) & 1];
                        (b[ax[0]] ^ (f & 1))
                            | ((b[ax[1]] ^ ((f >> 1) & 1)) << 1)
                            | ((b[ax[2]] ^ ((f >> 2) & 1)) << 2)
                    })
                    .collect();
                out.push(p);
            }
        }
        return out;
    }
    let nv = tetraloma_refcell::vertices(cell).len();
    let edges: Vec<_> = sub_entities(cell, 1)
        .iter()
        .map(|e| (e[0].min(e[1]), e[0].max(e[1])))
        .collect();
    let (mut cur, mut out) = ((0..nv).collect::<Vec<_>>(), Vec::new());
    collect_syms(&mut cur, 0, &edges, &mut out);
    out
}

fn collect_syms(cur: &mut [usize], i: usize, edges: &[(usize, usize)], out: &mut Vec<Vec<usize>>) {
    if i == cur.len() {
        if edges
            .iter()
            .all(|&(u, v)| edges.contains(&(cur[u].min(cur[v]), cur[u].max(cur[v]))))
        {
            out.push(cur.to_vec());
        }
        return;
    }
    for j in i..cur.len() {
        cur.swap(i, j);
        collect_syms(cur, i + 1, edges, out);
        cur.swap(i, j);
    }
}

/// Randomises origin ids and permutes each P1 cell's local vertex order by a cell symmetry.
fn with_permuted_vertices(mesh: &HostMesh, mut s: u64) -> HostMesh {
    let mut out = mesh.with_shuffled_ids(s);
    for block in &mut out.blocks {
        assert_eq!(block.degree, 1, "expected P1 block");
        let (syms, nv) = (cell_symmetries(block.cell), block.nodes_per_cell());
        let mut orig = vec![0u32; nv];
        for row in block.cells.chunks_exact_mut(nv) {
            orig.copy_from_slice(row);
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = (s ^ (s >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            for (dst, &src) in row
                .iter_mut()
                .zip(&syms[(z ^ (z >> 31)) as usize % syms.len()])
            {
                *dst = orig[src];
            }
        }
    }
    out
}

fn check_mesh<F: Real, TD: TopoDim + EmbeddedIn<GD>, GD: Dim, C: Comm>(
    comm: &C,
    client: &Client,
    name: &str,
    host: &HostMesh,
) {
    let tol = if size_of::<F>() == 4 {
        TOL_F32
    } else {
        TOL_F64
    };
    let mut mesh = host
        .chunk(comm.rank(), comm.size())
        .builder::<F, TD, GD>(client)
        .unwrap()
        .ghosts(GhostMode::SharedFacet)
        .build_on(comm.clone())
        .unwrap();
    let td = TD::N;
    mesh.create_entities(td - 1).unwrap();
    mesh.create_connectivity(td - 1, td).unwrap();

    let (d, f) = (td as u8, (td - 1) as u8);
    let down: Vec<_> = match mesh.connectivity(ConnKey { from: d, to: f }).unwrap() {
        Connectivity::Downward(dr) => dr
            .iter()
            .map(|s| (s.stride as usize, s.data.to_host().unwrap()))
            .collect(),
        _ => unreachable!(),
    };
    let (up_off, up_idx) = match mesh.connectivity(ConnKey { from: f, to: d }).unwrap() {
        Connectivity::Upward(uc) => (uc.offsets.to_host().unwrap(), uc.indices.to_host().unwrap()),
        _ => unreachable!(),
    };

    let cbs = mesh.cell_blocks();
    let perms: Vec<_> = (0..cbs.len())
        .map(|b| mesh.permutation_info(b).unwrap().to_host().unwrap())
        .collect();
    let starts: Vec<usize> = core::iter::once(0)
        .chain(cbs.iter().scan(0, |acc, cb| {
            *acc += cb.index_map.num_local();
            Some(*acc)
        }))
        .collect();
    let cell_types: Vec<TD::Cell> = cbs.iter().map(|b| b.cell).collect();

    for degree in [3u32, 5] {
        let frule =
            FacetQuadRule::<F, TD>::new(client, &cell_types, degree, QuadratureType::Default)
                .unwrap();
        let phys: Vec<_> = (0..cbs.len())
            .map(|b| {
                let pts = frule.get(cbs[b].cell).unwrap().points();
                physical_points_block(&mesh, b, pts)
                    .unwrap()
                    .to_host_cell_major()
                    .unwrap()
            })
            .collect();

        let mut owned_interior = 0u64;
        for eb in mesh.entity_blocks(td - 1).unwrap() {
            let rrule = quadrature_rule(eb.cell, degree, QuadratureType::Default).unwrap();
            let nq = rrule.weights.len();
            for f_local in 0..eb.index_map.num_owned() {
                let f_flat = eb.offset as usize + f_local;
                let row = &up_idx[up_off[f_flat] as usize..up_off[f_flat + 1] as usize];
                assert!(row.len() == 1 || row.len() == 2, "{name}: facet {f_flat}");
                if row.len() != 2 {
                    continue;
                }
                owned_interior += 1;
                let side = |cf: usize| {
                    let b = starts.partition_point(|&s| s <= cf) - 1;
                    let c = cf - starts[b];
                    let (st, ref data) = down[b];
                    let lf = data[c * st..(c + 1) * st]
                        .iter()
                        .position(|&x| x as usize == f_flat)
                        .unwrap();
                    let ct: CellType = cbs[b].cell.into();
                    let bits = if td == 2 {
                        (perms[b][c] >> lf) & 1
                    } else {
                        (perms[b][c] >> (num_sub_entities(ct, 1) + 3 * lf)) & 0b111
                    };
                    let perm = rule_permutation(eb.cell, &rrule, bits).unwrap();
                    let fps = frule.get(cbs[b].cell).unwrap();
                    let s = fps.facet_offsets()[lf];
                    assert_eq!((fps.facet_offsets()[lf + 1] - s, perm.len()), (nq, nq));
                    (b, c, fps.points().len(), s, perm)
                };
                let (b0, c0, n0, s0, p0) = side(row[0] as usize);
                let (b1, c1, n1, s1, p1) = side(row[1] as usize);
                for q in 0..nq {
                    let (x0, x1) = (
                        phys[b0][c0 * n0 + s0 + p0[q] as usize],
                        phys[b1][c1 * n1 + s1 + p1[q] as usize],
                    );
                    for (g, (&a, &b)) in x0.as_ref().iter().zip(x1.as_ref()).enumerate() {
                        let d = (a.widen() - b.widen()).abs();
                        assert!(
                            d <= tol,
                            "{name} d={degree} f={f_flat} q={q} g={g}: {d:.3e}"
                        );
                    }
                }
            }
        }
        assert!(
            comm.sum(owned_interior) > 0,
            "{name} d={degree}: no interior facets"
        );
    }
}

/// Runs the A6 interior-facet point-matching check on `comm` across the tetrahedral box,
/// hexahedral box, mixed box and triangulated sphere in `f32` and (where supported) `f64`.
pub fn check<C: Comm>(comm: &C, client: &Client) {
    let tet = with_permuted_vertices(&crate::cube_box(Cell3::Tetrahedron, 2, 1), 11);
    let hex = with_permuted_vertices(&crate::cube_box(Cell3::Hexahedron, 2, 1), 23);
    let mixed = with_permuted_vertices(&crate::mixed_box(2, 1), 37);
    let sph = with_permuted_vertices(&crate::sphere(1, 1), 53);
    macro_rules! run {
        ($f:ty) => {
            check_mesh::<$f, D3, D3, C>(comm, client, "tet_box", &tet);
            check_mesh::<$f, D3, D3, C>(comm, client, "hex_box", &hex);
            check_mesh::<$f, D3, D3, C>(comm, client, "mixed_box", &mixed);
            check_mesh::<$f, D2, D3, C>(comm, client, "sphere", &sph);
        };
    }
    run!(f32);
    if supports::<f64>(client) {
        run!(f64);
    }
}

#[cfg(test)]
mod tests {
    use super::cell_symmetries;
    use tetraloma_dims::CellType::*;

    #[test]
    fn symmetry_group_orders_match_design() {
        let want = [
            (Point, 1),
            (Interval, 2),
            (Triangle, 6),
            (Quadrilateral, 8),
            (Tetrahedron, 24),
            (Hexahedron, 48),
            (Prism, 12),
            (Pyramid, 8),
        ];
        for (c, n) in want {
            let mut s = cell_symmetries(c);
            s.sort();
            s.dedup();
            assert_eq!(s.len(), n, "{c:?}");
        }
    }
}

//! M11 acceptance tests (docs/plans/M11.md: A6, A7, A8, A10 and A12) and M11 G8 high-degree
//! validation tests for tetraloma-element.

use std::collections::BTreeMap;

use tetraloma_dims::CellType;
use tetraloma_element::{
    BaseTransformation, Element, ElementError, ElementKey, Family, Generator, TransformationKind,
    Variant,
};
use tetraloma_refcell::{
    NodeFamily, canonical_order, gauss_rule, lagrange_entity_nodes, lagrange_points,
    num_sub_entities, orientation_bits, polyset, sub_entities, sub_entity_types, tdim, vertices,
};

const TOL: f64 = 1e-12;

const ALL_CELLS: [CellType; 7] = [
    CellType::Interval,
    CellType::Triangle,
    CellType::Quadrilateral,
    CellType::Tetrahedron,
    CellType::Hexahedron,
    CellType::Prism,
    CellType::Pyramid,
];

const ALL_VARIANTS: [Variant; 3] = [
    Variant::Nodal(NodeFamily::Equispaced),
    Variant::Nodal(NodeFamily::Gll),
    Variant::Legendre,
];

fn max_degree(cell: CellType) -> u32 {
    match cell {
        CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron => 20,
        CellType::Triangle | CellType::Tetrahedron | CellType::Prism => 15,
        CellType::Pyramid => 8,
        CellType::Point => unreachable!("no element on Point"),
    }
}

fn variants_for(cell: CellType) -> Vec<Variant> {
    ALL_VARIANTS
        .into_iter()
        .filter(|&v| !(cell == CellType::Pyramid && v == Variant::Nodal(NodeFamily::Gll)))
        .collect()
}

fn lagrange_key(cell: CellType, degree: u32, variant: Variant) -> ElementKey {
    ElementKey {
        family: Family::Lagrange,
        cell,
        degree,
        variant,
    }
}

/// M11 acceptance A12: degrees outside the table of section 26, and `Nodal(Gll)` on pyramids,
/// return `ElementError::Unsupported`.
#[test]
fn unsupported_keys_return_unsupported() {
    let mut keys = Vec::new();
    for cell in ALL_CELLS {
        for variant in ALL_VARIANTS {
            for degree in [max_degree(cell) + 1, u32::MAX] {
                keys.push(lagrange_key(cell, degree, variant));
            }
        }
    }
    for degree in 0..=9 {
        keys.push(lagrange_key(
            CellType::Pyramid,
            degree,
            Variant::Nodal(NodeFamily::Gll),
        ));
    }
    for variant in ALL_VARIANTS {
        for degree in [0, 1, 2] {
            keys.push(lagrange_key(CellType::Point, degree, variant));
        }
    }
    // Keys outside section 26's table for N1curl and RaviartThomas (unsupported cells, degree 0,
    // degrees above range, and nodal variants).
    for family in [Family::N1curl, Family::RaviartThomas] {
        for cell in [
            CellType::Point,
            CellType::Interval,
            CellType::Prism,
            CellType::Pyramid,
        ] {
            keys.push(ElementKey {
                family,
                cell,
                degree: 1,
                variant: Variant::Legendre,
            });
        }
        for cell in [
            CellType::Triangle,
            CellType::Quadrilateral,
            CellType::Tetrahedron,
            CellType::Hexahedron,
        ] {
            for degree in [0, max_degree(cell) + 1] {
                keys.push(ElementKey {
                    family,
                    cell,
                    degree,
                    variant: Variant::Legendre,
                });
            }
            for nf in [NodeFamily::Equispaced, NodeFamily::Gll] {
                keys.push(ElementKey {
                    family,
                    cell,
                    degree: 1,
                    variant: Variant::Nodal(nf),
                });
            }
        }
    }

    for key in keys {
        let Err(err) = Element::get(key) else {
            panic!("expected Unsupported for {key:?}");
        };
        assert_eq!(
            err,
            ElementError::Unsupported {
                family: key.family,
                cell: key.cell,
                degree: key.degree,
                variant: key.variant,
            },
            "expected Unsupported for {key:?}"
        );
    }
}

/// M11 acceptance A6: for every Lagrange element and variant of section 26 with p <= 10
/// (`Nodal(Equispaced)`: p <= 6), applying its interpolation to its basis gives the identity
/// to 1e-12.
#[test]
fn interpolation_of_basis_is_identity() {
    for cell in ALL_CELLS {
        let td = tdim(cell);
        let cap = max_degree(cell).min(10);
        for variant in variants_for(cell) {
            let max_p = if variant == Variant::Nodal(NodeFamily::Equispaced) {
                cap.min(6)
            } else {
                cap
            };
            for degree in 0..=max_p {
                let key = lagrange_key(cell, degree, variant);
                let el = Element::get(key).unwrap();
                let dim = el.dim();
                assert_eq!(el.reference_value_size(), 1, "{key:?}: scalar");

                let pts = el.interpolation_points();
                assert_eq!(pts.len() % td, 0);
                let m = pts.len() / td;
                let mat = el.interpolation_matrix();
                assert_eq!(mat.len(), dim * m);

                let tab = el.tabulate(pts, 0).unwrap();
                assert_eq!(tab.values.len(), m * dim);

                let mut row = vec![0.0; dim];
                for i in 0..dim {
                    row.fill(0.0);
                    for (q, &a) in mat[i * m..(i + 1) * m].iter().enumerate() {
                        if a != 0.0 {
                            for (r, &b) in row.iter_mut().zip(&tab.values[q * dim..(q + 1) * dim]) {
                                *r += a * b;
                            }
                        }
                    }
                    for (j, &val) in row.iter().enumerate() {
                        let want = if i == j { 1.0 } else { 0.0 };
                        let err = (val - want).abs();
                        assert!(err <= TOL, "{key:?}: (M B)[{i}, {j}] = {val} (err {err:e})");
                    }
                }
            }
        }
    }
}

/// M11 acceptance A7: for `Legendre` on every cell, p = 0–10 (pyramids: p = 0–8), the reference
/// mass matrix under a `Gauss` rule of degree 2p is the identity to 1e-12.
#[test]
fn legendre_mass_matrix_is_identity() {
    for cell in ALL_CELLS {
        for degree in 0..=max_degree(cell).min(10) {
            let key = lagrange_key(cell, degree, Variant::Legendre);
            let el = Element::get(key).unwrap();
            let dim = el.dim();
            let rule = gauss_rule(cell, 2 * degree);
            let tab = el.tabulate(&rule.points, 0).unwrap();
            assert_eq!(tab.values.len(), rule.weights.len() * dim);

            let mut gram = vec![0.0; dim * dim];
            for (q, &w) in rule.weights.iter().enumerate() {
                let row = &tab.values[q * dim..(q + 1) * dim];
                for i in 0..dim {
                    let wi = w * row[i];
                    for j in i..dim {
                        gram[i * dim + j] += wi * row[j];
                    }
                }
            }
            for i in 0..dim {
                for j in i..dim {
                    let want = if i == j { 1.0 } else { 0.0 };
                    let err = (gram[i * dim + j] - want).abs();
                    assert!(
                        err <= TOL,
                        "{key:?}: mass[{i}, {j}] = {} (err {err:e})",
                        gram[i * dim + j]
                    );
                }
            }
        }
    }
}

fn apply_generator(entity: CellType, generator: Generator, points: &[f64]) -> Vec<f64> {
    match (entity, generator) {
        (CellType::Interval, Generator::Reversal) => points.iter().map(|&t| 1.0 - t).collect(),
        (CellType::Triangle, Generator::Rotation) => points
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[1], 1.0 - p[0] - p[1]])
            .collect(),
        (CellType::Triangle | CellType::Quadrilateral, Generator::Reflection) => points
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[1], p[0]])
            .collect(),
        (CellType::Quadrilateral, Generator::Rotation) => points
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[1], 1.0 - p[0]])
            .collect(),
        _ => panic!("unsupported ({entity:?}, {generator:?})"),
    }
}

/// M11 acceptance A8: every base transformation of every Lagrange element with p <= 10
/// (pyramids: p <= 8) is a permutation without negation, and the raw base-transformation matrix
/// computed directly from `Element::tabulate` matches that permutation to 1e-12.
#[test]
fn lagrange_base_transformations_are_permutations() {
    let entity_generators = [
        (CellType::Interval, Generator::Reversal),
        (CellType::Triangle, Generator::Rotation),
        (CellType::Triangle, Generator::Reflection),
        (CellType::Quadrilateral, Generator::Rotation),
        (CellType::Quadrilateral, Generator::Reflection),
    ];

    for cell in ALL_CELLS {
        let td = tdim(cell);
        for variant in variants_for(cell) {
            for degree in 0..=max_degree(cell).min(10) {
                let key = lagrange_key(cell, degree, variant);
                let el = Element::get(key).unwrap();
                let dim = el.dim();
                let kind = el.transformation_kind();
                assert_ne!(
                    kind,
                    TransformationKind::Dense,
                    "{key:?}: Lagrange must not be Dense"
                );

                let mut any_nontrivial = false;
                for (entity, generator) in entity_generators {
                    let ed = tdim(entity);
                    let rep = if ed < td {
                        sub_entity_types(cell, ed)
                            .iter()
                            .enumerate()
                            .find(|(_, ty)| **ty == entity)
                            .map(|(e, _)| e)
                    } else {
                        None
                    };
                    let expected = rep.map_or(0, |e| el.entity_dofs(ed, e).len());
                    match el.base_transformation(entity, generator) {
                        None => assert_eq!(expected, 0, "{key:?} ({entity:?}, {generator:?})"),
                        Some(BaseTransformation::SignedPermutation { perm, negate }) => {
                            assert!(
                                expected > 0 && perm.len() == expected && negate.len() == expected
                            );
                            assert!(negate.iter().all(|&b| !b), "{key:?}: negation in Lagrange");
                            let mut seen = vec![false; expected];
                            for (i, &p) in perm.iter().enumerate() {
                                let idx = p as usize;
                                assert!(idx < expected && !seen[idx], "{key:?}: bad perm {perm:?}");
                                seen[idx] = true;
                                any_nontrivial |= idx != i;
                            }

                            // Directly check the numerically evaluated base-transformation matrix
                            // against the classified permutation at section 16's 1e-12 tolerance.
                            let Variant::Nodal(family) = variant else {
                                panic!("{key:?}: non-nodal element has sub-entity DOFs");
                            };
                            let e = rep.unwrap();
                            let dofs = el.entity_dofs(ed, e);
                            let all_pts = lagrange_points(entity, degree, family).unwrap();
                            let interior = lagrange_entity_nodes(entity, degree, ed, 0);
                            let epts = &all_pts[interior.start * ed..interior.end * ed];
                            let reoriented = apply_generator(entity, generator, epts);
                            let cell_pts = map_entity_points(
                                cell,
                                entity,
                                sub_entities(cell, ed)[e],
                                &reoriented,
                            );
                            let tab = el.tabulate(&cell_pts, 0).unwrap();
                            for (i, &pi) in perm.iter().enumerate() {
                                for (j, &dof) in dofs.iter().enumerate() {
                                    let got = tab.values[i * dim + dof];
                                    let want = if pi as usize == j { 1.0 } else { 0.0 };
                                    let err = (got - want).abs();
                                    assert!(
                                        err <= TOL,
                                        "{key:?} ({entity:?}, {generator:?}) [{i}, {j}]: {got} vs {want} (err {err:e})"
                                    );
                                }
                            }
                        }
                        Some(BaseTransformation::Dense { .. }) => {
                            panic!("{key:?}: Dense in Lagrange")
                        }
                    }
                }
                let want = if any_nontrivial {
                    TransformationKind::SignedPermutation
                } else {
                    TransformationKind::Identity
                };
                assert_eq!(kind, want, "{key:?}: transformation_kind");
            }
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z >> 11) as f64) * (2.0 / ((1u64 << 53) as f64)) - 1.0
    }
}

/// All valid vertex symmetries of a sub-entity type (1, 2, 6 or 8): entry `k` is the canonical
/// vertex index sitting at local vertex `k`.
fn entity_symmetries(entity: CellType) -> Vec<Vec<usize>> {
    match entity {
        CellType::Point => vec![vec![0]],
        CellType::Interval => vec![vec![0, 1], vec![1, 0]],
        CellType::Triangle => vec![
            vec![0, 1, 2],
            vec![0, 2, 1],
            vec![1, 0, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
            vec![2, 1, 0],
        ],
        CellType::Quadrilateral => (0..4)
            .flat_map(|s| {
                [(s ^ 1, s ^ 2), (s ^ 2, s ^ 1)].map(move |(n1, n2)| vec![s, n1, n2, s ^ 3])
            })
            .collect(),
        other => panic!("not a sub-entity type: {other:?}"),
    }
}

fn apply_bt(bt: &BaseTransformation, slice: &mut [f64]) {
    let orig = slice.to_vec();
    match bt {
        BaseTransformation::SignedPermutation { perm, negate } => {
            for (i, (&p, &neg)) in perm.iter().zip(negate).enumerate() {
                slice[i] = if neg {
                    -orig[p as usize]
                } else {
                    orig[p as usize]
                };
            }
        }
        BaseTransformation::Dense { n, matrix } => {
            for (i, out) in slice.iter_mut().enumerate() {
                *out = (0..*n).map(|j| matrix[i * n + j] * orig[j]).sum();
            }
        }
    }
}

/// Composes base transformations from `orientation_bits` (A.6) to map canonical global DOF
/// coefficients to cell-local reference coefficients (`u_local = M_R^rot M_S^refl u_global`).
fn apply_cell_dof_transformations(el: &Element, bits: u32, coeffs: &mut [f64]) {
    let cell = el.key().cell;
    let td = tdim(cell);
    let mut apply_to = |dofs: &[usize], bt: &BaseTransformation, times: u32| {
        if dofs.is_empty() || times == 0 {
            return;
        }
        let mut block: Vec<f64> = dofs.iter().map(|&d| coeffs[d]).collect();
        for _ in 0..times {
            apply_bt(bt, &mut block);
        }
        for (&d, v) in dofs.iter().zip(block) {
            coeffs[d] = v;
        }
    };
    if td >= 2
        && let Some(bt) = el.base_transformation(CellType::Interval, Generator::Reversal)
    {
        for e in 0..num_sub_entities(cell, 1) {
            if (bits >> e) & 1 == 1 {
                apply_to(el.entity_dofs(1, e), bt, 1);
            }
        }
    }
    if td == 3 {
        let n_edges = num_sub_entities(cell, 1);
        for (f, &fty) in sub_entity_types(cell, 2).iter().enumerate() {
            let shift = n_edges + 3 * f;
            let rot = (bits >> shift) & 3;
            let refl = (bits >> (shift + 2)) & 1;
            if refl == 1
                && let Some(bt) = el.base_transformation(fty, Generator::Reflection)
            {
                apply_to(el.entity_dofs(2, f), bt, 1);
            }
            if rot > 0
                && let Some(bt) = el.base_transformation(fty, Generator::Rotation)
            {
                apply_to(el.entity_dofs(2, f), bt, rot);
            }
        }
    }
}

fn shared_entity_points(entity: CellType, degree: u32) -> Vec<f64> {
    if entity == CellType::Point {
        return Vec::new();
    }
    let mut pts = gauss_rule(entity, degree + 1).points;
    let extra: &[f64] = match entity {
        CellType::Interval => &[0.0, 0.17, 0.63, 1.0],
        CellType::Triangle => &[0.13, 0.29, 0.61, 0.11, 0.21, 0.67],
        CellType::Quadrilateral => &[0.13, 0.79, 0.83, 0.21, 0.41, 0.09],
        other => panic!("not a shared entity type: {other:?}"),
    };
    pts.extend_from_slice(extra);
    pts
}

fn map_entity_points(
    cell: CellType,
    entity: CellType,
    cell_verts: &[usize],
    pts: &[f64],
) -> Vec<f64> {
    let (td, ed) = (tdim(cell), tdim(entity));
    let verts = vertices(cell);
    let v0 = verts[cell_verts[0]];
    if ed == 0 {
        return v0.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() / ed * td);
    for t in pts.chunks_exact(ed) {
        for i in 0..td {
            let delta: f64 = (0..ed)
                .map(|k| t[k] * (verts[cell_verts[k + 1]][i] - v0[i]))
                .sum();
            out.push(v0[i] + delta);
        }
    }
    out
}

fn assign_vertex_ids(
    cell: CellType,
    entity_verts: &[usize],
    sym: &[usize],
    base_other: i64,
) -> Vec<i64> {
    let mut ids = vec![-1i64; vertices(cell).len()];
    for (&v, &c) in entity_verts.iter().zip(sym) {
        ids[v] = c as i64;
    }
    let mut next = base_other;
    for id in &mut ids {
        if *id < 0 {
            *id = next;
            next += 1;
        }
    }
    ids
}

fn cell_local_coeffs(
    el: &Element,
    vertex_ids: &[i64],
    global_dofs: &mut BTreeMap<(usize, Vec<i64>), Vec<f64>>,
    rng: &mut Rng,
) -> Vec<f64> {
    let cell = el.key().cell;
    let td = tdim(cell);
    let mut coeffs = vec![0.0; el.dim()];
    for d in 0..=td {
        for (e, (&verts, &ty)) in sub_entities(cell, d)
            .iter()
            .zip(sub_entity_types(cell, d))
            .enumerate()
        {
            let dofs = el.entity_dofs(d, e);
            if dofs.is_empty() {
                continue;
            }
            let local_ids: Vec<i64> = verts.iter().map(|&v| vertex_ids[v]).collect();
            let key_ids = if d < td {
                canonical_order(ty, &local_ids)
                    .into_iter()
                    .map(|s| local_ids[s])
                    .collect()
            } else {
                local_ids
            };
            let vals = global_dofs
                .entry((d, key_ids))
                .or_insert_with(|| (0..dofs.len()).map(|_| rng.next_f64()).collect());
            for (&dof, &v) in dofs.iter().zip(vals.iter()) {
                coeffs[dof] = v;
            }
        }
    }
    apply_cell_dof_transformations(el, orientation_bits(cell, vertex_ids), &mut coeffs);
    coeffs
}

fn eval_at_points(coeffs: &[f64], values: &[f64], npts: usize) -> Vec<f64> {
    let dim = coeffs.len();
    (0..npts)
        .map(|q| {
            coeffs
                .iter()
                .zip(&values[q * dim..(q + 1) * dim])
                .map(|(c, b)| c * b)
                .sum()
        })
        .collect()
}

/// M11 acceptance A10: on two cells sharing an entity, for every relative orientation and every
/// pair of cell types that can share it, a Lagrange function with random coefficients is
/// continuous across the entity to 1e-12 for p <= 6.
#[test]
fn lagrange_is_continuous_across_shared_entities() {
    let dim_groups: &[&[CellType]] = &[
        &[CellType::Interval],
        &[CellType::Triangle, CellType::Quadrilateral],
        &[
            CellType::Tetrahedron,
            CellType::Hexahedron,
            CellType::Prism,
            CellType::Pyramid,
        ],
    ];
    let mut rng = Rng(0x51a7_c0de_2026_0011);

    for group in dim_groups {
        let td = tdim(group[0]);
        for (ia, &cell_a) in group.iter().enumerate() {
            for &cell_b in &group[ia..] {
                let mut variants = vec![Variant::Nodal(NodeFamily::Equispaced)];
                if cell_a != CellType::Pyramid && cell_b != CellType::Pyramid {
                    variants.push(Variant::Nodal(NodeFamily::Gll));
                }
                for variant in variants {
                    for degree in 1..=6 {
                        let el_a = Element::get(lagrange_key(cell_a, degree, variant)).unwrap();
                        let el_b = Element::get(lagrange_key(cell_b, degree, variant)).unwrap();
                        for dim in 0..td {
                            let (ents_a, tys_a) =
                                (sub_entities(cell_a, dim), sub_entity_types(cell_a, dim));
                            let (ents_b, tys_b) =
                                (sub_entities(cell_b, dim), sub_entity_types(cell_b, dim));
                            let mut pts_cache = BTreeMap::new();

                            let tabs_a: Vec<_> = ents_a
                                .iter()
                                .zip(tys_a)
                                .map(|(&va, &ty)| {
                                    let epts = pts_cache
                                        .entry(ty)
                                        .or_insert_with(|| shared_entity_points(ty, degree));
                                    el_a.tabulate(&map_entity_points(cell_a, ty, va, epts), 0)
                                        .unwrap()
                                })
                                .collect();
                            let tabs_b: Vec<Vec<_>> = ents_b
                                .iter()
                                .zip(tys_b)
                                .map(|(&vb, &ty)| {
                                    let epts = pts_cache
                                        .entry(ty)
                                        .or_insert_with(|| shared_entity_points(ty, degree));
                                    entity_symmetries(ty)
                                        .into_iter()
                                        .map(|sym| {
                                            let mut vb_canon = vec![0usize; sym.len()];
                                            for (pos, &c) in sym.iter().enumerate() {
                                                vb_canon[c] = vb[pos];
                                            }
                                            el_b.tabulate(
                                                &map_entity_points(cell_b, ty, &vb_canon, epts),
                                                0,
                                            )
                                            .unwrap()
                                        })
                                        .collect()
                                })
                                .collect();

                            for (e_a, (&va, &ty_a)) in ents_a.iter().zip(tys_a).enumerate() {
                                let id_sym: Vec<usize> = (0..va.len()).collect();
                                let ids_a = assign_vertex_ids(cell_a, va, &id_sym, 100);
                                let syms = entity_symmetries(ty_a);
                                for (e_b, (&vb, &ty_b)) in ents_b.iter().zip(tys_b).enumerate() {
                                    if ty_a != ty_b {
                                        continue;
                                    }
                                    for (s_idx, sym) in syms.iter().enumerate() {
                                        let ids_b = assign_vertex_ids(cell_b, vb, sym, 200);
                                        let mut global_dofs = BTreeMap::new();
                                        let ca = cell_local_coeffs(
                                            &el_a,
                                            &ids_a,
                                            &mut global_dofs,
                                            &mut rng,
                                        );
                                        let cb = cell_local_coeffs(
                                            &el_b,
                                            &ids_b,
                                            &mut global_dofs,
                                            &mut rng,
                                        );
                                        let (ta, tb) = (&tabs_a[e_a], &tabs_b[e_b][s_idx]);
                                        assert_eq!(ta.num_points, tb.num_points);
                                        let va_vals =
                                            eval_at_points(&ca, &ta.values, ta.num_points);
                                        let vb_vals =
                                            eval_at_points(&cb, &tb.values, tb.num_points);
                                        let scale = va_vals
                                            .iter()
                                            .chain(&vb_vals)
                                            .fold(1.0_f64, |m, v| m.max(v.abs()));
                                        for (q, (&a, &b)) in
                                            va_vals.iter().zip(&vb_vals).enumerate()
                                        {
                                            let err = (a - b).abs() / scale;
                                            assert!(
                                                err <= TOL,
                                                "{cell_a:?}(e={e_a})/{cell_b:?}(e={e_b}) {ty_a:?} sym={sym:?} {variant:?} p={degree} pt={q}: {a} vs {b} (rel err {err:e})"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Shifted Chebyshev polynomial `T_k(2t - 1)` on `[0, 1]`, bounded in `[-1, 1]`.
fn chebyshev_01(k: u32, t: f64) -> f64 {
    let u = 2.0 * t - 1.0;
    if k == 0 {
        return 1.0;
    }
    let (mut prev, mut cur) = (1.0, u);
    for _ in 1..k {
        let next = 2.0 * u * cur - prev;
        prev = cur;
        cur = next;
    }
    cur
}

/// A bounded degree-`p` polynomial lying in the Lagrange space of `cell`.
fn test_polynomial(cell: CellType, p: u32, pt: &[f64]) -> f64 {
    match cell {
        CellType::Interval => chebyshev_01(p, pt[0]) + 0.5 * chebyshev_01(p - 1, pt[0]),
        CellType::Triangle => {
            let (a, b) = (p / 2, p - p / 2);
            chebyshev_01(p, pt[0] + pt[1]) + 0.5 * chebyshev_01(a, pt[0]) * chebyshev_01(b, pt[1])
        }
        CellType::Quadrilateral => {
            chebyshev_01(p, pt[0]) * chebyshev_01(p, pt[1])
                + 0.5 * chebyshev_01(p - 1, pt[0]) * chebyshev_01(p, pt[1])
        }
        CellType::Tetrahedron => {
            let a = p / 3;
            let b = p / 3;
            let c = p - a - b;
            chebyshev_01(p, pt[0] + pt[1] + pt[2])
                + 0.5 * chebyshev_01(a, pt[0]) * chebyshev_01(b, pt[1]) * chebyshev_01(c, pt[2])
        }
        CellType::Hexahedron => {
            chebyshev_01(p, pt[0]) * chebyshev_01(p, pt[1]) * chebyshev_01(p, pt[2])
                + 0.5
                    * chebyshev_01(p - 1, pt[0])
                    * chebyshev_01(p, pt[1])
                    * chebyshev_01(p - 2, pt[2])
        }
        CellType::Prism => {
            let (a, b) = (p / 2, p - p / 2);
            (chebyshev_01(p, pt[0] + pt[1]) + 0.5 * chebyshev_01(a, pt[0]) * chebyshev_01(b, pt[1]))
                * chebyshev_01(p, pt[2])
        }
        CellType::Point | CellType::Pyramid => unreachable!("no degree > 10 element on {cell:?}"),
    }
}

/// Deterministic test points per cell (interior and boundary) for cells with max degree > 10.
fn reproduction_points(cell: CellType) -> &'static [f64] {
    match cell {
        CellType::Interval => &[0.13, 0.37, 0.68, 0.91],
        CellType::Triangle => &[0.11, 0.17, 0.31, 0.43, 0.59, 0.23, 0.0, 0.41],
        CellType::Quadrilateral => &[0.11, 0.19, 0.37, 0.73, 0.83, 0.29, 0.0, 0.61],
        CellType::Tetrahedron => &[
            0.11, 0.13, 0.19, 0.27, 0.31, 0.17, 0.09, 0.41, 0.37, 0.23, 0.0, 0.29,
        ],
        CellType::Hexahedron => &[
            0.11, 0.19, 0.29, 0.43, 0.67, 0.23, 0.79, 0.31, 0.83, 0.0, 0.53, 0.41,
        ],
        CellType::Prism => &[
            0.11, 0.17, 0.23, 0.31, 0.41, 0.67, 0.53, 0.19, 0.89, 0.0, 0.37, 0.51,
        ],
        CellType::Point | CellType::Pyramid => unreachable!("no degree > 10 element on {cell:?}"),
    }
}

/// Strided index selection of length at most 33, always including `0` and `n - 1`.
fn strided_indices(n: usize) -> Vec<usize> {
    if n <= 33 {
        return (0..n).collect();
    }
    let step = n.div_ceil(32);
    let mut idxs: Vec<usize> = (0..n).step_by(step).collect();
    if *idxs.last().unwrap() != n - 1 {
        idxs.push(n - 1);
    }
    idxs
}

/// Checks that `dofs` tabulated at `test_pts` reproduces `exact` to `TOL` relative to
/// `max(1, max |exact|)`.
fn assert_reproduces(key: ElementKey, dofs: &[f64], tab_values: &[f64], exact: &[f64]) {
    let approx = eval_at_points(dofs, tab_values, exact.len());
    let scale = exact.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
    for (q, (&got, &want)) in approx.iter().zip(exact).enumerate() {
        let err = (got - want).abs() / scale;
        assert!(
            err <= TOL,
            "{key:?}: reproduction pt {q}: {got} vs {want} (rel err {err:e})"
        );
    }
}

/// M11 G8: checks interpolation identity and degree-`p` polynomial reproduction to 1e-12 at
/// intermediate and maximum degrees above A6's `p = 10` bound (`Gll` and `Legendre` up to
/// p = 15/20), keeping hexahedron dense tables lazy.
#[test]
fn high_degree_interpolation_and_reproduction() {
    let cases: &[(CellType, &[u32], &[Variant])] = &[
        (
            CellType::Interval,
            &[12, 16, 20],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
        (
            CellType::Triangle,
            &[12, 15],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
        (
            CellType::Quadrilateral,
            &[12, 16, 20],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
        (
            CellType::Tetrahedron,
            &[12, 15],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
        (
            CellType::Hexahedron,
            &[12, 16, 20],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
        (
            CellType::Prism,
            &[12, 15],
            &[Variant::Nodal(NodeFamily::Gll), Variant::Legendre],
        ),
    ];

    for &(cell, degrees, variants) in cases {
        let td = tdim(cell);
        let test_pts = reproduction_points(cell);
        let ntest = test_pts.len() / td;

        for &variant in variants {
            for &p in degrees {
                let key = lagrange_key(cell, p, variant);
                let el = Element::get(key).unwrap();
                let dim = el.dim();
                let tab_test = el.tabulate(test_pts, 0).unwrap();

                match variant {
                    Variant::Nodal(_) => {
                        let pts = el.interpolation_points();
                        assert_eq!(pts.len(), dim * td, "{key:?}");

                        // Interpolation identity: M = I, so φ_j(x_i) = δ_{ij}.
                        // Sample rows when dim > 1000 (Prism p=12/15, Hexahedron p=12/16/20) to
                        // stay fast and keep hexahedron lazy tables unallocated.
                        let check_rows: Vec<usize> = if dim <= 1000 {
                            (0..dim).collect()
                        } else {
                            strided_indices(dim)
                        };
                        if cell != CellType::Hexahedron {
                            let mat = el.interpolation_matrix();
                            assert_eq!(mat.len(), dim * dim, "{key:?}");
                            for &i in &check_rows {
                                for j in 0..dim {
                                    let want = if i == j { 1.0 } else { 0.0 };
                                    assert_eq!(mat[i * dim + j], want, "{key:?}: M[{i}, {j}]");
                                }
                            }
                        }

                        let check_pts: Vec<f64> = check_rows
                            .iter()
                            .flat_map(|&i| &pts[i * td..(i + 1) * td])
                            .copied()
                            .collect();
                        let tab_nodes = el.tabulate(&check_pts, 0).unwrap();
                        for (k, &i) in check_rows.iter().enumerate() {
                            for j in 0..dim {
                                let want = if i == j { 1.0 } else { 0.0 };
                                let got = tab_nodes.values[k * dim + j];
                                let err = (got - want).abs();
                                assert!(err <= TOL, "{key:?}: B[{i}, {j}] = {got} (err {err:e})");
                            }
                        }

                        // Polynomial reproduction at test_pts (never allocates dense tables).
                        let dofs: Vec<f64> = pts
                            .chunks_exact(td)
                            .map(|pt| test_polynomial(cell, p, pt))
                            .collect();
                        let exact: Vec<f64> = test_pts
                            .chunks_exact(td)
                            .map(|pt| test_polynomial(cell, p, pt))
                            .collect();
                        assert_reproduces(key, &dofs, &tab_test.values, &exact);
                    }
                    Variant::Legendre if cell == CellType::Hexahedron => {
                        // Keep Hexahedron's dim x dim interpolation_matrix lazy: check tabulation
                        // directly against polyset::tabulate, and verify 3D separable polynomial
                        // reproduction via the 1D Legendre interval element (whose full M B = I
                        // is checked in the Interval case above).
                        let poly = polyset::tabulate(cell, p, test_pts, 0);
                        let poly_scale = poly.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                        for q in 0..ntest {
                            for j in 0..dim {
                                let got = tab_test.values[q * dim + j];
                                let want = poly[j * ntest + q];
                                let err = (got - want).abs() / poly_scale;
                                assert!(
                                    err <= TOL,
                                    "{key:?}: tabulate vs polyset pt {q} basis {j}: {got} vs {want} (rel err {err:e})"
                                );
                            }
                        }

                        let tf = el.tensor_factors().expect("hexahedron has tensor factors");
                        let el1 =
                            Element::get(lagrange_key(CellType::Interval, p, Variant::Legendre))
                                .unwrap();
                        let (n, pts1, mat1) = (
                            el1.dim(),
                            el1.interpolation_points(),
                            el1.interpolation_matrix(),
                        );
                        let m1 = pts1.len();
                        let hex_pts = el.interpolation_points();
                        assert_eq!(hex_pts.len(), m1 * m1 * m1 * td, "{key:?}");
                        for (q2, &z) in pts1.iter().enumerate() {
                            for (q1, &y) in pts1.iter().enumerate() {
                                for (q0, &x) in pts1.iter().enumerate() {
                                    let q = q0 + m1 * (q1 + m1 * q2);
                                    let got = &hex_pts[q * td..(q + 1) * td];
                                    for (d, (&g, w)) in got.iter().zip([x, y, z]).enumerate() {
                                        let err = (g - w).abs();
                                        assert!(
                                            err <= TOL,
                                            "{key:?}: interp point {q} coord {d}: {g} vs {w} (err {err:e})"
                                        );
                                    }
                                }
                            }
                        }
                        let proj_1d = |k: u32| -> Vec<f64> {
                            let s: Vec<f64> = pts1.iter().map(|&t| chebyshev_01(k, t)).collect();
                            (0..n)
                                .map(|i| {
                                    mat1[i * m1..(i + 1) * m1]
                                        .iter()
                                        .zip(&s)
                                        .map(|(a, v)| a * v)
                                        .sum()
                                })
                                .collect()
                        };
                        let (dx, dy, dz) = (proj_1d(p), proj_1d(p - 1), proj_1d(p - 2));
                        let mut dofs = vec![0.0; dim];
                        for (a2, &vz) in dz.iter().enumerate() {
                            for (a1, &vy) in dy.iter().enumerate() {
                                for (a0, &vx) in dx.iter().enumerate() {
                                    let dof = tf.lexicographic[0][a0 + n * (a1 + n * a2)];
                                    dofs[dof] = vx * vy * vz;
                                }
                            }
                        }
                        let exact: Vec<f64> = test_pts
                            .chunks_exact(td)
                            .map(|pt| {
                                chebyshev_01(p, pt[0])
                                    * chebyshev_01(p - 1, pt[1])
                                    * chebyshev_01(p - 2, pt[2])
                            })
                            .collect();
                        assert_reproduces(key, &dofs, &tab_test.values, &exact);
                    }
                    Variant::Legendre => {
                        let pts = el.interpolation_points();
                        let m = pts.len() / td;
                        let mat = el.interpolation_matrix();
                        assert_eq!(mat.len(), dim * m, "{key:?}");
                        let tab_nodes = el.tabulate(pts, 0).unwrap();

                        let check_rows: Vec<usize> = if dim <= 1000 {
                            (0..dim).collect()
                        } else {
                            strided_indices(dim)
                        };
                        let mut row = vec![0.0; dim];
                        for &i in &check_rows {
                            row.fill(0.0);
                            for (q, &a) in mat[i * m..(i + 1) * m].iter().enumerate() {
                                if a != 0.0 {
                                    for (r, &b) in row
                                        .iter_mut()
                                        .zip(&tab_nodes.values[q * dim..(q + 1) * dim])
                                    {
                                        *r += a * b;
                                    }
                                }
                            }
                            for (j, &val) in row.iter().enumerate() {
                                let want = if i == j { 1.0 } else { 0.0 };
                                let err = (val - want).abs();
                                assert!(
                                    err <= TOL,
                                    "{key:?}: (M B)[{i}, {j}] = {val} (err {err:e})"
                                );
                            }
                        }

                        // Polynomial reproduction: dofs = M f(pts).
                        let samples: Vec<f64> = pts
                            .chunks_exact(td)
                            .map(|pt| test_polynomial(cell, p, pt))
                            .collect();
                        let dofs: Vec<f64> = (0..dim)
                            .map(|i| {
                                mat[i * m..(i + 1) * m]
                                    .iter()
                                    .zip(&samples)
                                    .map(|(a, s)| a * s)
                                    .sum()
                            })
                            .collect();
                        let exact: Vec<f64> = test_pts
                            .chunks_exact(td)
                            .map(|pt| test_polynomial(cell, p, pt))
                            .collect();
                        assert_reproduces(key, &dofs, &tab_test.values, &exact);
                    }
                }
            }
        }
    }
}

/// M11 G8: every supported nodal Lagrange key in section 26's table constructs without panicking
/// and classifies every sub-entity base transformation as a pure permutation (not `Dense`),
/// exhaustively backing `Element::get`'s `# Panics` invariant.
#[test]
fn all_nodal_lagrange_keys_construct() {
    for cell in ALL_CELLS {
        let max_p = max_degree(cell);
        for variant in variants_for(cell) {
            if !matches!(variant, Variant::Nodal(_)) {
                continue;
            }
            for degree in 0..=max_p {
                let key = lagrange_key(cell, degree, variant);
                let el = Element::get(key).unwrap();
                assert_ne!(
                    el.transformation_kind(),
                    TransformationKind::Dense,
                    "{key:?}: Lagrange must not be Dense"
                );
            }
        }
    }
}

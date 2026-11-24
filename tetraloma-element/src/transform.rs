// Design: section 26 ("Finite elements", "DOF transformations"), Listing E1; Appendix A.6;
// ADR-0071.
// Follow-up (M13): replace `sub_entity_points_to_cell` below by
// `tetraloma_refcell::sub_entity_points_to_cell` once Listing Q1 lands.
//! Numerical computation and classification of sub-entity base DOF transformations.

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    lagrange_entity_nodes, lagrange_points, sub_entities, sub_entity_types, tdim, vertices,
};

use crate::{
    BaseTransformation, ElementError, ElementKey, ElementTabulation, Family, Generator,
    TransformationKind, Variant,
};

/// Tolerance for recognising a numerically computed base transformation matrix as a signed
/// permutation (one entry per row and column within `SIGNED_PERM_TOL` of `±1`, all others
/// within `SIGNED_PERM_TOL` of `0`).
///
/// Tight enough to reject any genuinely dense transformation block, while staying well above
/// rounding error across all supported Lagrange elements (up to degree 20 on intervals,
/// quadrilaterals and hexahedra, degree 15 on triangles, tetrahedra and prisms, and degree 8 on
/// pyramids).
const SIGNED_PERM_TOL: f64 = 1e-9;

/// All five `(sub-entity type, generator)` pairs defined by Appendix A.6.
const ENTITY_GENERATORS: [(CellType, Generator); 5] = [
    (CellType::Interval, Generator::Reversal),
    (CellType::Triangle, Generator::Rotation),
    (CellType::Triangle, Generator::Reflection),
    (CellType::Quadrilateral, Generator::Rotation),
    (CellType::Quadrilateral, Generator::Reflection),
];

/// Maps a valid `(sub-entity type, generator)` pair to its slot in `BaseTransformations::tables`.
fn slot(entity: CellType, generator: Generator) -> Option<usize> {
    match (entity, generator) {
        (CellType::Interval, Generator::Reversal) => Some(0),
        (CellType::Triangle, Generator::Rotation) => Some(1),
        (CellType::Triangle, Generator::Reflection) => Some(2),
        (CellType::Quadrilateral, Generator::Rotation) => Some(3),
        (CellType::Quadrilateral, Generator::Reflection) => Some(4),
        _ => None,
    }
}

/// Stored base transformations and classification of one element.
pub(crate) struct BaseTransformations {
    pub(crate) kind: TransformationKind,
    tables: [Option<BaseTransformation>; 5],
}

impl BaseTransformations {
    pub(crate) fn get(
        &self,
        entity: CellType,
        generator: Generator,
    ) -> Option<&BaseTransformation> {
        slot(entity, generator).and_then(|idx| self.tables[idx].as_ref())
    }
}

/// Private affine sub-entity map until M13 adds `tetraloma_refcell::sub_entity_points_to_cell`.
fn sub_entity_points_to_cell(cell: CellType, d: usize, e: usize, points: &[f64]) -> Vec<f64> {
    let td = tdim(cell);
    let list = sub_entities(cell, d)[e];
    let verts = vertices(cell);
    let v0 = verts[list[0]];
    if d == 0 {
        debug_assert!(points.is_empty());
        return v0.to_vec();
    }
    debug_assert!(points.len().is_multiple_of(d));
    let mut out = Vec::with_capacity((points.len() / d) * td);
    for t in points.chunks_exact(d) {
        for i in 0..td {
            let delta: f64 = (0..d).map(|k| t[k] * (verts[list[k + 1]][i] - v0[i])).sum();
            out.push(v0[i] + delta);
        }
    }
    out
}

/// Applies the reorientation map `Φ_G: K̂_entity → K̂_entity` of `generator` to flat reference
/// points on `entity`.
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
        _ => unreachable!("apply_generator called with unsupported ({entity:?}, {generator:?})"),
    }
}

/// Classifies a row-major `n × n` matrix as a `SignedPermutation` if every row and column has
/// one entry within `SIGNED_PERM_TOL` of `±1` and all other entries within `SIGNED_PERM_TOL` of
/// `0`, or as `Dense` otherwise. Non-finite entries (including `NaN`) are classified as `Dense`.
fn classify_matrix(matrix: Vec<f64>, n: usize) -> BaseTransformation {
    debug_assert_eq!(matrix.len(), n * n);
    let mut perm = Vec::with_capacity(n);
    let mut negate = Vec::with_capacity(n);
    let mut seen = vec![false; n];

    for row in matrix.chunks_exact(n) {
        let mut best_col = 0;
        let mut best_val = 0.0_f64;
        for (j, &v) in row.iter().enumerate() {
            if !v.is_finite() {
                return BaseTransformation::Dense { n, matrix };
            }
            if v.abs() > best_val.abs() {
                best_col = j;
                best_val = v;
            }
        }
        if (best_val.abs() - 1.0).abs() > SIGNED_PERM_TOL || seen[best_col] {
            return BaseTransformation::Dense { n, matrix };
        }
        for (j, &v) in row.iter().enumerate() {
            if j != best_col && v.abs() > SIGNED_PERM_TOL {
                return BaseTransformation::Dense { n, matrix };
            }
        }
        seen[best_col] = true;
        perm.push(u32::try_from(best_col).expect("sub-entity DOF index fits in u32"));
        negate.push(best_val < 0.0);
    }

    BaseTransformation::SignedPermutation { perm, negate }
}

/// Classifies the element's overall `TransformationKind` from its stored base transformations.
fn classify_kind(tables: &[Option<BaseTransformation>; 5]) -> TransformationKind {
    let mut any_nontrivial = false;
    for bt in tables.iter().flatten() {
        match bt {
            BaseTransformation::Dense { .. } => return TransformationKind::Dense,
            BaseTransformation::SignedPermutation { perm, negate } => {
                any_nontrivial |= perm.iter().enumerate().any(|(i, &p)| p as usize != i)
                    || negate.iter().any(|&neg| neg);
            }
        }
    }
    if any_nontrivial {
        TransformationKind::SignedPermutation
    } else {
        TransformationKind::Identity
    }
}

/// Reference data for one representative sub-entity of a cell.
struct SubEntityRef<'a> {
    entity: CellType,
    index: usize,
    dofs: &'a [usize],
    points: &'a [f64],
}

/// Numerically computes the `n × n` base transformation matrix on one sub-entity of a nodal
/// Lagrange element by evaluating the basis at the reoriented interior nodes of that sub-entity.
fn compute_nodal_matrix<F>(
    cell: CellType,
    dim: usize,
    tabulate_values: &F,
    sub: SubEntityRef<'_>,
    generator: Generator,
) -> Result<Vec<f64>, ElementError>
where
    F: Fn(&[f64]) -> Result<ElementTabulation, ElementError>,
{
    let ed = tdim(sub.entity);
    let n = sub.dofs.len();
    let reoriented = apply_generator(sub.entity, generator, sub.points);
    let cell_pts = sub_entity_points_to_cell(cell, ed, sub.index, &reoriented);
    let tab = tabulate_values(&cell_pts)?;
    let mut matrix = Vec::with_capacity(n * n);
    for i in 0..n {
        let row = &tab.values[i * dim..(i + 1) * dim];
        for &dof in sub.dofs {
            matrix.push(row[dof]);
        }
    }
    Ok(matrix)
}

/// Computes and classifies all base transformations of a validated Lagrange element.
pub(crate) fn build_lagrange_transformations<F>(
    key: ElementKey,
    dim: usize,
    entity_dofs: &[Vec<Vec<usize>>],
    tabulate_values: F,
) -> Result<BaseTransformations, ElementError>
where
    F: Fn(&[f64]) -> Result<ElementTabulation, ElementError>,
{
    debug_assert_eq!(key.family, Family::Lagrange);
    let mut tables: [Option<BaseTransformation>; 5] = [None, None, None, None, None];

    let Variant::Nodal(family) = key.variant else {
        return Ok(BaseTransformations {
            kind: TransformationKind::Identity,
            tables,
        });
    };
    if key.degree == 0 {
        return Ok(BaseTransformations {
            kind: TransformationKind::Identity,
            tables,
        });
    }

    let td = tdim(key.cell);
    let mut cached_entity: Option<(CellType, Vec<f64>)> = None;

    for (entity, generator) in ENTITY_GENERATORS {
        let ed = tdim(entity);
        if ed >= td {
            continue;
        }
        let Some(e) = sub_entity_types(key.cell, ed)
            .iter()
            .position(|&ty| ty == entity)
        else {
            continue;
        };
        let dofs = &entity_dofs[ed][e];
        if dofs.is_empty() {
            continue;
        }

        let entity_pts = match &cached_entity {
            Some((c, pts)) if *c == entity => pts.as_slice(),
            _ => {
                let all_pts = lagrange_points(entity, key.degree, family)?;
                let interior = lagrange_entity_nodes(entity, key.degree, ed, 0);
                let slice = all_pts[interior.start * ed..interior.end * ed].to_vec();
                debug_assert_eq!(slice.len(), dofs.len() * ed);
                cached_entity = Some((entity, slice));
                cached_entity.as_ref().unwrap().1.as_slice()
            }
        };

        let sub = SubEntityRef {
            entity,
            index: e,
            dofs,
            points: entity_pts,
        };
        let matrix = compute_nodal_matrix(key.cell, dim, &tabulate_values, sub, generator)?;
        let bt = classify_matrix(matrix, dofs.len());
        assert!(
            matches!(
                &bt,
                BaseTransformation::SignedPermutation { negate, .. } if negate.iter().all(|&b| !b)
            ),
            "Lagrange base transformation for {key:?} ({entity:?}, {generator:?}) was not a pure permutation"
        );
        let idx = slot(entity, generator).expect("ENTITY_GENERATORS entry has a slot");
        tables[idx] = Some(bt);
    }

    let kind = classify_kind(&tables);
    Ok(BaseTransformations { kind, tables })
}

#[cfg(test)]
mod tests {
    use tetraloma_dims::CellType;
    use tetraloma_refcell::{
        NodeFamily, facet_points_to_cell, lagrange_entity_nodes, lagrange_points, sub_entity_types,
        tdim,
    };

    use super::*;
    use crate::{Element, ElementKey, Family, Variant};

    fn lagrange(cell: CellType, degree: u32, variant: Variant) -> ElementKey {
        ElementKey {
            family: Family::Lagrange,
            cell,
            degree,
            variant,
        }
    }

    #[test]
    fn sub_entity_map_matches_facet_map_and_vertices() {
        for cell in [
            CellType::Interval,
            CellType::Triangle,
            CellType::Quadrilateral,
            CellType::Tetrahedron,
            CellType::Hexahedron,
            CellType::Prism,
            CellType::Pyramid,
        ] {
            let td = tdim(cell);
            for (v, &coord) in vertices(cell).iter().enumerate() {
                assert_eq!(sub_entity_points_to_cell(cell, 0, v, &[]), coord);
            }
            for (f, &fty) in sub_entity_types(cell, td - 1).iter().enumerate() {
                let pts: &[f64] = match tdim(fty) {
                    0 => &[],
                    1 => &[0.0, 0.25, 0.7, 1.0],
                    2 => &[0.1, 0.2, 0.4, 0.3],
                    _ => unreachable!(),
                };
                assert_eq!(
                    sub_entity_points_to_cell(cell, td - 1, f, pts),
                    facet_points_to_cell(cell, f, pts),
                    "{cell:?} facet {f}"
                );
            }
        }
    }

    #[test]
    fn classify_matrix_and_kind_cover_all_variants_and_nan() {
        let id = classify_matrix(vec![1.0, 0.0, 0.0, 1.0], 2);
        assert_eq!(
            id,
            BaseTransformation::SignedPermutation {
                perm: vec![0, 1],
                negate: vec![false, false],
            }
        );
        assert_eq!(
            classify_kind(&[Some(id.clone()), None, None, None, None]),
            TransformationKind::Identity
        );

        let neg1 = classify_matrix(vec![-1.0 + 1e-12], 1);
        assert_eq!(
            neg1,
            BaseTransformation::SignedPermutation {
                perm: vec![0],
                negate: vec![true],
            }
        );
        assert_eq!(
            classify_kind(&[Some(neg1), None, None, None, None]),
            TransformationKind::SignedPermutation
        );

        let sp = classify_matrix(vec![1e-11, -1.0 + 1e-11, 1.0 - 1e-11, -1e-11], 2);
        assert_eq!(
            sp,
            BaseTransformation::SignedPermutation {
                perm: vec![1, 0],
                negate: vec![true, false],
            }
        );
        assert_eq!(
            classify_kind(&[Some(id), Some(sp.clone()), None, None, None]),
            TransformationKind::SignedPermutation
        );

        let s = 3.0_f64.sqrt() * 0.5;
        let rot = vec![-0.5, -s, s, -0.5];
        let dense = classify_matrix(rot.clone(), 2);
        assert_eq!(dense, BaseTransformation::Dense { n: 2, matrix: rot });
        assert_eq!(
            classify_kind(&[Some(sp), Some(dense), None, None, None]),
            TransformationKind::Dense
        );

        // Threshold boundary: just below SIGNED_PERM_TOL classifies as SignedPermutation;
        // just above (on diagonal or off-diagonal) classifies as Dense.
        let eps_ok = 0.5 * SIGNED_PERM_TOL;
        let eps_bad = 2.0 * SIGNED_PERM_TOL;
        assert_eq!(
            classify_matrix(vec![eps_ok, -1.0 + eps_ok, 1.0 - eps_ok, -eps_ok], 2),
            BaseTransformation::SignedPermutation {
                perm: vec![1, 0],
                negate: vec![true, false],
            }
        );
        for bad in [
            vec![0.0, -1.0 + eps_bad, 1.0, 0.0],
            vec![eps_bad, -1.0, 1.0, 0.0],
            vec![1.0, 0.0, 1.0, 0.0],
            vec![1.0, f64::NAN, 0.0, 1.0],
            vec![f64::NAN, 0.0, 0.0, 1.0],
        ] {
            assert!(matches!(
                classify_matrix(bad, 2),
                BaseTransformation::Dense { n: 2, .. }
            ));
        }
    }

    #[test]
    fn inapplicable_pairs_return_none_and_generators_obey_group_relations() {
        let tet4 = Element::get(lagrange(
            CellType::Tetrahedron,
            4,
            Variant::Nodal(NodeFamily::Gll),
        ))
        .unwrap();
        for (entity, generator) in [
            (CellType::Point, Generator::Reversal),
            (CellType::Interval, Generator::Rotation),
            (CellType::Interval, Generator::Reflection),
            (CellType::Triangle, Generator::Reversal),
            (CellType::Quadrilateral, Generator::Reversal),
            (CellType::Quadrilateral, Generator::Rotation),
            (CellType::Quadrilateral, Generator::Reflection),
            (CellType::Tetrahedron, Generator::Rotation),
        ] {
            assert!(tet4.base_transformation(entity, generator).is_none());
        }

        let pow = |perm: &[u32], k: usize| -> Vec<u32> {
            let mut cur: Vec<u32> = (0..perm.len() as u32).collect();
            for _ in 0..k {
                cur = perm.iter().map(|&i| cur[i as usize]).collect();
            }
            cur
        };

        for (cell, family, degree) in [
            (CellType::Triangle, NodeFamily::Equispaced, 15),
            (CellType::Triangle, NodeFamily::Gll, 15),
            (CellType::Quadrilateral, NodeFamily::Equispaced, 20),
            (CellType::Quadrilateral, NodeFamily::Gll, 20),
            (CellType::Tetrahedron, NodeFamily::Equispaced, 15),
            (CellType::Tetrahedron, NodeFamily::Gll, 15),
            (CellType::Hexahedron, NodeFamily::Equispaced, 20),
            (CellType::Hexahedron, NodeFamily::Gll, 20),
            (CellType::Prism, NodeFamily::Equispaced, 15),
            (CellType::Prism, NodeFamily::Gll, 15),
            (CellType::Pyramid, NodeFamily::Equispaced, 8),
        ] {
            let el = Element::get(lagrange(cell, degree, Variant::Nodal(family))).unwrap();
            let td = tdim(cell);
            let Some(BaseTransformation::SignedPermutation { perm: rev, .. }) =
                el.base_transformation(CellType::Interval, Generator::Reversal)
            else {
                panic!("{cell:?} p={degree}: expected SignedPermutation for edge reversal");
            };
            assert_eq!(pow(rev, 2), (0..(degree - 1)).collect::<Vec<_>>());

            if td == 3 {
                for (face_ty, order) in [(CellType::Triangle, 3), (CellType::Quadrilateral, 4)] {
                    if let (
                        Some(BaseTransformation::SignedPermutation { perm: r, .. }),
                        Some(BaseTransformation::SignedPermutation { perm: s, .. }),
                    ) = (
                        el.base_transformation(face_ty, Generator::Rotation),
                        el.base_transformation(face_ty, Generator::Reflection),
                    ) {
                        let id_f: Vec<u32> = (0..r.len() as u32).collect();
                        let rs: Vec<u32> = r.iter().map(|&i| s[i as usize]).collect();
                        assert_eq!(pow(r, order), id_f, "{cell:?} {face_ty:?} R^{order}");
                        assert_eq!(pow(s, 2), id_f, "{cell:?} {face_ty:?} S^2");
                        assert_eq!(pow(&rs, 2), id_f, "{cell:?} {face_ty:?} (RS)^2");
                    }
                }
            }

            // Every sub-entity of the same type produces the exact same base transformation.
            for (entity, generator) in ENTITY_GENERATORS {
                let ed = tdim(entity);
                let Some(want) = el.base_transformation(entity, generator) else {
                    continue;
                };
                let all_pts = lagrange_points(entity, degree, family).unwrap();
                let interior = lagrange_entity_nodes(entity, degree, ed, 0);
                let epts = &all_pts[interior.start * ed..interior.end * ed];
                for (e, &ty) in sub_entity_types(cell, ed).iter().enumerate() {
                    if ty != entity {
                        continue;
                    }
                    let dofs = el.entity_dofs(ed, e);
                    let sub = SubEntityRef {
                        entity,
                        index: e,
                        dofs,
                        points: epts,
                    };
                    let mat = compute_nodal_matrix(
                        cell,
                        el.dim(),
                        &|pts| el.tabulate(pts, 0),
                        sub,
                        generator,
                    )
                    .unwrap();
                    assert_eq!(&classify_matrix(mat, dofs.len()), want);
                }
            }
        }
    }
}

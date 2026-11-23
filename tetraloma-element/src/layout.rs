// Design: section 26 ("Finite elements"), table of supported elements and DOF layouts; ADR-0071,
// ADR-0072.
//! Supported-key validation and sub-entity DOF layouts.

use tetraloma_dims::CellType;
use tetraloma_refcell::{
    NodeFamily, lagrange_entity_nodes, num_lagrange_nodes, num_sub_entities, sub_entities, tdim,
};

use crate::{ElementError, ElementKey, Family, Variant};

/// Maximum supported Lagrange degree for `cell`, or `None` for `CellType::Point`.
pub(crate) fn lagrange_max_degree(cell: CellType) -> Option<u32> {
    match cell {
        CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron => Some(20),
        CellType::Triangle | CellType::Tetrahedron | CellType::Prism => Some(15),
        CellType::Pyramid => Some(8),
        CellType::Point => None,
    }
}

/// Checks `key` against the table of supported elements.
pub(crate) fn validate_key(key: ElementKey) -> Result<(), ElementError> {
    let unsupported = || ElementError::Unsupported {
        family: key.family,
        cell: key.cell,
        degree: key.degree,
        variant: key.variant,
    };

    match key.family {
        Family::Lagrange => {
            let Some(max_p) = lagrange_max_degree(key.cell) else {
                return Err(unsupported());
            };
            if key.degree > max_p {
                return Err(unsupported());
            }
            match key.variant {
                Variant::Nodal(NodeFamily::Equispaced) | Variant::Legendre => Ok(()),
                Variant::Nodal(NodeFamily::Gll) if key.cell != CellType::Pyramid => Ok(()),
                Variant::Nodal(_) => Err(unsupported()),
            }
        }
        // N1curl and RaviartThomas are implemented in M12.
        Family::N1curl | Family::RaviartThomas => Err(unsupported()),
    }
}

/// Combinatorial DOF layout of one element.
pub(crate) struct DofLayout {
    pub(crate) dim: usize,
    pub(crate) discontinuous_only: bool,
    pub(crate) entity_dofs: Vec<Vec<Vec<usize>>>,
    pub(crate) entity_closure_dofs: Vec<Vec<Vec<usize>>>,
}

/// Builds the DOF layout of a validated Lagrange element key.
pub(crate) fn build_lagrange_layout(key: ElementKey) -> DofLayout {
    let (cell, degree) = (key.cell, key.degree);
    let td = tdim(cell);
    let dim = num_lagrange_nodes(cell, degree);
    let discontinuous_only = matches!(key.variant, Variant::Legendre) || degree == 0;

    let entity_dofs: Vec<Vec<Vec<usize>>> = (0..=td)
        .map(|d| {
            (0..num_sub_entities(cell, d))
                .map(|e| {
                    if discontinuous_only {
                        if d == td {
                            (0..dim).collect()
                        } else {
                            Vec::new()
                        }
                    } else {
                        lagrange_entity_nodes(cell, degree, d, e).collect()
                    }
                })
                .collect()
        })
        .collect();

    let entity_closure_dofs = build_closure_dofs(cell, &entity_dofs);
    DofLayout {
        dim,
        discontinuous_only,
        entity_dofs,
        entity_closure_dofs,
    }
}

/// Computes `entity_closure_dofs` from `entity_dofs` in ascending `(sub_d, sub_e)` order.
fn build_closure_dofs(cell: CellType, entity_dofs: &[Vec<Vec<usize>>]) -> Vec<Vec<Vec<usize>>> {
    let td = tdim(cell);
    (0..=td)
        .map(|d| {
            sub_entities(cell, d)
                .iter()
                .enumerate()
                .map(|(e, &verts)| {
                    let mut closure = Vec::new();
                    for (sub_d, sub_dofs) in entity_dofs.iter().enumerate().take(d) {
                        for (sub_e, &sub_verts) in sub_entities(cell, sub_d).iter().enumerate() {
                            if sub_verts.iter().all(|v| verts.contains(v)) {
                                closure.extend_from_slice(&sub_dofs[sub_e]);
                            }
                        }
                    }
                    closure.extend_from_slice(&entity_dofs[d][e]);
                    closure
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use tetraloma_dims::CellType;
    use tetraloma_refcell::{
        NodeFamily, facet_points_to_cell, lagrange_points, num_lagrange_nodes, num_sub_entities,
        sub_entity_types, tdim,
    };

    use super::*;

    const ALL_CELLS: [CellType; 7] = [
        CellType::Interval,
        CellType::Triangle,
        CellType::Quadrilateral,
        CellType::Tetrahedron,
        CellType::Hexahedron,
        CellType::Prism,
        CellType::Pyramid,
    ];

    fn make_key(family: Family, cell: CellType, degree: u32, variant: Variant) -> ElementKey {
        ElementKey {
            family,
            cell,
            degree,
            variant,
        }
    }

    #[test]
    fn validate_key_accepts_full_lagrange_table_and_rejects_boundaries() {
        let variants = [
            Variant::Nodal(NodeFamily::Equispaced),
            Variant::Nodal(NodeFamily::Gll),
            Variant::Legendre,
        ];
        for cell in ALL_CELLS {
            let max_p = lagrange_max_degree(cell).unwrap();
            for variant in variants {
                let gll_pyramid =
                    cell == CellType::Pyramid && variant == Variant::Nodal(NodeFamily::Gll);
                for degree in [0, 1, max_p] {
                    let k = make_key(Family::Lagrange, cell, degree, variant);
                    assert_eq!(validate_key(k).is_ok(), !gll_pyramid, "{k:?}");
                }
                assert!(
                    validate_key(make_key(Family::Lagrange, cell, max_p + 1, variant)).is_err()
                );
            }
        }
        for family in [Family::N1curl, Family::RaviartThomas] {
            for cell in [
                CellType::Triangle,
                CellType::Quadrilateral,
                CellType::Tetrahedron,
                CellType::Hexahedron,
            ] {
                let k = make_key(family, cell, 1, Variant::Legendre);
                assert_eq!(
                    validate_key(k),
                    Err(ElementError::Unsupported {
                        family,
                        cell,
                        degree: 1,
                        variant: Variant::Legendre,
                    })
                );
            }
        }
    }

    /// Independent literal dimensions at degrees 0, 1, 2, 3 and `max_p` for each cell.
    const EXPECTED_DIMS: [(CellType, u32, [usize; 5]); 7] = [
        (CellType::Interval, 20, [1, 2, 3, 4, 21]),
        (CellType::Triangle, 15, [1, 3, 6, 10, 136]),
        (CellType::Quadrilateral, 20, [1, 4, 9, 16, 441]),
        (CellType::Tetrahedron, 15, [1, 4, 10, 20, 816]),
        (CellType::Hexahedron, 20, [1, 8, 27, 64, 9261]),
        (CellType::Prism, 15, [1, 6, 18, 40, 2176]),
        (CellType::Pyramid, 8, [1, 5, 14, 30, 285]),
    ];

    #[test]
    fn lagrange_layouts_partition_dofs_and_match_sub_entity_counts() {
        for (cell, max_p, want_dims) in EXPECTED_DIMS {
            let td = tdim(cell);
            assert_eq!(lagrange_max_degree(cell), Some(max_p));
            for variant in [Variant::Nodal(NodeFamily::Equispaced), Variant::Legendre] {
                for (&degree, &want_dim) in [0, 1, 2, 3, max_p].iter().zip(&want_dims) {
                    let k = make_key(Family::Lagrange, cell, degree, variant);
                    let layout = build_lagrange_layout(k);
                    let want_disc = matches!(variant, Variant::Legendre) || degree == 0;
                    assert_eq!(layout.discontinuous_only, want_disc, "{k:?}");
                    assert_eq!(layout.dim, want_dim, "{k:?}");

                    let flat: Vec<usize> = (0..=td)
                        .flat_map(|d| (0..num_sub_entities(cell, d)).map(move |e| (d, e)))
                        .flat_map(|(d, e)| layout.entity_dofs[d][e].iter().copied())
                        .collect();
                    let expected: Vec<usize> = (0..layout.dim).collect();
                    assert_eq!(flat, expected, "{k:?}: entity_dofs partition");
                    assert_eq!(layout.entity_closure_dofs[td][0], expected, "{k:?}");

                    for d in 0..td {
                        for (e, &ety) in sub_entity_types(cell, d).iter().enumerate() {
                            if want_disc {
                                assert!(layout.entity_dofs[d][e].is_empty(), "{k:?}");
                                assert!(layout.entity_closure_dofs[d][e].is_empty(), "{k:?}");
                            } else {
                                assert_eq!(
                                    layout.entity_closure_dofs[d][e].len(),
                                    num_lagrange_nodes(ety, degree),
                                    "{k:?} ({d}, {e}) {ety:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn facet_closure_dofs_match_mapped_facet_lagrange_points() {
        let eq = NodeFamily::Equispaced;
        for cell in ALL_CELLS {
            let td = tdim(cell);
            for degree in 1..=5 {
                let layout = build_lagrange_layout(make_key(
                    Family::Lagrange,
                    cell,
                    degree,
                    Variant::Nodal(eq),
                ));
                let cell_pts = lagrange_points(cell, degree, eq).unwrap();
                for (f, &fty) in sub_entity_types(cell, td - 1).iter().enumerate() {
                    let closure = &layout.entity_closure_dofs[td - 1][f];
                    let mapped =
                        facet_points_to_cell(cell, f, &lagrange_points(fty, degree, eq).unwrap());
                    assert_eq!(mapped.len(), closure.len() * td);
                    for (slot, &dof) in closure.iter().enumerate() {
                        let got = &cell_pts[dof * td..(dof + 1) * td];
                        let want = &mapped[slot * td..(slot + 1) * td];
                        assert!(
                            got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-14),
                            "{cell:?} p={degree} facet {f} slot {slot} dof {dof}: {got:?} vs {want:?}"
                        );
                    }
                }
            }
        }
    }
}

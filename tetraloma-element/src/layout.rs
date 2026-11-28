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

/// Maximum supported degree for `Family::N1curl` and `Family::RaviartThomas` on `cell`, or `None`
/// for cells that do not offer those families.
#[cfg_attr(not(test), expect(dead_code))] // wired in M12.T7
pub(crate) fn hcurl_hdiv_max_degree(cell: CellType) -> Option<u32> {
    match cell {
        CellType::Triangle | CellType::Tetrahedron => Some(15),
        CellType::Quadrilateral | CellType::Hexahedron => Some(20),
        CellType::Point | CellType::Interval | CellType::Prism | CellType::Pyramid => None,
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

/// Number of interior DOFs on each sub-entity of dimension `d` for a validated `N1curl` or
/// `RaviartThomas` key.
#[cfg_attr(not(test), expect(dead_code))] // wired in M12.T7
fn hcurl_hdiv_entity_dof_count(family: Family, cell: CellType, degree: u32, d: usize) -> usize {
    let p = degree as usize;
    match (family, cell, d) {
        (_, _, 0) => 0,
        (
            Family::N1curl,
            CellType::Triangle
            | CellType::Quadrilateral
            | CellType::Tetrahedron
            | CellType::Hexahedron,
            1,
        )
        | (Family::RaviartThomas, CellType::Triangle | CellType::Quadrilateral, 1) => p,
        (Family::RaviartThomas, CellType::Tetrahedron | CellType::Hexahedron, 1) => 0,
        (Family::N1curl | Family::RaviartThomas, CellType::Triangle, 2)
        | (Family::N1curl, CellType::Tetrahedron, 2) => p * (p - 1),
        (Family::N1curl | Family::RaviartThomas, CellType::Quadrilateral, 2)
        | (Family::N1curl, CellType::Hexahedron, 2) => 2 * p * (p - 1),
        (Family::RaviartThomas, CellType::Tetrahedron, 2) => p * (p + 1) / 2,
        (Family::RaviartThomas, CellType::Hexahedron, 2) => p * p,
        (Family::N1curl, CellType::Tetrahedron, 3) => p * (p - 1) * p.saturating_sub(2) / 2,
        (Family::N1curl, CellType::Hexahedron, 3) => 3 * p * (p - 1) * (p - 1),
        (Family::RaviartThomas, CellType::Tetrahedron, 3) => p * (p - 1) * (p + 1) / 2,
        (Family::RaviartThomas, CellType::Hexahedron, 3) => 3 * (p - 1) * p * p,
        _ => unreachable!("unsupported ({family:?}, {cell:?}, d={d})"),
    }
}

/// Builds the DOF layout of a validated `N1curl` or `RaviartThomas` element key.
///
/// DOFs are assigned contiguously to sub-entities in ascending `(d, e)` order and partition
/// `0..dim`.
#[cfg_attr(not(test), expect(dead_code))] // wired in M12.T7
pub(crate) fn build_hcurl_hdiv_layout(key: ElementKey) -> DofLayout {
    assert!(
        matches!(key.family, Family::N1curl | Family::RaviartThomas)
            && key.variant == Variant::Legendre
            && hcurl_hdiv_max_degree(key.cell)
                .is_some_and(|max_p| key.degree >= 1 && key.degree <= max_p),
        "build_hcurl_hdiv_layout: unsupported key {key:?}"
    );
    let td = tdim(key.cell);
    let mut next_dof = 0usize;
    let entity_dofs: Vec<Vec<Vec<usize>>> = (0..=td)
        .map(|d| {
            let count = hcurl_hdiv_entity_dof_count(key.family, key.cell, key.degree, d);
            (0..num_sub_entities(key.cell, d))
                .map(|_| {
                    let start = next_dof;
                    next_dof += count;
                    (start..next_dof).collect()
                })
                .collect()
        })
        .collect();
    let entity_closure_dofs = build_closure_dofs(key.cell, &entity_dofs);
    DofLayout {
        dim: next_dof,
        discontinuous_only: false,
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
    fn sub_entity_closure_dofs_match_mapped_lagrange_points() {
        use tetraloma_refcell::vertices;

        let eq = NodeFamily::Equispaced;
        for cell in ALL_CELLS {
            let td = tdim(cell);
            let verts = vertices(cell);
            for degree in 1..=5 {
                let layout = build_lagrange_layout(make_key(
                    Family::Lagrange,
                    cell,
                    degree,
                    Variant::Nodal(eq),
                ));
                let cell_pts = lagrange_points(cell, degree, eq).unwrap();

                // Edges (d = 1) for every 2D and 3D cell: checks edges below facet dimension too.
                if td >= 2 {
                    let edge_pts = lagrange_points(CellType::Interval, degree, eq).unwrap();
                    for (e, &ev) in sub_entities(cell, 1).iter().enumerate() {
                        let closure = &layout.entity_closure_dofs[1][e];
                        let (v0, v1) = (verts[ev[0]], verts[ev[1]]);
                        assert_eq!(closure.len(), edge_pts.len());
                        for (slot, (&dof, &t)) in closure.iter().zip(&edge_pts).enumerate() {
                            let got = &cell_pts[dof * td..(dof + 1) * td];
                            for j in 0..td {
                                let want = v0[j] + t * (v1[j] - v0[j]);
                                assert!(
                                    (got[j] - want).abs() < 1e-14,
                                    "{cell:?} p={degree} edge {e} slot {slot} dof {dof}: {got:?}"
                                );
                            }
                        }
                    }
                }

                // Facets (d = td - 1) against `facet_points_to_cell`.
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

        // Literal check for a 3D cell edge below facet dimension (tetrahedron p = 2, edge 5 = (2, 3)).
        let tet2 = build_lagrange_layout(make_key(
            Family::Lagrange,
            CellType::Tetrahedron,
            2,
            Variant::Nodal(eq),
        ));
        assert_eq!(tet2.entity_closure_dofs[1][5], &[2, 3, 9]);
    }

    #[test]
    fn hcurl_hdiv_max_degree_and_layouts_cover_all_supported_cells_and_degrees() {
        assert_eq!(hcurl_hdiv_max_degree(CellType::Triangle), Some(15));
        assert_eq!(hcurl_hdiv_max_degree(CellType::Tetrahedron), Some(15));
        assert_eq!(hcurl_hdiv_max_degree(CellType::Quadrilateral), Some(20));
        assert_eq!(hcurl_hdiv_max_degree(CellType::Hexahedron), Some(20));
        for cell in [
            CellType::Point,
            CellType::Interval,
            CellType::Prism,
            CellType::Pyramid,
        ] {
            assert_eq!(hcurl_hdiv_max_degree(cell), None, "{cell:?}");
        }

        let cells = [
            CellType::Triangle,
            CellType::Quadrilateral,
            CellType::Tetrahedron,
            CellType::Hexahedron,
        ];
        for family in [Family::N1curl, Family::RaviartThomas] {
            for cell in cells {
                let (td, max_p) = (tdim(cell), hcurl_hdiv_max_degree(cell).unwrap());
                for degree in 1..=max_p {
                    let p = degree as usize;
                    let key = make_key(family, cell, degree, Variant::Legendre);
                    let layout = build_hcurl_hdiv_layout(key);
                    assert!(!layout.discontinuous_only, "{key:?}");
                    let want_dim = match (family, cell) {
                        (Family::N1curl | Family::RaviartThomas, CellType::Triangle) => p * (p + 2),
                        (Family::N1curl | Family::RaviartThomas, CellType::Quadrilateral) => {
                            2 * p * (p + 1)
                        }
                        (Family::N1curl, CellType::Tetrahedron) => p * (p + 2) * (p + 3) / 2,
                        (Family::RaviartThomas, CellType::Tetrahedron) => p * (p + 1) * (p + 3) / 2,
                        (Family::N1curl, CellType::Hexahedron) => 3 * p * (p + 1) * (p + 1),
                        (Family::RaviartThomas, CellType::Hexahedron) => 3 * p * p * (p + 1),
                        _ => unreachable!(),
                    };
                    assert_eq!(layout.dim, want_dim, "{key:?}");

                    let flat: Vec<usize> = (0..=td)
                        .flat_map(|d| layout.entity_dofs[d].iter().flatten().copied())
                        .collect();
                    let expected: Vec<usize> = (0..layout.dim).collect();
                    assert_eq!(flat, expected, "{key:?}");
                    assert_eq!(layout.entity_closure_dofs[td][0], expected, "{key:?}");
                    assert!(layout.entity_dofs[0].iter().all(Vec::is_empty));
                    assert!(layout.entity_closure_dofs[0].iter().all(Vec::is_empty));

                    let want_edge = usize::from(!(family == Family::RaviartThomas && td == 3)) * p;
                    for (ed, ec) in layout.entity_dofs[1]
                        .iter()
                        .zip(&layout.entity_closure_dofs[1])
                    {
                        assert_eq!(ed.len(), want_edge, "{key:?}");
                        assert_eq!(ec, ed, "{key:?}");
                    }
                    if td == 3 {
                        for (f, &fty) in sub_entity_types(cell, 2).iter().enumerate() {
                            let c = &layout.entity_closure_dofs[2][f];
                            assert!(c.windows(2).all(|w| w[0] < w[1]), "{key:?} f={f}");
                            let want_c = match (family, fty) {
                                (Family::N1curl, CellType::Triangle) => p * (p + 2),
                                (Family::N1curl, CellType::Quadrilateral) => 2 * p * (p + 1),
                                (Family::RaviartThomas, CellType::Triangle) => p * (p + 1) / 2,
                                (Family::RaviartThomas, CellType::Quadrilateral) => p * p,
                                _ => unreachable!(),
                            };
                            assert_eq!(c.len(), want_c, "{key:?} f={f}");
                            if family == Family::RaviartThomas {
                                assert_eq!(c, &layout.entity_dofs[2][f], "{key:?} f={f}");
                            }
                        }
                    }
                }
            }
        }
        // Literal spot checks at low degrees and upper bounds.
        for (fam, cell, deg, want) in [
            (Family::N1curl, CellType::Triangle, 1, 3),
            (Family::N1curl, CellType::Triangle, 15, 255),
            (Family::N1curl, CellType::Quadrilateral, 20, 840),
            (Family::N1curl, CellType::Tetrahedron, 1, 6),
            (Family::N1curl, CellType::Tetrahedron, 15, 2295),
            (Family::N1curl, CellType::Hexahedron, 20, 26_460),
            (Family::RaviartThomas, CellType::Tetrahedron, 1, 4),
            (Family::RaviartThomas, CellType::Tetrahedron, 15, 2160),
            (Family::RaviartThomas, CellType::Hexahedron, 20, 25_200),
        ] {
            let l = build_hcurl_hdiv_layout(make_key(fam, cell, deg, Variant::Legendre));
            assert_eq!(l.dim, want, "{fam:?} {cell:?} p={deg}");
        }
    }
}

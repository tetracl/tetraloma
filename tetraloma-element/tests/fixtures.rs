//! M11 and M12 acceptance tests against committed oracle fixtures from ndelement, Basix and
//! Symfem (docs/design.md, A.10, A.11; docs/plans/M11.md, A1, A2, A9; docs/plans/M12.md, A1).
//!
//! Expected JSON schemas:
//!
//! - `lagrange_ndelement.json` and `lagrange_basix.json` (M11.T1):
//!   `{ "<Cell>": { "points": [f64; 8 * tdim], "<Variant>": { "<p>": [f64; (1 + tdim) * 8 * dim] } } }`
//!   with tabulation entries in `[derivative][point][basis]` order. Basix files use Basix's DOF
//!   order, converted here with Appendix A.11.
//! - `base_transformations_ndelement.json` (M11.T1):
//!   `{ "<Cell>": { "<Variant>": { "<p>": { "<Entity>": { "<Transform>": { "kind": "Identity" } | { "kind": "Permutation", "perm": [u32; n] } } } } } }`
//!   where `perm` is expanded from ndelement's swap list so that `(M * x)[i] = x[perm[i]]`.
//! - `hcurl_hdiv_dofs_symfem.json` (M12.T1):
//!   `{ "<Family>": { "<Cell>": { "<p>": [[usize; num_sub_entities(cell, d)]; tdim + 1] } } }`
//!   giving Symfem's sub-entity DOF counts in DefElement numbering (identical to tetraloma's,
//!   Appendix A.11) for `N1curl` and `RaviartThomas` on `Triangle`, `Quadrilateral`,
//!   `Tetrahedron` and `Hexahedron`, `p = 1..=8`.

use serde_json::{Map, Value};
use tetraloma_dims::CellType;
use tetraloma_element::{BaseTransformation, Element, ElementKey, Family, Generator, Variant};
use tetraloma_refcell::{
    NodeFamily, lagrange_entity_nodes, num_sub_entities, sub_entity_types, tdim,
};

const TOL: f64 = 1e-12;
const NUM_FIXTURE_POINTS: usize = 8;
const NDELEMENT_CELLS: [&str; 5] = [
    "Hexahedron",
    "Interval",
    "Quadrilateral",
    "Tetrahedron",
    "Triangle",
];

fn fixture(file: &str) -> Value {
    let path = format!("{}/tests/fixtures/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

fn cell(name: &str) -> CellType {
    use CellType::*;
    [
        Point,
        Interval,
        Triangle,
        Quadrilateral,
        Tetrahedron,
        Hexahedron,
        Prism,
        Pyramid,
    ]
    .into_iter()
    .find(|c| format!("{c:?}") == name)
    .unwrap_or_else(|| panic!("unknown cell type {name}"))
}

fn node_family(name: &str) -> NodeFamily {
    match name {
        "Equispaced" => NodeFamily::Equispaced,
        "Gll" => NodeFamily::Gll,
        other => panic!("unknown node family {other}"),
    }
}

fn sorted_keys(obj: &Map<String, Value>) -> Vec<&str> {
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

fn sorted_degrees(obj: &Map<String, Value>) -> Vec<u32> {
    let mut ps: Vec<u32> = obj.keys().map(|d| d.parse().unwrap()).collect();
    ps.sort_unstable();
    ps
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

fn u32s(v: &Value) -> Vec<u32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| u32::try_from(x.as_u64().unwrap()).unwrap())
        .collect()
}

fn ndelement_variants(c: CellType) -> &'static [&'static str] {
    if matches!(
        c,
        CellType::Interval | CellType::Quadrilateral | CellType::Hexahedron
    ) {
        &["Equispaced", "Gll"]
    } else {
        &["Equispaced"]
    }
}

/// Appendix A.11: Basix sub-entity index to tetraloma sub-entity index (an involution).
fn a11_entity(cell: CellType, dim: usize, basix_e: usize) -> usize {
    match (cell, dim) {
        (CellType::Triangle, 1) => 2 - basix_e,
        (CellType::Tetrahedron, 1) => 5 - basix_e,
        (CellType::Tetrahedron, 2) => 3 - basix_e,
        _ => basix_e,
    }
}

fn basix_to_tetraloma_dofs(cell: CellType, degree: u32) -> Vec<usize> {
    let mut map = Vec::new();
    for dim in 0..=tdim(cell) {
        for basix_e in 0..num_sub_entities(cell, dim) {
            map.extend(lagrange_entity_nodes(
                cell,
                degree,
                dim,
                a11_entity(cell, dim, basix_e),
            ));
        }
    }
    map
}

fn check_tabulation(key: ElementKey, points: &[f64], want: &[f64], dof_map: Option<&[usize]>) {
    let td = tdim(key.cell);
    assert_eq!(
        points.len(),
        NUM_FIXTURE_POINTS * td,
        "{key:?}: fixture points"
    );

    let el = Element::get(key).unwrap();
    let dim = el.dim();
    let tab = el.tabulate(points, 1).unwrap();
    assert_eq!(
        (tab.nderivs, tab.num_points, tab.dim, tab.value_size),
        (1, NUM_FIXTURE_POINTS, dim, 1)
    );
    assert_eq!(
        want.len(),
        (1 + td) * NUM_FIXTURE_POINTS * dim,
        "{key:?}: fixture length"
    );
    assert_eq!(tab.values.len(), want.len(), "{key:?}: tabulation length");
    if let Some(map) = dof_map {
        assert_eq!(map.len(), dim, "{key:?}: A.11 map length");
    }

    let block = NUM_FIXTURE_POINTS * dim;
    for d in 0..=td {
        let base = d * block;
        let scale = want[base..base + block]
            .iter()
            .fold(1.0_f64, |m, v| m.max(v.abs()));
        for q in 0..NUM_FIXTURE_POINTS {
            for k in 0..dim {
                let tetra_k = dof_map.map_or(k, |m| m[k]);
                let got = tab.values[base + q * dim + tetra_k];
                let expected = want[base + q * dim + k];
                let err = (got - expected).abs() / scale;
                assert!(
                    err <= TOL,
                    "{key:?} deriv {d} pt {q} dof {k} (tetra {tetra_k}): {got} vs {expected} (rel err {err:e})"
                );
            }
        }
    }
}

/// M11 acceptance A1: for `Nodal(Equispaced)` Lagrange on intervals, triangles, quadrilaterals,
/// tetrahedra and hexahedra and `Nodal(Gll)` on intervals, quadrilaterals and hexahedra,
/// p = 1–6, values and first derivatives at the fixture points equal ndelement's to 1e-12.
#[test]
fn lagrange_tabulation_matches_ndelement() {
    let data = fixture("lagrange_ndelement.json");
    let cells = data.as_object().unwrap();
    assert_eq!(sorted_keys(cells), NDELEMENT_CELLS);

    for (name, entry) in cells {
        let c = cell(name);
        let obj = entry.as_object().unwrap();
        let points = floats(&obj["points"]);
        let variants: Vec<&str> = sorted_keys(obj)
            .into_iter()
            .filter(|&k| k != "points")
            .collect();
        assert_eq!(variants, ndelement_variants(c), "variants of {name}");

        for vname in variants {
            let nf = node_family(vname);
            let degrees = obj[vname].as_object().unwrap();
            let ps = sorted_degrees(degrees);
            assert_eq!(ps, [1, 2, 3, 4, 5, 6], "degrees of {name} {vname}");
            for p in ps {
                let key = ElementKey {
                    family: Family::Lagrange,
                    cell: c,
                    degree: p,
                    variant: Variant::Nodal(nf),
                };
                check_tabulation(key, &points, &floats(&degrees[&p.to_string()]), None);
            }
        }
    }
}

/// M11 acceptance A2: for `Nodal(Gll)` on triangles, tetrahedra and prisms and
/// `Nodal(Equispaced)` on prisms (p = 1–6) and pyramids (p = 1–3), values and first derivatives
/// at the fixture points equal Basix's after the A.11 conversion to 1e-12.
#[test]
fn lagrange_tabulation_matches_basix() {
    let data = fixture("lagrange_basix.json");
    let cells = data.as_object().unwrap();
    assert_eq!(
        sorted_keys(cells),
        ["Prism", "Pyramid", "Tetrahedron", "Triangle"]
    );

    for (name, entry) in cells {
        let c = cell(name);
        let obj = entry.as_object().unwrap();
        let points = floats(&obj["points"]);
        let variants: Vec<&str> = sorted_keys(obj)
            .into_iter()
            .filter(|&k| k != "points")
            .collect();
        let (want_variants, want_ps): (&[&str], &[u32]) = match c {
            CellType::Triangle | CellType::Tetrahedron => (&["Gll"], &[1, 2, 3, 4, 5, 6]),
            CellType::Prism => (&["Equispaced", "Gll"], &[1, 2, 3, 4, 5, 6]),
            CellType::Pyramid => (&["Equispaced"], &[1, 2, 3]),
            other => panic!("unexpected cell {other:?} in Basix fixture"),
        };
        assert_eq!(variants, want_variants, "variants of {name}");

        for vname in variants {
            let nf = node_family(vname);
            let degrees = obj[vname].as_object().unwrap();
            let ps = sorted_degrees(degrees);
            assert_eq!(ps, want_ps, "degrees of {name} {vname}");
            for p in ps {
                let key = ElementKey {
                    family: Family::Lagrange,
                    cell: c,
                    degree: p,
                    variant: Variant::Nodal(nf),
                };
                let dof_map = basix_to_tetraloma_dofs(c, p);
                check_tabulation(
                    key,
                    &points,
                    &floats(&degrees[&p.to_string()]),
                    Some(&dof_map),
                );
            }
        }
    }
}

fn map_ndelement_generator(entity_name: &str, transform_name: &str) -> (CellType, Generator) {
    let entity = cell(entity_name);
    let generator = match (entity, transform_name) {
        (CellType::Interval, "Reflection") => Generator::Reversal,
        (CellType::Triangle | CellType::Quadrilateral, "Rotation") => Generator::Rotation,
        (CellType::Triangle | CellType::Quadrilateral, "Reflection") => Generator::Reflection,
        _ => panic!("unexpected ndelement transformation ({entity_name}, {transform_name})"),
    };
    (entity, generator)
}

fn dofs_on_entity_type(el: &Element, entity: CellType) -> usize {
    let d = tdim(entity);
    if d >= tdim(el.key().cell) {
        return 0;
    }
    sub_entity_types(el.key().cell, d)
        .iter()
        .enumerate()
        .find(|(_, ty)| **ty == entity)
        .map_or(0, |(e, _)| el.entity_dofs(d, e).len())
}

/// M11 acceptance A9: for p <= 6, the base transformations of Lagrange elements on intervals,
/// triangles, quadrilaterals, tetrahedra and hexahedra equal ndelement's.
#[test]
fn lagrange_base_transformations_match_ndelement() {
    let data = fixture("base_transformations_ndelement.json");
    let cells = data.as_object().unwrap();
    assert_eq!(sorted_keys(cells), NDELEMENT_CELLS);

    for (name, variants_val) in cells {
        let c = cell(name);
        let variants_obj = variants_val.as_object().unwrap();
        let variants = sorted_keys(variants_obj);
        assert_eq!(variants, ndelement_variants(c), "variants of {name}");

        let expected_entities: &[&str] = match c {
            CellType::Interval => &[],
            CellType::Triangle | CellType::Quadrilateral => &["Interval"],
            CellType::Tetrahedron => &["Interval", "Triangle"],
            CellType::Hexahedron => &["Interval", "Quadrilateral"],
            other => panic!("unexpected cell {other:?}"),
        };

        for vname in variants {
            let nf = node_family(vname);
            let degrees = variants_obj[vname].as_object().unwrap();
            let ps = sorted_degrees(degrees);
            assert_eq!(ps, [1, 2, 3, 4, 5, 6], "degrees of {name} {vname}");

            for p in ps {
                let key = ElementKey {
                    family: Family::Lagrange,
                    cell: c,
                    degree: p,
                    variant: Variant::Nodal(nf),
                };
                let el = Element::get(key).unwrap();
                let entities = degrees[&p.to_string()].as_object().unwrap();
                assert_eq!(
                    sorted_keys(entities),
                    expected_entities,
                    "{key:?}: entities"
                );

                for (entity_name, transforms_val) in entities {
                    let transforms = transforms_val.as_object().unwrap();
                    let want_t: &[&str] = if entity_name == "Interval" {
                        &["Reflection"]
                    } else {
                        &["Reflection", "Rotation"]
                    };
                    assert_eq!(
                        sorted_keys(transforms),
                        want_t,
                        "{key:?} {entity_name}: transforms"
                    );

                    for (tname, spec) in transforms {
                        let (entity, generator) = map_ndelement_generator(entity_name, tname);
                        let ndofs = dofs_on_entity_type(&el, entity);
                        let ours = el.base_transformation(entity, generator);
                        match spec["kind"].as_str().unwrap() {
                            "Identity" if ndofs == 0 => assert!(
                                ours.is_none(),
                                "{key:?} ({entity:?}, {generator:?}): expected None when entity has 0 DOFs"
                            ),
                            "Identity" => match ours {
                                Some(BaseTransformation::SignedPermutation { perm, negate }) => {
                                    let want_perm: Vec<u32> = (0..ndofs as u32).collect();
                                    assert_eq!(
                                        perm, &want_perm,
                                        "{key:?} ({entity:?}, {generator:?}): expected identity perm"
                                    );
                                    assert!(
                                        negate.len() == ndofs && negate.iter().all(|&b| !b),
                                        "{key:?} ({entity:?}, {generator:?}): unexpected negation"
                                    );
                                }
                                _ => panic!(
                                    "{key:?} ({entity:?}, {generator:?}): expected identity SignedPermutation"
                                ),
                            },
                            "Permutation" => {
                                let want_perm = u32s(&spec["perm"]);
                                assert_eq!(want_perm.len(), ndofs, "{key:?}: fixture perm length");
                                match ours {
                                    Some(BaseTransformation::SignedPermutation {
                                        perm,
                                        negate,
                                    }) => {
                                        assert_eq!(
                                            perm, &want_perm,
                                            "{key:?} ({entity:?}, {generator:?}): permutation mismatch"
                                        );
                                        assert!(
                                            negate.len() == ndofs && negate.iter().all(|&b| !b),
                                            "{key:?} ({entity:?}, {generator:?}): unexpected negation"
                                        );
                                    }
                                    _ => panic!(
                                        "{key:?} ({entity:?}, {generator:?}): expected SignedPermutation"
                                    ),
                                }
                            }
                            other => {
                                panic!("{key:?} ({entity_name}, {tname}): unknown kind {other}")
                            }
                        }
                    }
                }
            }
        }
    }
}

/// M12 acceptance A1: for N1curl and RT on triangles, quadrilaterals, tetrahedra and hexahedra,
/// p = 1–8, the number of DOFs on each sub-entity equals Symfem's.
#[test]
#[ignore = "pending M12.T7"]
fn hcurl_hdiv_dofs_match_symfem() {
    let data = fixture("hcurl_hdiv_dofs_symfem.json");
    let families = data.as_object().unwrap();
    assert_eq!(sorted_keys(families), ["N1curl", "RaviartThomas"]);

    for (fname, cells_val) in families {
        let family = match fname.as_str() {
            "N1curl" => Family::N1curl,
            "RaviartThomas" => Family::RaviartThomas,
            other => panic!("unexpected family {other}"),
        };
        let cells = cells_val.as_object().unwrap();
        assert_eq!(
            sorted_keys(cells),
            ["Hexahedron", "Quadrilateral", "Tetrahedron", "Triangle"],
            "cells of {fname}"
        );

        for (cname, degrees_val) in cells {
            let c = cell(cname);
            let td = tdim(c);
            let degrees = degrees_val.as_object().unwrap();
            let ps = sorted_degrees(degrees);
            assert_eq!(ps, [1, 2, 3, 4, 5, 6, 7, 8], "degrees of {fname} {cname}");

            for p in ps {
                let key = ElementKey {
                    family,
                    cell: c,
                    degree: p,
                    variant: Variant::Legendre,
                };
                let el = Element::get(key).unwrap();
                let by_dim = degrees[&p.to_string()].as_array().unwrap();
                assert_eq!(by_dim.len(), td + 1, "{key:?}: fixture dim count");
                let mut total = 0usize;
                for (d, dim_val) in by_dim.iter().enumerate() {
                    let counts = dim_val.as_array().unwrap();
                    assert_eq!(
                        counts.len(),
                        num_sub_entities(c, d),
                        "{key:?} dim {d}: entity count"
                    );
                    for (e, c_val) in counts.iter().enumerate() {
                        let want = usize::try_from(c_val.as_u64().unwrap()).unwrap();
                        assert_eq!(
                            el.entity_dofs(d, e).len(),
                            want,
                            "{key:?} entity ({d}, {e})"
                        );
                        total += want;
                    }
                }
                assert_eq!(el.dim(), total, "{key:?}: total dim");
            }
        }
    }
}

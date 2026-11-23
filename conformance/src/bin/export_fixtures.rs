//! Exports ndelement's reference data to `tetraloma-refcell/tests/fixtures/` and
//! `tetraloma-element/tests/fixtures/` (docs/design.md, A.10; docs/plans/M11.md, T1), so that
//! `cargo test --workspace` checks tetraloma against it without ndelement or BLAS.
//!
//! Run after every ndelement pin bump, then commit the fixtures with the new pin:
//!
//! ```text
//! cargo run --manifest-path conformance/Cargo.toml --bin export-fixtures
//! cargo run --manifest-path conformance/Cargo.toml --bin export-fixtures -- --check
//! ```
//!
//! `tetraloma-refcell/tests/fixtures/`:
//! - `tables.json`: vertices, sub-entities and sub-entity types of the eight cells (A.1–A.3);
//! - `orientation.json`: `compute_orientation` of every permutation of 0..V, in lexicographic
//!   order, for each cell with D_TOP ≥ 2 (A.6; docs/plans/M1b.md, question 5);
//! - `lagrange_points.json`: ndelement's interpolation points in DOF order for intervals,
//!   triangles, quadrilaterals, tetrahedra and hexahedra, p ≤ 4, equispaced and (tensor cells)
//!   GLL (A.5);
//! - `sources.json`: the pinned ndelement, Basix, DOLFINx and Symfem commits.
//!
//! `tetraloma-element/tests/fixtures/`:
//! - `lagrange_ndelement.json`: values and first derivatives at eight fixture points per cell
//!   for M11 A1 (`[derivative][point][basis]`);
//! - `base_transformations_ndelement.json`: base DOF transformations per sub-entity type and
//!   transformation for M11 A9, expanded so `(M * x)[i] = x[perm[i]]`;
//! - `sources.json`: the same pinned oracle commits.
//!
//! `--check` runs on a 64 MiB stack and compares `lagrange_ndelement.json` to `1e-13` (relative
//! per derivative block; points to `1e-15`) and every other file byte for byte.

use std::path::{Path, PathBuf};
use std::{env, fs, process};

use ndelement::ciarlet::{LagrangeVariant, lagrange};
use ndelement::orientation::compute_orientation;
use ndelement::reference_cell;
use ndelement::traits::{FiniteElement, MappedFiniteElement};
use ndelement::types::{Continuity, DofTransformation, Transformation};
use rlst::DynArray;
use serde_json::{Map, Value, json};
use tetraloma_conformance::{CELLS, reference_cell_type};
use tetraloma_dims::CellType;

/// The pinned commits of the other sources (docs/design.md, A.10; ADR-0007). ndelement's is
/// read from Cargo.lock, so it follows the pin.
const BASIX: (&str, &str) = ("v0.11.0", "8ba83f732cd9d83cec643fa9059d49afb4482485");
const DOLFINX: (&str, &str) = ("main", "be8352e677303b686d6862b70923bab086b279ff");
const SYMFEM: (&str, &str) = ("v2025.12.0", "c37ffd6951a7fddd99353335fd5a511efdcf18fc");

const STACK_BYTES: usize = 64 << 20;
const NUM_POINTS: usize = 8;
const POINT_TOL: f64 = 1e-15;
const TAB_TOL: f64 = 1e-13;
const CASES: [(CellType, bool); 5] = [
    (CellType::Interval, true),
    (CellType::Triangle, false),
    (CellType::Quadrilateral, true),
    (CellType::Tetrahedron, false),
    (CellType::Hexahedron, true),
];

fn name(cell: CellType) -> String {
    format!("{cell:?}")
}

/// Eight fixture points per cell (midpoint, interior, face on 3D, edge on >=2D, vertex).
/// Triangle and Tetrahedron match `scripts/fixtures/element/generate.py` verbatim.
fn fixture_points(cell: CellType) -> Vec<f64> {
    match cell {
        CellType::Interval => vec![0.5, 0.15, 0.31, 0.62, 0.83, 0.94, 0.0, 1.0],
        CellType::Triangle => [
            [1.0 / 3.0, 1.0 / 3.0],
            [0.17, 0.23],
            [0.53, 0.29],
            [0.11, 0.71],
            [0.37, 0.0],
            [0.0, 0.61],
            [0.4, 0.6],
            [1.0, 0.0],
        ]
        .into_iter()
        .flatten()
        .collect(),
        CellType::Quadrilateral => [
            [0.5, 0.5],
            [0.17, 0.23],
            [0.73, 0.31],
            [0.29, 0.83],
            [0.41, 0.0],
            [0.0, 0.67],
            [0.79, 1.0],
            [1.0, 1.0],
        ]
        .into_iter()
        .flatten()
        .collect(),
        CellType::Tetrahedron => [
            [0.25, 0.25, 0.25],
            [0.13, 0.19, 0.29],
            [0.47, 0.17, 0.11],
            [0.31, 0.43, 0.0],
            [0.2, 0.3, 0.5],
            [0.63, 0.0, 0.0],
            [0.0, 0.35, 0.65],
            [0.0, 0.0, 1.0],
        ]
        .into_iter()
        .flatten()
        .collect(),
        CellType::Hexahedron => [
            [0.5, 0.5, 0.5],
            [0.17, 0.23, 0.37],
            [0.79, 0.61, 0.19],
            [0.31, 0.73, 0.0],
            [1.0, 0.43, 0.81],
            [0.67, 0.0, 0.0],
            [0.29, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ]
        .into_iter()
        .flatten()
        .collect(),
        other => panic!("no ndelement fixture points for {other:?}"),
    }
}

fn tables() -> Value {
    let mut out = Map::new();
    for cell in CELLS {
        let nd = reference_cell_type(cell);
        let types = reference_cell::entity_types(nd);
        let counts = reference_cell::entity_counts(nd);
        let mut entities = vec![];
        let mut entity_types = vec![];
        for (dim, types) in types.iter().enumerate() {
            let list: Vec<Vec<usize>> = match dim {
                0 => (0..counts[0]).map(|v| vec![v]).collect(),
                1 => reference_cell::edges(nd),
                2 => reference_cell::faces(nd),
                _ => reference_cell::volumes(nd),
            };
            entities.push(json!(list));
            let names: Vec<String> = types.iter().map(|t| format!("{t:?}")).collect();
            entity_types.push(json!(names));
        }
        out.insert(
            name(cell),
            json!({
                "tdim": reference_cell::dim(nd),
                "vertices": reference_cell::vertices::<f64>(nd),
                "sub_entities": entities,
                "sub_entity_types": entity_types,
            }),
        );
    }
    Value::Object(out)
}

/// Advances `p` to the next permutation in lexicographic order; false after the last one.
fn next_permutation(p: &mut [usize]) -> bool {
    let Some(i) = (1..p.len()).rev().find(|&i| p[i - 1] < p[i]) else {
        return false;
    };
    let j = (i..p.len())
        .rev()
        .find(|&j| p[j] > p[i - 1])
        .expect("p[i] > p[i - 1]");
    p.swap(i - 1, j);
    p[i..].reverse();
    true
}

fn orientation() -> Value {
    let mut out = Map::new();
    for cell in CELLS
        .into_iter()
        .filter(|&c| tetraloma_refcell::tdim(c) >= 2)
    {
        let nd = reference_cell_type(cell);
        let nv = reference_cell::entity_counts(nd)[0];
        let mut ids: Vec<usize> = (0..nv).collect();
        let mut bits = vec![];
        loop {
            bits.push(compute_orientation(nd, &ids) as u32);
            if !next_permutation(&mut ids) {
                break;
            }
        }
        out.insert(name(cell), json!({ "num_vertices": nv, "bits": bits }));
    }
    Value::Object(out)
}

fn lagrange_points() -> Value {
    let mut out = Map::new();
    for (cell, tensor) in CASES {
        let mut variants = vec![("Equispaced", LagrangeVariant::Equispaced)];
        if tensor {
            variants.push(("Gll", LagrangeVariant::Gll));
        }
        let mut per_family = Map::new();
        for (family, variant) in variants {
            let mut per_degree = Map::new();
            for degree in 1..=4usize {
                let element = lagrange::create::<f64, f64>(
                    reference_cell_type(cell),
                    degree,
                    Continuity::Standard,
                    variant,
                );
                let td = tetraloma_refcell::tdim(cell);
                let mut points = vec![];
                for entities in element.interpolation_points() {
                    for block in entities {
                        for point in block.col_iter() {
                            points.extend((0..td).map(|j| point[[j]]));
                        }
                    }
                }
                per_degree.insert(degree.to_string(), json!(points));
            }
            per_family.insert(family.to_string(), Value::Object(per_degree));
        }
        out.insert(name(cell), Value::Object(per_family));
    }
    Value::Object(out)
}

/// Expands ndelement's swap list (`ndelement::math::apply_permutation`) into `perm` with
/// `(M * x)[i] = x[perm[i]]`.
fn expand_permutation(swaps: &[usize]) -> Vec<u32> {
    let mut perm: Vec<u32> = (0..u32::try_from(swaps.len()).expect("fits u32")).collect();
    for (i, &j) in swaps.iter().enumerate() {
        perm.swap(i, j);
    }
    let mut seen = vec![false; swaps.len()];
    for &p in &perm {
        assert!(!std::mem::replace(&mut seen[p as usize], true));
    }
    perm
}

/// Returns `(lagrange_ndelement, base_transformations_ndelement)` for M11 A1 and A9.
fn ndelement_elements() -> (Value, Value) {
    let (mut tab_out, mut bt_out) = (Map::new(), Map::new());
    for (cell, tensor) in CASES {
        let td = tetraloma_refcell::tdim(cell);
        let pts_flat = fixture_points(cell);
        let mut pts_arr = DynArray::<f64, 2>::from_shape([td, NUM_POINTS]);
        for q in 0..NUM_POINTS {
            for j in 0..td {
                pts_arr[[j, q]] = pts_flat[q * td + j];
            }
        }
        let entities: &[CellType] = match cell {
            CellType::Interval => &[],
            CellType::Triangle | CellType::Quadrilateral => &[CellType::Interval],
            CellType::Tetrahedron => &[CellType::Interval, CellType::Triangle],
            CellType::Hexahedron => &[CellType::Interval, CellType::Quadrilateral],
            other => panic!("unexpected cell {other:?}"),
        };
        let mut variants = vec![("Equispaced", LagrangeVariant::Equispaced)];
        if tensor {
            variants.push(("Gll", LagrangeVariant::Gll));
        }
        let mut tab_cell = Map::from_iter([("points".to_string(), json!(pts_flat))]);
        let mut bt_cell = Map::new();

        for (family, variant) in variants {
            let (mut tab_degs, mut bt_degs) = (Map::new(), Map::new());
            for degree in 1..=6usize {
                let el = lagrange::create::<f64, f64>(
                    reference_cell_type(cell),
                    degree,
                    Continuity::Standard,
                    variant,
                );
                let dim = el.dim();
                assert_eq!(el.value_size(), 1);
                let shape = el.tabulate_array_shape(1, NUM_POINTS);
                assert_eq!(shape, [NUM_POINTS, 1, 1 + td, dim]);
                let mut data = DynArray::<f64, 4>::from_shape(shape);
                el.tabulate(&pts_arr, 1, &mut data);

                let mut flat = Vec::with_capacity((1 + td) * NUM_POINTS * dim);
                for d in 0..=td {
                    let start = flat.len();
                    for q in 0..NUM_POINTS {
                        for k in 0..dim {
                            let v = data[[q, 0, d, k]];
                            assert!(v.is_finite(), "{cell:?} {family} p={degree}: non-finite");
                            flat.push(v);
                        }
                    }
                    let block = &flat[start..];
                    let scale = block.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                    for q in 0..NUM_POINTS {
                        let sum: f64 = block[q * dim..(q + 1) * dim].iter().sum();
                        let want = if d == 0 { 1.0 } else { 0.0 };
                        assert!(
                            (sum - want).abs() / scale < 1e-11,
                            "{cell:?} {family} p={degree}"
                        );
                    }
                }
                tab_degs.insert(degree.to_string(), json!(flat));

                let mut bt_ents = Map::new();
                for &entity in entities {
                    let transforms: &[(&str, Transformation)] = if entity == CellType::Interval {
                        &[("Reflection", Transformation::Reflection)]
                    } else {
                        &[
                            ("Reflection", Transformation::Reflection),
                            ("Rotation", Transformation::Rotation),
                        ]
                    };
                    let mut bt_tr = Map::new();
                    for &(tname, tr) in transforms {
                        let spec = match el.dof_transformation(reference_cell_type(entity), tr) {
                            Some(DofTransformation::Identity) => json!({ "kind": "Identity" }),
                            Some(DofTransformation::Permutation(swaps)) => {
                                json!({ "kind": "Permutation", "perm": expand_permutation(swaps) })
                            }
                            other => panic!("{cell:?} {family} p={degree} {entity:?}: {other:?}"),
                        };
                        bt_tr.insert(tname.to_string(), spec);
                    }
                    bt_ents.insert(name(entity), Value::Object(bt_tr));
                }
                bt_degs.insert(degree.to_string(), Value::Object(bt_ents));
            }
            tab_cell.insert(family.to_string(), Value::Object(tab_degs));
            bt_cell.insert(family.to_string(), Value::Object(bt_degs));
        }
        tab_out.insert(name(cell), Value::Object(tab_cell));
        bt_out.insert(name(cell), Value::Object(bt_cell));
    }
    (Value::Object(tab_out), Value::Object(bt_out))
}

/// ndelement's git revision, from the `source` line of its Cargo.lock entry.
fn ndelement_rev(manifest_dir: &Path) -> String {
    let lock = fs::read_to_string(manifest_dir.join("Cargo.lock")).expect("read Cargo.lock");
    let mut lines = lock.lines().skip_while(|l| *l != "name = \"ndelement\"");
    let source = lines
        .find(|l| l.starts_with("source = "))
        .expect("ndelement has a source line in Cargo.lock");
    source
        .rsplit('#')
        .next()
        .expect("a git source ends in #<rev>")
        .trim_end_matches('"')
        .to_string()
}

fn sources(manifest_dir: &Path) -> Value {
    let pin = |(tag, commit): (&str, &str), repo: &str| json!({ "repository": repo, "ref": tag, "commit": commit });
    json!({
        "ndelement": {
            "repository": "https://codeberg.org/nd-project/nd",
            "ref": "main",
            "commit": ndelement_rev(manifest_dir),
        },
        "basix": pin(BASIX, "https://github.com/FEniCS/basix"),
        "dolfinx": pin(DOLFINX, "https://github.com/FEniCS/dolfinx"),
        "symfem": pin(SYMFEM, "https://github.com/mscroggs/symfem"),
    })
}

fn serialise(value: &Value, pretty: bool) -> String {
    let mut text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .expect("serialise");
    text.push('\n');
    text
}

fn same_keys(a: &Map<String, Value>, b: &Map<String, Value>) -> bool {
    a.len() == b.len() && a.keys().all(|k| b.contains_key(k))
}

fn floats(v: &Value) -> Option<Vec<f64>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_f64().filter(|f| f.is_finite()))
        .collect()
}

fn compare_blocks(
    label: &str,
    have: &Value,
    want: &Value,
    nblocks: usize,
    tol: f64,
    errs: &mut Vec<String>,
) {
    let (Some(h), Some(w)) = (floats(have), floats(want)) else {
        errs.push(format!("{label}: invalid float array"));
        return;
    };
    if h.len() != w.len() || !w.len().is_multiple_of(nblocks) {
        errs.push(format!("{label}: length {} vs {}", h.len(), w.len()));
        return;
    }
    let blen = w.len() / nblocks;
    for d in 0..nblocks {
        let (hb, wb) = (&h[d * blen..(d + 1) * blen], &w[d * blen..(d + 1) * blen]);
        let scale = if nblocks == 1 {
            1.0
        } else {
            wb.iter().fold(1.0_f64, |m, v| m.max(v.abs()))
        };
        let worst = hb
            .iter()
            .zip(wb)
            .map(|(a, b)| (a - b).abs() / scale)
            .fold(0.0_f64, f64::max);
        if worst > tol {
            errs.push(format!("{label} block {d}: diff {worst:e} > {tol:e}"));
        }
    }
}

fn compare_tabulations_json(have: &Value, want: &Value, errs: &mut Vec<String>) {
    let (Some(hc), Some(wc)) = (have.as_object(), want.as_object()) else {
        errs.push("lagrange_ndelement.json: not an object".to_string());
        return;
    };
    if !same_keys(hc, wc) {
        errs.push("lagrange_ndelement.json: cell keys differ".to_string());
        return;
    }
    for (cname, wval) in wc {
        let td = CELLS
            .into_iter()
            .find(|c| name(*c) == *cname)
            .map(tetraloma_refcell::tdim)
            .expect("known cell");
        let (Some(ho), Some(wo)) = (hc[cname].as_object(), wval.as_object()) else {
            errs.push(format!("{cname}: not an object"));
            continue;
        };
        if !same_keys(ho, wo) {
            errs.push(format!("{cname}: keys differ"));
            continue;
        }
        compare_blocks(
            &format!("{cname} points"),
            &ho["points"],
            &wo["points"],
            1,
            POINT_TOL,
            errs,
        );
        for (vname, wdegs) in wo {
            if vname == "points" {
                continue;
            }
            let (Some(hd), Some(wd)) = (ho[vname].as_object(), wdegs.as_object()) else {
                errs.push(format!("{cname} {vname}: not an object"));
                continue;
            };
            if !same_keys(hd, wd) {
                errs.push(format!("{cname} {vname}: degree keys differ"));
                continue;
            }
            for (deg, wtab) in wd {
                compare_blocks(
                    &format!("{cname} {vname} p={deg}"),
                    &hd[deg],
                    wtab,
                    1 + td,
                    TAB_TOL,
                    errs,
                );
            }
        }
    }
}

fn run(check: bool) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let refcell_dir = manifest_dir.join("../tetraloma-refcell/tests/fixtures");
    let element_dir = manifest_dir.join("../tetraloma-element/tests/fixtures");

    let (tables_val, orient_val, pts_val) = (tables(), orientation(), lagrange_points());
    let sources_val = sources(&manifest_dir);
    let (tab_val, bt_val) = ndelement_elements();
    let exact: [(&Path, &str, &Value, bool); 6] = [
        (&refcell_dir, "tables.json", &tables_val, true),
        (&refcell_dir, "orientation.json", &orient_val, false),
        (&refcell_dir, "lagrange_points.json", &pts_val, false),
        (&refcell_dir, "sources.json", &sources_val, true),
        (
            &element_dir,
            "base_transformations_ndelement.json",
            &bt_val,
            false,
        ),
        (&element_dir, "sources.json", &sources_val, true),
    ];

    if !check {
        fs::create_dir_all(&refcell_dir).expect("create refcell fixture directory");
        fs::create_dir_all(&element_dir).expect("create element fixture directory");
        for (dir, file, val, pretty) in exact {
            let path = dir.join(file);
            fs::write(&path, serialise(val, pretty)).expect("write fixture");
            println!("wrote {}", path.display());
        }
        let tab_path = element_dir.join("lagrange_ndelement.json");
        fs::write(&tab_path, serialise(&tab_val, false)).expect("write lagrange_ndelement.json");
        println!("wrote {}", tab_path.display());
        return;
    }

    let mut errs = Vec::new();
    for (dir, file, val, pretty) in exact {
        let path = dir.join(file);
        match fs::read_to_string(&path) {
            Ok(have) if have == serialise(val, pretty) => {}
            Ok(_) => errs.push(format!("{}: differs byte-for-byte", path.display())),
            Err(e) => errs.push(format!("{}: {e}", path.display())),
        }
    }
    let tab_path = element_dir.join("lagrange_ndelement.json");
    match fs::read_to_string(&tab_path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()))
    {
        Ok(have) => compare_tabulations_json(&have, &tab_val, &mut errs),
        Err(e) => errs.push(format!("{}: {e}", tab_path.display())),
    }
    if !errs.is_empty() {
        eprintln!("ndelement fixtures differ from a fresh export:");
        for err in &errs {
            eprintln!("  {err}");
        }
        process::exit(1);
    }
    println!(
        "all 7 ndelement fixtures match a fresh export (6 byte-identical, tabulations <= 1e-13)"
    );
}

fn main() {
    let check = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            eprintln!("usage: export-fixtures [--check]");
            process::exit(2);
        }
    };
    std::thread::Builder::new()
        .name("export-fixtures".to_string())
        .stack_size(STACK_BYTES)
        .spawn(move || run(check))
        .expect("spawn export thread")
        .join()
        .expect("export thread panicked");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndelement::math::{apply_permutation, prepare_permutation};

    #[test]
    fn expand_permutation_matches_apply_permutation() {
        let original = vec![1usize, 4, 3, 0, 6, 5, 2];
        let mut swaps = original.clone();
        prepare_permutation(&mut swaps);
        let expanded = expand_permutation(&swaps);
        assert_eq!(
            expanded,
            original.iter().map(|&i| i as u32).collect::<Vec<_>>()
        );
        let data = vec![10, 20, 30, 40, 50, 60, 70];
        let mut applied = data.clone();
        apply_permutation(&swaps, &mut applied);
        assert_eq!(
            applied,
            expanded
                .iter()
                .map(|&p| data[p as usize])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn compare_tabulations_respects_tolerance_and_structure() {
        let want =
            json!({"Interval": {"points": [0.5], "Equispaced": {"1": [0.5, 0.5, -40.0, 40.0]}}});
        let ok = json!({"Interval": {"points": [0.5 + 5e-16], "Equispaced": {"1": [0.5 + 5e-14, 0.5, -40.0 + 2e-12, 40.0]}}});
        let bad_val = json!({"Interval": {"points": [0.5], "Equispaced": {"1": [0.5 + 2e-13, 0.5, -40.0, 40.0]}}});
        let bad_deriv = json!({"Interval": {"points": [0.5], "Equispaced": {"1": [0.5, 0.5, -40.0 + 1e-11, 40.0]}}});
        let bad_pt = json!({"Interval": {"points": [0.5 + 3e-15], "Equispaced": {"1": [0.5, 0.5, -40.0, 40.0]}}});
        let mut errs = Vec::new();
        compare_tabulations_json(&ok, &want, &mut errs);
        assert!(errs.is_empty(), "{errs:?}");
        for case in [&bad_val, &bad_deriv, &bad_pt] {
            errs.clear();
            compare_tabulations_json(case, &want, &mut errs);
            assert_eq!(errs.len(), 1, "{case}: {errs:?}");
        }
        errs.clear();
        compare_tabulations_json(&json!({"Interval": {"points": [0.5]}}), &want, &mut errs);
        assert!(!errs.is_empty());
    }
}

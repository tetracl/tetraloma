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
//! - `lagrange_ndelement.json`: values and first derivatives at eight fixture points per cell for
//!   `Nodal(Equispaced)` on intervals, triangles, quadrilaterals, tetrahedra and hexahedra and
//!   `Nodal(Gll)` on intervals, quadrilaterals and hexahedra, p = 1..=6 (M11 A1);
//! - `base_transformations_ndelement.json`: base DOF transformations per sub-entity type and
//!   transformation for the same elements, with permutations expanded from ndelement's swap lists
//!   so that `(M * x)[i] = x[perm[i]]` (M11 A9);
//! - `sources.json`: the same pinned oracle commits.
//!
//! `--check` regenerates in memory on a 64 MiB stack and compares `lagrange_ndelement.json` to
//! `1e-13` (relative per derivative block; points to `1e-15`) and every other file byte for byte.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

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

/// Thread stack size: ndelement construction in debug builds with OpenBLAS overflows 2 MiB
/// (docs/plans/M11.md, T1).
const STACK_BYTES: usize = 64 << 20;

const NUM_FIXTURE_POINTS: usize = 8;
const POINT_TOL: f64 = 1e-15;
const TAB_TOL: f64 = 1e-13;

const NDELEMENT_CASES: [(CellType, bool); 5] = [
    (CellType::Interval, true),
    (CellType::Triangle, false),
    (CellType::Quadrilateral, true),
    (CellType::Tetrahedron, false),
    (CellType::Hexahedron, true),
];

fn name(cell: CellType) -> String {
    format!("{cell:?}")
}

/// Eight fixture points per cell (midpoint, interior, face on 3D, edge on >=2D, vertex), flat
/// point-major. Triangle and Tetrahedron match `scripts/fixtures/element/generate.py` verbatim.
fn fixture_points(cell: CellType) -> Vec<f64> {
    match cell {
        CellType::Interval => vec![
            0.5,  // midpoint
            0.15, // interior
            0.31, // interior
            0.62, // interior
            0.83, // interior
            0.94, // interior
            0.0,  // vertex 0
            1.0,  // vertex 1
        ],
        CellType::Triangle => vec![
            1.0 / 3.0,
            1.0 / 3.0, // midpoint
            0.17,
            0.23, // interior
            0.53,
            0.29, // interior
            0.11,
            0.71, // interior
            0.37,
            0.0, // edge 0 (y = 0)
            0.0,
            0.61, // edge 1 (x = 0)
            0.4,
            0.6, // edge 2 (x + y = 1)
            1.0,
            0.0, // vertex 1
        ],
        CellType::Quadrilateral => vec![
            0.5, 0.5, // midpoint
            0.17, 0.23, // interior
            0.73, 0.31, // interior
            0.29, 0.83, // interior
            0.41, 0.0, // edge 0 (y = 0)
            0.0, 0.67, // edge 1 (x = 0)
            0.79, 1.0, // edge 3 (y = 1)
            1.0, 1.0, // vertex 3
        ],
        CellType::Tetrahedron => vec![
            0.25, 0.25, 0.25, // midpoint
            0.13, 0.19, 0.29, // interior
            0.47, 0.17, 0.11, // interior
            0.31, 0.43, 0.0, // face 0 (z = 0)
            0.2, 0.3, 0.5, // face 3 (x + y + z = 1)
            0.63, 0.0, 0.0, // edge 0 (y = z = 0)
            0.0, 0.35, 0.65, // edge 5 (x = 0, y + z = 1)
            0.0, 0.0, 1.0, // vertex 3
        ],
        CellType::Hexahedron => vec![
            0.5, 0.5, 0.5, // midpoint
            0.17, 0.23, 0.37, // interior
            0.79, 0.61, 0.19, // interior
            0.31, 0.73, 0.0, // face 0 (z = 0)
            1.0, 0.43, 0.81, // face 3 (x = 1)
            0.67, 0.0, 0.0, // edge 0 (y = z = 0)
            0.29, 1.0, 1.0, // edge 11 (y = z = 1)
            1.0, 1.0, 1.0, // vertex 7
        ],
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
    for (cell, tensor) in NDELEMENT_CASES {
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

/// Expands ndelement's in-place swap list (`ndelement::math::apply_permutation`) into a direct
/// permutation array `perm` such that `(M * x)[i] = x[perm[i]]`.
fn expand_permutation(swaps: &[usize]) -> Vec<u32> {
    let n = swaps.len();
    let mut perm: Vec<u32> = (0..u32::try_from(n).expect("perm fits in u32")).collect();
    for (i, &j) in swaps.iter().enumerate() {
        perm.swap(i, j);
    }
    let mut seen = vec![false; n];
    for &p in &perm {
        let idx = p as usize;
        assert!(idx < n && !seen[idx], "invalid permutation {perm:?}");
        seen[idx] = true;
    }
    perm
}

/// Returns `(lagrange_ndelement, base_transformations_ndelement)` for the elements of M11 A1 and A9.
fn ndelement_elements() -> (Value, Value) {
    let mut tab_out = Map::new();
    let mut bt_out = Map::new();

    for (cell, tensor) in NDELEMENT_CASES {
        let td = tetraloma_refcell::tdim(cell);
        let pts_flat = fixture_points(cell);
        assert_eq!(pts_flat.len(), NUM_FIXTURE_POINTS * td);

        let mut pts_arr = DynArray::<f64, 2>::from_shape([td, NUM_FIXTURE_POINTS]);
        for q in 0..NUM_FIXTURE_POINTS {
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

        let mut tab_cell = Map::new();
        tab_cell.insert("points".to_string(), json!(pts_flat));
        let mut bt_cell = Map::new();

        for (family, variant) in variants {
            let mut tab_degs = Map::new();
            let mut bt_degs = Map::new();

            for degree in 1..=6usize {
                let element = lagrange::create::<f64, f64>(
                    reference_cell_type(cell),
                    degree,
                    Continuity::Standard,
                    variant,
                );
                let dim = element.dim();
                assert_eq!(element.value_size(), 1);

                let shape = element.tabulate_array_shape(1, NUM_FIXTURE_POINTS);
                assert_eq!(shape, [NUM_FIXTURE_POINTS, 1, 1 + td, dim]);
                let mut data = DynArray::<f64, 4>::from_shape(shape);
                element.tabulate(&pts_arr, 1, &mut data);

                // Flat [derivative][point][basis].
                let mut flat = Vec::with_capacity((1 + td) * NUM_FIXTURE_POINTS * dim);
                for d in 0..=td {
                    let mut max_abs = 1.0_f64;
                    let start = flat.len();
                    for q in 0..NUM_FIXTURE_POINTS {
                        let mut sum = 0.0_f64;
                        for k in 0..dim {
                            let v = data[[q, 0, d, k]];
                            assert!(v.is_finite(), "{cell:?} {family} p={degree}: non-finite");
                            max_abs = max_abs.max(v.abs());
                            sum += v;
                            flat.push(v);
                        }
                        if d == 0 {
                            assert!(
                                (sum - 1.0).abs() < 1e-11,
                                "{cell:?} {family} p={degree} pt {q}: sum(phi)-1 = {}",
                                sum - 1.0
                            );
                        }
                    }
                    if d > 0 {
                        for q in 0..NUM_FIXTURE_POINTS {
                            let row = &flat[start + q * dim..start + (q + 1) * dim];
                            let sum: f64 = row.iter().sum();
                            assert!(
                                sum.abs() / max_abs < 1e-11,
                                "{cell:?} {family} p={degree} deriv {d} pt {q}: sum(dphi)/scale = {}",
                                sum.abs() / max_abs
                            );
                        }
                    }
                }
                tab_degs.insert(degree.to_string(), json!(flat));

                let mut bt_entities = Map::new();
                for &entity in entities {
                    let transforms: &[(&str, Transformation)] = match entity {
                        CellType::Interval => &[("Reflection", Transformation::Reflection)],
                        CellType::Triangle | CellType::Quadrilateral => &[
                            ("Reflection", Transformation::Reflection),
                            ("Rotation", Transformation::Rotation),
                        ],
                        other => panic!("unexpected entity {other:?}"),
                    };
                    let mut bt_transforms = Map::new();
                    for &(tname, transform) in transforms {
                        let spec = match element
                            .dof_transformation(reference_cell_type(entity), transform)
                        {
                            Some(DofTransformation::Identity) => json!({ "kind": "Identity" }),
                            Some(DofTransformation::Permutation(swaps)) => {
                                let perm = expand_permutation(swaps);
                                json!({ "kind": "Permutation", "perm": perm })
                            }
                            Some(DofTransformation::Transformation(..)) => panic!(
                                "{cell:?} {family} p={degree} {entity:?} {tname}: unexpected dense transformation"
                            ),
                            None => panic!(
                                "{cell:?} {family} p={degree} {entity:?} {tname}: missing dof_transformation"
                            ),
                        };
                        bt_transforms.insert(tname.to_string(), spec);
                    }
                    bt_entities.insert(name(entity), Value::Object(bt_transforms));
                }
                bt_degs.insert(degree.to_string(), Value::Object(bt_entities));
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

fn write(dir: &Path, file: &str, value: &Value, pretty: bool) {
    let text = serialise(value, pretty);
    let path = dir.join(file);
    fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("wrote {}", path.display());
}

fn check_exact(dir: &Path, file: &str, value: &Value, pretty: bool, errors: &mut Vec<String>) {
    let path = dir.join(file);
    let want = serialise(value, pretty);
    match fs::read_to_string(&path) {
        Ok(have) if have == want => {}
        Ok(_) => errors.push(format!(
            "{}: differs byte-for-byte from a fresh export",
            path.display()
        )),
        Err(e) => errors.push(format!("{}: {e}", path.display())),
    }
}

fn cell_tdim_by_name(name: &str) -> Option<usize> {
    CELLS
        .into_iter()
        .find(|c| format!("{c:?}") == name)
        .map(tetraloma_refcell::tdim)
}

fn float_slice(v: &Value) -> Option<Vec<f64>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for x in arr {
        let f = x.as_f64()?;
        if !f.is_finite() {
            return None;
        }
        out.push(f);
    }
    Some(out)
}

fn compare_tabulations_json(have: &Value, want: &Value, errors: &mut Vec<String>) {
    let (Some(have_cells), Some(want_cells)) = (have.as_object(), want.as_object()) else {
        errors.push("lagrange_ndelement.json: top level is not an object".to_string());
        return;
    };
    let mut have_keys: Vec<&str> = have_cells.keys().map(String::as_str).collect();
    let mut want_keys: Vec<&str> = want_cells.keys().map(String::as_str).collect();
    have_keys.sort_unstable();
    want_keys.sort_unstable();
    if have_keys != want_keys {
        errors.push(format!(
            "lagrange_ndelement.json: cell keys differ (have {have_keys:?}, want {want_keys:?})"
        ));
        return;
    }

    for (cell_name, want_entry) in want_cells {
        let Some(td) = cell_tdim_by_name(cell_name) else {
            errors.push(format!("lagrange_ndelement.json: unknown cell {cell_name}"));
            continue;
        };
        let (Some(have_obj), Some(want_obj)) =
            (have_cells[cell_name].as_object(), want_entry.as_object())
        else {
            errors.push(format!(
                "lagrange_ndelement.json {cell_name}: not an object"
            ));
            continue;
        };
        let mut hk: Vec<&str> = have_obj.keys().map(String::as_str).collect();
        let mut wk: Vec<&str> = want_obj.keys().map(String::as_str).collect();
        hk.sort_unstable();
        wk.sort_unstable();
        if hk != wk {
            errors.push(format!(
                "lagrange_ndelement.json {cell_name}: keys differ (have {hk:?}, want {wk:?})"
            ));
            continue;
        }

        // Points: compare to POINT_TOL.
        match (
            float_slice(&have_obj["points"]),
            float_slice(&want_obj["points"]),
        ) {
            (Some(hp), Some(wp)) if hp.len() == wp.len() => {
                for (i, (a, b)) in hp.iter().zip(&wp).enumerate() {
                    let err = (a - b).abs();
                    if err > POINT_TOL {
                        errors.push(format!(
                            "lagrange_ndelement.json {cell_name} points[{i}]: diff {err:e} > {POINT_TOL:e}"
                        ));
                        break;
                    }
                }
            }
            _ => errors.push(format!(
                "lagrange_ndelement.json {cell_name}: points mismatch"
            )),
        }

        // Tabulations per variant and degree: compare per derivative block to TAB_TOL.
        for (variant, want_degs) in want_obj {
            if variant == "points" {
                continue;
            }
            let (Some(have_degs), Some(want_degs)) =
                (have_obj[variant].as_object(), want_degs.as_object())
            else {
                errors.push(format!(
                    "lagrange_ndelement.json {cell_name} {variant}: not an object"
                ));
                continue;
            };
            let mut hd: Vec<&str> = have_degs.keys().map(String::as_str).collect();
            let mut wd: Vec<&str> = want_degs.keys().map(String::as_str).collect();
            hd.sort_unstable();
            wd.sort_unstable();
            if hd != wd {
                errors.push(format!(
                    "lagrange_ndelement.json {cell_name} {variant}: degree keys differ"
                ));
                continue;
            }
            for (deg, want_tab) in want_degs {
                let (Some(ht), Some(wt)) = (float_slice(&have_degs[deg]), float_slice(want_tab))
                else {
                    errors.push(format!(
                        "lagrange_ndelement.json {cell_name} {variant} p={deg}: invalid float array"
                    ));
                    continue;
                };
                if ht.len() != wt.len() || !wt.len().is_multiple_of(1 + td) {
                    errors.push(format!(
                        "lagrange_ndelement.json {cell_name} {variant} p={deg}: length {} vs {}",
                        ht.len(),
                        wt.len()
                    ));
                    continue;
                }
                let block_len = wt.len() / (1 + td);
                for d in 0..=td {
                    let base = d * block_len;
                    let hb = &ht[base..base + block_len];
                    let wb = &wt[base..base + block_len];
                    let scale = wb.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                    let mut worst = (0.0_f64, 0usize);
                    for (i, (a, b)) in hb.iter().zip(wb).enumerate() {
                        let err = (a - b).abs() / scale;
                        if err > worst.0 {
                            worst = (err, i);
                        }
                    }
                    if worst.0 > TAB_TOL {
                        errors.push(format!(
                            "lagrange_ndelement.json {cell_name} {variant} p={deg} deriv {d}[{}]: rel diff {:e} > {:e}",
                            worst.1, worst.0, TAB_TOL
                        ));
                    }
                }
            }
        }
    }
}

fn check_tabulations(dir: &Path, file: &str, want: &Value, errors: &mut Vec<String>) {
    let path = dir.join(file);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            errors.push(format!("{}: {e}", path.display()));
            return;
        }
    };
    let have: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("{}: parse error: {e}", path.display()));
            return;
        }
    };
    compare_tabulations_json(&have, want, errors);
}

fn run(check: bool) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let refcell_dir = manifest_dir.join("../tetraloma-refcell/tests/fixtures");
    let element_dir = manifest_dir.join("../tetraloma-element/tests/fixtures");

    let tables_val = tables();
    let orientation_val = orientation();
    let lagrange_points_val = lagrange_points();
    let sources_val = sources(&manifest_dir);
    let (lagrange_nd_val, base_trans_val) = ndelement_elements();

    if !check {
        fs::create_dir_all(&refcell_dir).expect("create refcell fixture directory");
        fs::create_dir_all(&element_dir).expect("create element fixture directory");
        write(&refcell_dir, "tables.json", &tables_val, true);
        // Compact: the hexahedron alone has 8! = 40 320 words.
        write(&refcell_dir, "orientation.json", &orientation_val, false);
        write(
            &refcell_dir,
            "lagrange_points.json",
            &lagrange_points_val,
            false,
        );
        write(&refcell_dir, "sources.json", &sources_val, true);

        write(
            &element_dir,
            "lagrange_ndelement.json",
            &lagrange_nd_val,
            false,
        );
        write(
            &element_dir,
            "base_transformations_ndelement.json",
            &base_trans_val,
            false,
        );
        write(&element_dir, "sources.json", &sources_val, true);
        return;
    }

    let mut errors = Vec::new();
    check_exact(&refcell_dir, "tables.json", &tables_val, true, &mut errors);
    check_exact(
        &refcell_dir,
        "orientation.json",
        &orientation_val,
        false,
        &mut errors,
    );
    check_exact(
        &refcell_dir,
        "lagrange_points.json",
        &lagrange_points_val,
        false,
        &mut errors,
    );
    check_exact(
        &refcell_dir,
        "sources.json",
        &sources_val,
        true,
        &mut errors,
    );
    check_tabulations(
        &element_dir,
        "lagrange_ndelement.json",
        &lagrange_nd_val,
        &mut errors,
    );
    check_exact(
        &element_dir,
        "base_transformations_ndelement.json",
        &base_trans_val,
        false,
        &mut errors,
    );
    check_exact(
        &element_dir,
        "sources.json",
        &sources_val,
        true,
        &mut errors,
    );

    if !errors.is_empty() {
        eprintln!("ndelement fixtures differ from a fresh export:");
        for err in &errors {
            eprintln!("  {err}");
        }
        process::exit(1);
    }
    println!(
        "all 7 ndelement fixtures match a fresh export (6 byte-identical, lagrange_ndelement <= 1e-13)"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let check = match args.as_slice() {
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
        let expected: Vec<u32> = original.iter().map(|&i| i as u32).collect();
        assert_eq!(expanded, expected);

        let data = vec![10, 20, 30, 40, 50, 60, 70];
        let mut applied = data.clone();
        apply_permutation(&swaps, &mut applied);
        let via_expanded: Vec<i32> = expanded.iter().map(|&p| data[p as usize]).collect();
        assert_eq!(applied, via_expanded);
    }

    #[test]
    fn compare_tabulations_respects_tolerance_and_structure() {
        let want = json!({
            "Interval": {
                "points": [0.5],
                "Equispaced": {
                    "1": [0.5, 0.5, -1.0, 1.0]
                }
            }
        });
        let within = json!({
            "Interval": {
                "points": [0.5 + 5e-16],
                "Equispaced": {
                    "1": [0.5 + 5e-14, 0.5, -1.0 + 5e-14, 1.0]
                }
            }
        });
        let mut errs = Vec::new();
        compare_tabulations_json(&within, &want, &mut errs);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");

        let outside = json!({
            "Interval": {
                "points": [0.5],
                "Equispaced": {
                    "1": [0.5 + 2e-13, 0.5, -1.0, 1.0]
                }
            }
        });
        compare_tabulations_json(&outside, &want, &mut errs);
        assert_eq!(errs.len(), 1, "expected 1 error, got {errs:?}");

        let missing_key = json!({
            "Interval": {
                "points": [0.5]
            }
        });
        errs.clear();
        compare_tabulations_json(&missing_key, &want, &mut errs);
        assert!(!errs.is_empty());
    }
}

//! M13 acceptance A9 and protected-path check (docs/design.md, section 23 row 9):
//! every stored quadrature table under `tetraloma-refcell/tables/` has a source and
//! MIT/Apache-2.0-compatible licence entry in `tables/sources.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde_json::{Map, Value};

const COMPATIBLE_LICENCES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "MIT OR Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "0BSD",
    "ISC",
    "CC0-1.0",
];

fn collect_table_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_table_files(root, &path, out);
            continue;
        }
        let name = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if name != "sources.json" && !(name.starts_with("LICENSE-") && name.ends_with(".md")) {
            out.insert(name);
        }
    }
}

fn require_str<'a>(obj: &'a Map<String, Value>, file: &str, key: &str) -> &'a str {
    let val = obj
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{file}: missing string field `{key}`"));
    assert!(!val.trim().is_empty(), "{file}: empty `{key}`");
    val
}

fn is_single_filename(name: &str) -> bool {
    let mut comps = Path::new(name).components();
    matches!(comps.next(), Some(Component::Normal(_))) && comps.next().is_none()
}

/// M13 acceptance A9: every data table in `tetraloma-refcell/tables/` has a valid provenance
/// entry in `tables/sources.json`, and `sources.json` names no missing file.
#[test]
#[ignore = "pending M13.T1"]
fn every_table_has_source_and_licence() {
    let tables_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tables");
    assert!(
        tables_dir.is_dir(),
        "tables directory missing: {}",
        tables_dir.display()
    );

    let sources_path = tables_dir.join("sources.json");
    let raw = fs::read_to_string(&sources_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", sources_path.display()));
    let manifest: Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("parse {}: {e}", sources_path.display()));
    let entries = manifest
        .as_object()
        .expect("sources.json must be a JSON object");

    let mut data_files = BTreeSet::new();
    collect_table_files(&tables_dir, &tables_dir, &mut data_files);
    assert!(
        !data_files.is_empty(),
        "no quadrature data tables found in {}",
        tables_dir.display()
    );

    let manifest_keys: BTreeSet<String> = entries.keys().cloned().collect();
    assert_eq!(
        data_files, manifest_keys,
        "tables/ data files do not match sources.json keys"
    );

    for (file, meta) in entries {
        let obj = meta
            .as_object()
            .unwrap_or_else(|| panic!("{file}: metadata entry must be a JSON object"));
        let _source = require_str(obj, file, "source");
        let _commit = require_str(obj, file, "commit");

        let licence = require_str(obj, file, "licence");
        assert!(
            COMPATIBLE_LICENCES.contains(&licence),
            "{file}: licence {licence:?} is not MIT/Apache-2.0-compatible"
        );

        let license_file = require_str(obj, file, "license_file");
        assert!(
            is_single_filename(license_file),
            "{file}: invalid `license_file` {license_file:?}"
        );
        let notice = fs::read_to_string(tables_dir.join(license_file))
            .unwrap_or_else(|e| panic!("{file}: read {license_file}: {e}"));
        assert!(
            !notice.trim().is_empty(),
            "{file}: empty licence notice {license_file}"
        );

        let sha256 = require_str(obj, file, "sha256");
        assert!(
            sha256.len() == 64
                && sha256
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "{file}: `sha256` {sha256:?} is not a 64-char lowercase hex digest"
        );
    }
}

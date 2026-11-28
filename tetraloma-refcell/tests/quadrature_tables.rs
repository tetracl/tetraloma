//! M13 acceptance A9 and protected-path check (docs/design.md, section 23 row 9):
//! every stored quadrature table under `tetraloma-refcell/tables/` has a source,
//! MIT/Apache-2.0 licence entry, notice file and matching SHA-256 digest in `tables/sources.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde_json::{Map, Value};

const COMPATIBLE_LICENCES: &[&str] = &["MIT", "Apache-2.0", "MIT OR Apache-2.0"];

fn collect_table_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let entry = entry.expect("dir entry");
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
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

/// Dependency-free FIPS 180-4 SHA-256 hex digest (lowercase) so the section 23 protected-path
/// check detects table content edits without adding a crate dependency.
fn sha256_hex(data: &[u8]) -> String {
    #[rustfmt::skip]
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = Vec::with_capacity((data.len() + 9).div_ceil(64) * 64);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());

    let mut w = [0u32; 64];
    for block in msg.as_chunks::<64>().0 {
        for (i, chunk) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*chunk);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (hi, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *hi = hi.wrapping_add(v);
        }
    }
    let mut out = String::with_capacity(64);
    for word in h {
        use std::fmt::Write;
        let _ = write!(&mut out, "{word:08x}");
    }
    out
}

#[test]
fn sha256_matches_nist_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

/// M13 acceptance A9: every data table in `tetraloma-refcell/tables/` has a valid provenance
/// entry and matching SHA-256 digest in `tables/sources.json`, and `sources.json` names no
/// missing file.
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
        let bytes = fs::read(tables_dir.join(file))
            .unwrap_or_else(|e| panic!("{file}: read table bytes: {e}"));
        let actual = sha256_hex(&bytes);
        assert_eq!(
            actual, sha256,
            "{file}: SHA-256 digest mismatch against tables/sources.json"
        );
    }
}

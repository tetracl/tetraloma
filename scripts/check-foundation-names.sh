#!/usr/bin/env bash
# Fails if any public struct, enum, union, trait, type alias, function, method, constant or static
# of the foundation crates has `Grid`, `Mesh`, `Cell`, `Conn` or `Dof` in its name, checked on the
# generated rustdoc API pages (docs/design.md, M18 A3; ADR-0074).
#
# Reading of names (docs/plans/M18.md, Acceptance A3):
# - Camel case (contains both uppercase and lowercase letters): fails if it contains `Grid`,
#   `Mesh`, `Cell`, `Conn` or `Dof` as a substring.
# - Snake or upper case (no mixed case): fails if any `_`-separated segment, case-insensitively,
#   is one of those five words or its plural (`grid`/`grids`, `mesh`/`meshes`, `cell`/`cells`,
#   `conn`/`conns`, `dof`/`dofs`).
# - Enum variants and `#[doc(hidden)]` items are not checked.
#
# Usage: scripts/check-foundation-names.sh [--crates C,...] [--api DIR]
#   --crates C,...  comma-separated crate names (default: the foundation crates that exist:
#                   tetraloma-device,tetraloma-parallel,tetraloma-partition; M18.T2 adds
#                   tetraloma-comm). E.g. `--crates tetraloma-topology` must fail.
#   --api DIR       check already-built rustdoc HTML pages under DIR without running `cargo doc`.
#
# Exit status: 0 if none found, 3 if forbidden names are found, 2 on usage/markup errors,
# anything else on a build error.
set -euo pipefail

cd "$(dirname "$0")/.."

DEFAULT_CRATES="tetraloma-device,tetraloma-parallel,tetraloma-partition"
PYTHON=${TETRALOMA_PYTHON:-python3}
die() { echo "check-foundation-names: $*" >&2; exit 2; }

crates=$DEFAULT_CRATES
api=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --crates) [[ $# -ge 2 && -n "$2" ]] || die "--crates requires a list"; crates=$2; shift 2 ;;
        --crates=*) crates=${1#--crates=}; [[ -n "$crates" ]] || die "--crates requires a list"; shift ;;
        --api) [[ $# -ge 2 && -n "$2" ]] || die "--api requires a directory"; api=$2; shift 2 ;;
        --api=*) api=${1#--api=}; [[ -n "$api" ]] || die "--api requires a directory"; shift ;;
        -h | --help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown argument '$1' (see --help)" ;;
    esac
done

[[ "$crates" =~ ^[A-Za-z0-9_-]+(,[A-Za-z0-9_-]+)*$ ]] || die "--crates '$crates' is not a valid crate list"
IFS=',' read -r -a crate_list <<<"$crates"

if [[ -z "$api" ]]; then
    api="${CARGO_TARGET_DIR:-target}/doc"
    doc_args=()
    for c in "${crate_list[@]}"; do
        rm -rf "$api/${c//-/_}"
        doc_args+=(-p "$c")
    done
    cargo doc --no-deps "${doc_args[@]}" >/dev/null
fi
[[ -d "$api" ]] || die "API directory '$api' does not exist"

"$PYTHON" - "$api" "${crate_list[@]}" <<'PY'
import re, sys
from html.parser import HTMLParser
from pathlib import Path

FORBIDDEN_CAMEL = ("Grid", "Mesh", "Cell", "Conn", "Dof")
FORBIDDEN_SEGMENTS = {"grid", "grids", "mesh", "meshes", "cell", "cells", "conn", "conns", "dof", "dofs"}
ITEM_PAGE = re.compile(r"^(struct|enum|union|trait|type|constant|static|fn)\.(.+)\.html$")
MEMBER_ID = re.compile(r"^(?:ty)?(method|associatedconstant|associatedtype)\.([A-Za-z_][A-Za-z0-9_]*)$")
REEXPORT_USE = re.compile(r"\buse\s+([^;]+);")
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}
KIND_LABEL = {"method": "method", "associatedconstant": "constant", "associatedtype": "type"}


def is_forbidden(name: str) -> bool:
    if any(c.isupper() for c in name) and any(c.islower() for c in name):
        return any(w in name for w in FORBIDDEN_CAMEL)
    return any(seg.lower() in FORBIDDEN_SEGMENTS for seg in name.split("_") if seg)


def reexport_names(text: str) -> list[str]:
    m = REEXPORT_USE.search(text)
    if not m:
        return []
    expr = m.group(1).strip()
    parts = expr[expr.index("{") + 1:expr.rindex("}")].split(",") if "{" in expr and "}" in expr else [expr]
    names = []
    for p in (p.strip() for p in parts if p.strip()):
        name = p.split(" as ", 1)[1].strip() if " as " in p else p.split("::")[-1].strip()
        if name and name not in ("_", "*"):
            names.append(name)
    return names


class PageScanner(HTMLParser):
    def __init__(self, is_index: bool, is_trait: bool):
        super().__init__(convert_charrefs=True)
        self.is_index, self.is_trait = is_index, is_trait
        self.stack: list[str] = []
        self.region_depth = self.reexport_dl_depth = None
        self.after_reexports = self.in_dt = False
        self.dt_buf: list[str] = []
        self.members: list[tuple[str, str]] = []
        self.reexports: list[str] = []

    def handle_starttag(self, tag, attrs):
        d = {k: (v or "") for k, v in attrs}
        cls, el_id, depth = set(d.get("class", "").split()), d.get("id", ""), len(self.stack)
        if self.is_index:
            if tag == "h2":
                self.after_reexports = el_id == "reexports"
            elif self.after_reexports and tag == "dl" and "item-table" in cls:
                self.reexport_dl_depth = depth
            elif self.reexport_dl_depth is not None and tag == "dt":
                self.in_dt, self.dt_buf = True, []
        elif self.region_depth is None:
            if (self.is_trait and tag == "div" and "methods" in cls) or (not self.is_trait and el_id == "implementations-list"):
                self.region_depth = depth
        elif tag == "section" and "method" in cls and "trait-impl" not in cls:
            mm = MEMBER_ID.match(el_id)
            if mm:
                self.members.append((KIND_LABEL[mm.group(1)], mm.group(2)))
        if tag not in VOID:
            self.stack.append(tag)

    def handle_endtag(self, tag):
        if self.in_dt and tag == "dt":
            self.reexports.extend(reexport_names("".join(self.dt_buf)))
            self.in_dt = False
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i] == tag:
                del self.stack[i:]
                break
        if self.region_depth is not None and len(self.stack) <= self.region_depth:
            self.region_depth = None
        if self.reexport_dl_depth is not None and len(self.stack) <= self.reexport_dl_depth:
            self.reexport_dl_depth = None

    def handle_data(self, data):
        if self.in_dt:
            self.dt_buf.append(data)


def scan_crate(api_dir: Path, crate: str) -> tuple[int, list[tuple[str, str, str]]]:
    crate_dir = api_dir / crate.replace("-", "_")
    if not crate_dir.is_dir():
        sys.exit(f"check-foundation-names: no rustdoc directory {crate_dir} for crate '{crate}'")
    seen: set[tuple[str, str]] = set()
    hits: list[tuple[str, str, str]] = []

    def record(path: str, kind: str, name: str):
        if (path, kind) in seen:
            return
        seen.add((path, kind))
        if is_forbidden(name):
            hits.append((path, kind, name))

    for page in sorted(crate_dir.rglob("*.html")):
        m = ITEM_PAGE.match(page.name)
        if not m and page.name != "index.html":
            continue
        text = page.read_text(errors="replace")
        if 'http-equiv="refresh"' in text[:2000]:
            continue
        dirs = page.relative_to(api_dir).as_posix().split("/")[:-1]
        scanner = PageScanner(page.name == "index.html", bool(m and m.group(1) == "trait"))
        scanner.feed(text)
        scanner.close()
        if page.name == "index.html":
            for name in scanner.reexports:
                record("::".join(dirs + [name]), "re-export", name)
        else:
            kind, name = m.group(1), m.group(2)
            item_path = "::".join(dirs + [name])
            record(item_path, kind, name)
            for mkind, mname in scanner.members:
                record(f"{item_path}::{mname}", mkind, mname)
    return len(seen), hits


api_dir, crates = Path(sys.argv[1]), sys.argv[2:]
total, all_hits = 0, []
for crate in crates:
    try:
        count, hits = scan_crate(api_dir, crate)
    except SystemExit as e:
        print(e, file=sys.stderr)
        sys.exit(2)
    if count == 0:
        print(f"check-foundation-names: no items found for '{crate}' under {api_dir} (rustdoc markup changed?)", file=sys.stderr)
        sys.exit(2)
    total += count
    all_hits.extend(hits)

if all_hits:
    print("check-foundation-names: forbidden public names found:", file=sys.stderr)
    for path, kind, name in all_hits:
        print(f"  {path} ({kind}: {name})", file=sys.stderr)
    sys.exit(3)

print(f"check-foundation-names: ok ({total} items across {', '.join(crates)}, none forbidden)")
PY

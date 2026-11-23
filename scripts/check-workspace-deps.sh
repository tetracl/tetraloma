#!/usr/bin/env bash
# Fails if:
# - the default build of the workspace depends on ndelement, BLAS/LAPACK or MPI
#   (docs/design.md, "Dependency rules" 5 and 6; M0 acceptance);
# - any crate in the --no-cubecl list depends on any cubecl crate across normal, build and dev
#   dependencies (rules 8 and 9; M11 A13, M18 A2);
# - any crate in the --no-mesh list depends on a mesh crate of rule 9 across normal and build
#   dependencies (M18 A1);
# - any crate in the --foundation-only list depends in its normal dependencies on a workspace
#   crate outside {tetraloma-comm, tetraloma-device, tetraloma-parallel, tetraloma-comm-testing,
#   tetraloma-testing-base} (M18 A6).
#
# Usage: scripts/check-workspace-deps.sh [--no-cubecl CRATE]... [--no-mesh CRATE]...
#                                        [--foundation-only CRATE]... [CARGO-TREE-ARGS...]
#   --no-cubecl CRATE        add CRATE to the cubecl-free list (repeatable; default:
#                            tetraloma-element); e.g. `--no-cubecl tetraloma-geometry` must fail.
#   --no-mesh CRATE          add CRATE to the mesh-free list (repeatable; default: none);
#                            e.g. `--no-mesh tetraloma-topology` must fail.
#   --foundation-only CRATE  add CRATE to the foundation-only list (repeatable; default: none).
#   Extra arguments go to the workspace `cargo tree`; e.g. `--features tetraloma-mpi-tests/mpi`
#   must fail.
#
# Exit status: 0 if no forbidden crate is found, 3 if one is, anything else on an error.
set -euo pipefail

cd "$(dirname "$0")/.."

# Default lists; enabling tasks add their crates here.
no_cubecl_crates=("tetraloma-element")
no_mesh_crates=()
foundation_only_crates=()
tree_args=()

has_item() {
    local needle=$1 item
    shift
    for item in "$@"; do [[ "$item" == "$needle" ]] && return 0; done
    return 1
}

fmt_list() {
    [[ $# -gt 0 ]] || { printf 'none'; return; }
    local out=$1 item; shift
    for item in "$@"; do out+=", $item"; done
    printf '%s' "$out"
}

add_opt() {
    local opt=$1 val=$2
    [[ -n "$val" ]] || { echo "check-workspace-deps: $opt requires a crate name" >&2; exit 2; }
    case "$opt" in
        --no-cubecl) has_item "$val" ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"} || no_cubecl_crates+=("$val") ;;
        --no-mesh) has_item "$val" ${no_mesh_crates[@]+"${no_mesh_crates[@]}"} || no_mesh_crates+=("$val") ;;
        --foundation-only) has_item "$val" ${foundation_only_crates[@]+"${foundation_only_crates[@]}"} || foundation_only_crates+=("$val") ;;
    esac
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-cubecl | --no-mesh | --foundation-only)
            [[ $# -ge 2 ]] || add_opt "$1" ""
            add_opt "$1" "$2"
            shift 2
            ;;
        --no-cubecl=* | --no-mesh=* | --foundation-only=*)
            add_opt "${1%%=*}" "${1#*=}"
            shift
            ;;
        -h | --help)
            sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            tree_args+=("$1")
            shift
            ;;
    esac
done

failed=0
report_hits() {
    local msg=$1 hits=$2
    [[ -n "$hits" ]] || return 0
    echo "check-workspace-deps: $msg:" >&2
    while IFS= read -r dep; do echo "  $dep" >&2; done <<<"$hits"
    failed=1
}

forbidden='^(ndelement|.*(blas|lapack|mkl|netlib).*|accelerate-src|mpi(-.*)?)$'
crates=$(cargo tree --workspace --target all -e normal,build,dev --prefix none --format '{p}' ${tree_args[@]+"${tree_args[@]}"} |
    awk 'NF { print $1 }' | sort -u)
report_hits "the workspace build depends on" "$(grep -E "$forbidden" <<<"$crates" || true)"

for crate in ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"}; do
    deps=$(cargo tree -p "$crate" --all-features --target all -e normal,build,dev --prefix none --format '{p}' |
        awk 'NF { print $1 }' | sort -u)
    report_hits "$crate depends on cubecl" "$(grep -E '^cubecl(-.*)?$' <<<"$deps" || true)"
done

mesh_re='^(tetraloma|tetraloma-(dims|refcell|topology|geometry|io|element|function))$'
for crate in ${no_mesh_crates[@]+"${no_mesh_crates[@]}"}; do
    deps=$(cargo tree -p "$crate" --all-features --target all -e normal,build --prefix none --format '{p}' |
        awk -v self="$crate" 'NF && $1 != self { print $1 }' | sort -u)
    report_hits "$crate depends on mesh crates (rule 9)" "$(grep -E "$mesh_re" <<<"$deps" || true)"
done

foundation_allowed_re='^tetraloma-(comm|device|parallel|comm-testing|testing-base)$'
if [[ ${#foundation_only_crates[@]} -gt 0 ]]; then
    ws_crates=$(cargo tree --workspace --depth 0 --prefix none --format '{p}' | awk 'NF { print $1 }' | sort -u)
    for crate in "${foundation_only_crates[@]}"; do
        deps=$(cargo tree -p "$crate" --all-features --target all -e normal --prefix none --format '{p}' |
            awk -v self="$crate" 'NF && $1 != self { print $1 }' | sort -u)
        ws_deps=$(awk 'NR == FNR { ws[$1] = 1; next } ($1 in ws) { print $1 }' \
            <(printf '%s\n' "$ws_crates") <(printf '%s\n' "$deps"))
        report_hits "$crate has disallowed workspace normal dependencies" \
            "$(grep -Ev "$foundation_allowed_re" <<<"$ws_deps" | grep . || true)"
    done
fi

[[ "$failed" -eq 0 ]] || exit 3

echo "check-workspace-deps: ok ($(wc -l <<<"$crates" | tr -d ' ') crates, none forbidden; free of cubecl: $(fmt_list ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"}); free of mesh: $(fmt_list ${no_mesh_crates[@]+"${no_mesh_crates[@]}"}); foundation-only: $(fmt_list ${foundation_only_crates[@]+"${foundation_only_crates[@]}"}))"

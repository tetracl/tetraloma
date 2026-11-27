#!/usr/bin/env bash
# Fails if:
# - the default build of the workspace depends on ndelement, BLAS/LAPACK or MPI
#   (docs/design.md, "Dependency rules" 5 and 6; M0 acceptance);
# - any crate in the --no-cubecl list depends on any cubecl crate across normal, build and dev
#   dependencies (rules 8 and 9; M11 A13, M18 A2);
# - any crate in the --no-mesh list depends on a mesh crate of rule 9 across normal, build and dev
#   dependencies (M18 A1, T7);
# - any crate in the --element-only list depends anywhere in its normal or build dependency tree
#   on any crate outside {tetraloma-dims, tetraloma-refcell} (rule 8; M11 G11);
# - any crate in the --foundation-only list depends in its normal dependencies on a workspace
#   crate outside {tetraloma-comm, tetraloma-device, tetraloma-parallel, tetraloma-comm-testing,
#   tetraloma-testing-base} (M18 A6).
#
# Usage: scripts/check-workspace-deps.sh [--no-cubecl CRATE]... [--no-mesh CRATE]...
#                                        [--element-only CRATE]... [--foundation-only CRATE]...
#                                        [CARGO-TREE-ARGS...]
#        scripts/check-workspace-deps.sh --self-test
#   --no-cubecl CRATE        add CRATE to the cubecl-free list (repeatable; default:
#                            tetraloma-element, tetraloma-comm, tetraloma-partition); e.g.
#                            `--no-cubecl tetraloma-geometry` must fail.
#   --no-mesh CRATE          add CRATE to the mesh-free list (repeatable; default:
#                            tetraloma-comm, tetraloma-device, tetraloma-parallel,
#                            tetraloma-partition); e.g. `--no-mesh tetraloma-topology` must fail.
#   --element-only CRATE     add CRATE to the rule-8 allowlist check (repeatable; default:
#                            tetraloma-element); e.g. `--element-only tetraloma-topology` and
#                            `--element-only tetraloma-comm` must fail.
#   --foundation-only CRATE  add CRATE to the foundation-only list (repeatable; default:
#                            tetraloma-foundation-tests); e.g. `--foundation-only tetraloma-testing`
#                            must fail.
#   --self-test              run built-in self-tests (direct and transitive dev-dependency-only
#                            `--no-mesh` negative controls on ephemeral manifests) and exit.
#   Extra arguments go to the workspace `cargo tree`; e.g. `--features tetraloma-mpi-tests/mpi`
#   must fail.
#
# Exit status: 0 if no forbidden crate is found (or self-tests pass), 3 if one is, anything else
# on an error.
set -euo pipefail

script_path="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$script_path")/.."

# Default lists; enabling tasks add their crates here.
no_cubecl_crates=("tetraloma-element" "tetraloma-comm" "tetraloma-partition")
no_mesh_crates=("tetraloma-comm" "tetraloma-device" "tetraloma-parallel" "tetraloma-partition")
element_only_crates=("tetraloma-element")
foundation_only_crates=("tetraloma-foundation-tests")
tree_args=()
self_test=0
explicit_opt=0
self_test_tmp=""
self_test_active_err=""

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
    explicit_opt=1
    case "$opt" in
        --no-cubecl) has_item "$val" ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"} || no_cubecl_crates+=("$val") ;;
        --no-mesh) has_item "$val" ${no_mesh_crates[@]+"${no_mesh_crates[@]}"} || no_mesh_crates+=("$val") ;;
        --element-only) has_item "$val" ${element_only_crates[@]+"${element_only_crates[@]}"} || element_only_crates+=("$val") ;;
        --foundation-only) has_item "$val" ${foundation_only_crates[@]+"${foundation_only_crates[@]}"} || foundation_only_crates+=("$val") ;;
    esac
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-cubecl | --no-mesh | --element-only | --foundation-only)
            [[ $# -ge 2 ]] || add_opt "$1" ""
            add_opt "$1" "$2"
            shift 2
            ;;
        --no-cubecl=* | --no-mesh=* | --element-only=* | --foundation-only=*)
            add_opt "${1%%=*}" "${1#*=}"
            shift
            ;;
        --self-test)
            self_test=1
            shift
            ;;
        -h | --help)
            awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$script_path"
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

mesh_re='^(tetraloma|tetraloma-(dims|refcell|topology|geometry|io|element|function))$'
check_no_mesh_crate() {
    local crate=$1
    shift
    local deps
    deps=$(cargo tree ${1+"$@"} -p "$crate" --all-features --target all -e normal,build,dev --prefix none --format '{p}' |
        awk -v self="$crate" 'NF && $1 != self { print $1 }' | sort -u)
    report_hits "$crate depends on mesh crates (rule 9)" "$(grep -E "$mesh_re" <<<"$deps" || true)"
}

cleanup_self_test() {
    local rc=$?
    if [[ "$rc" -ne 0 && -n "$self_test_active_err" && -s "$self_test_active_err" ]]; then
        cat "$self_test_active_err" >&3
    fi
    [[ -z "$self_test_tmp" ]] || rm -rf "$self_test_tmp"
}

run_self_test() {
    exec 3>&2
    trap cleanup_self_test EXIT
    self_test_tmp=$(mktemp -d "${TMPDIR:-/tmp}/check-workspace-deps-self-test.XXXXXX")

    mkdir -p \
        "$self_test_tmp/tetraloma-dims/src" \
        "$self_test_tmp/dev-helper/src" \
        "$self_test_tmp/dev-direct-probe/src" \
        "$self_test_tmp/dev-transitive-probe/src" \
        "$self_test_tmp/clean-probe/src"
    cat >"$self_test_tmp/Cargo.toml" <<'TOML'
[workspace]
resolver = "3"
members = ["tetraloma-dims", "dev-helper", "dev-direct-probe", "dev-transitive-probe", "clean-probe"]
TOML
    cat >"$self_test_tmp/tetraloma-dims/Cargo.toml" <<'TOML'
[package]
name = "tetraloma-dims"
version = "0.1.0"
edition = "2024"
TOML
    cat >"$self_test_tmp/dev-helper/Cargo.toml" <<'TOML'
[package]
name = "dev-helper"
version = "0.1.0"
edition = "2024"

[dependencies]
tetraloma-dims = { path = "../tetraloma-dims" }
TOML
    cat >"$self_test_tmp/dev-direct-probe/Cargo.toml" <<'TOML'
[package]
name = "dev-direct-probe"
version = "0.1.0"
edition = "2024"

[dev-dependencies]
tetraloma-dims = { path = "../tetraloma-dims" }
TOML
    cat >"$self_test_tmp/dev-transitive-probe/Cargo.toml" <<'TOML'
[package]
name = "dev-transitive-probe"
version = "0.1.0"
edition = "2024"

[dev-dependencies]
dev-helper = { path = "../dev-helper" }
TOML
    cat >"$self_test_tmp/clean-probe/Cargo.toml" <<'TOML'
[package]
name = "clean-probe"
version = "0.1.0"
edition = "2024"
TOML
    local pkg
    for pkg in tetraloma-dims dev-helper dev-direct-probe dev-transitive-probe clean-probe; do
        : >"$self_test_tmp/$pkg/src/lib.rs"
    done

    local manifest="$self_test_tmp/Cargo.toml" probe normal_deps err_file
    for probe in dev-direct-probe dev-transitive-probe; do
        normal_deps=$(cargo tree --manifest-path "$manifest" --offline -p "$probe" \
            --all-features --target all -e normal,build --prefix none --format '{p}' |
            awk -v self="$probe" 'NF && $1 != self { print $1 }' | sort -u)
        if [[ -n "$normal_deps" ]]; then
            echo "check-workspace-deps: self-test setup error: $probe has normal/build deps: $normal_deps" >&2
            exit 1
        fi
        failed=0
        err_file="$self_test_tmp/$probe.err"
        self_test_active_err="$err_file"
        check_no_mesh_crate "$probe" --manifest-path "$manifest" --offline 2>"$err_file"
        self_test_active_err=""
        if [[ "$failed" -ne 1 ]] || ! grep -qx '  tetraloma-dims' "$err_file"; then
            echo "check-workspace-deps: self-test failed: $probe dev-dependency on tetraloma-dims was not caught" >&2
            cat "$err_file" >&2
            exit 1
        fi
    done

    failed=0
    err_file="$self_test_tmp/clean-probe.err"
    self_test_active_err="$err_file"
    check_no_mesh_crate clean-probe --manifest-path "$manifest" --offline 2>"$err_file"
    self_test_active_err=""
    if [[ "$failed" -ne 0 ]] || grep -q '^check-workspace-deps:' "$err_file"; then
        echo "check-workspace-deps: self-test failed: clean-probe was rejected" >&2
        cat "$err_file" >&2
        exit 1
    fi

    echo "check-workspace-deps: self-test ok"
}

if [[ "$self_test" -eq 1 ]]; then
    if [[ "$explicit_opt" -ne 0 || ${#tree_args[@]} -gt 0 ]]; then
        echo "check-workspace-deps: --self-test takes no other arguments" >&2
        exit 2
    fi
    run_self_test
    exit 0
fi

forbidden='^(ndelement|.*(blas|lapack|mkl|netlib).*|accelerate-src|mpi(-.*)?)$'
crates=$(cargo tree --workspace --target all -e normal,build,dev --prefix none --format '{p}' ${tree_args[@]+"${tree_args[@]}"} |
    awk 'NF { print $1 }' | sort -u)
report_hits "the workspace build depends on" "$(grep -E "$forbidden" <<<"$crates" || true)"

for crate in ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"}; do
    deps=$(cargo tree -p "$crate" --all-features --target all -e normal,build,dev --prefix none --format '{p}' |
        awk 'NF { print $1 }' | sort -u)
    report_hits "$crate depends on cubecl" "$(grep -E '^cubecl(-.*)?$' <<<"$deps" || true)"
done

for crate in ${no_mesh_crates[@]+"${no_mesh_crates[@]}"}; do
    check_no_mesh_crate "$crate"
done

element_allowed_re='^tetraloma-(dims|refcell)$'
for crate in ${element_only_crates[@]+"${element_only_crates[@]}"}; do
    deps=$(cargo tree -p "$crate" --all-features --target all -e normal,build --prefix none --format '{p}' |
        awk -v self="$crate" 'NF && $1 != self { print $1 }' | sort -u)
    report_hits "$crate has disallowed normal/build dependencies (rule 8)" \
        "$(grep -Ev "$element_allowed_re" <<<"$deps" | grep . || true)"
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

echo "check-workspace-deps: ok ($(wc -l <<<"$crates" | tr -d ' ') crates, none forbidden; free of cubecl: $(fmt_list ${no_cubecl_crates[@]+"${no_cubecl_crates[@]}"}); free of mesh: $(fmt_list ${no_mesh_crates[@]+"${no_mesh_crates[@]}"}); element-only: $(fmt_list ${element_only_crates[@]+"${element_only_crates[@]}"}); foundation-only: $(fmt_list ${foundation_only_crates[@]+"${foundation_only_crates[@]}"}))"

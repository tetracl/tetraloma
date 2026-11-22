#!/usr/bin/env bash
# Fails if the default build of the workspace depends on ndelement, BLAS/LAPACK or MPI
# (docs/design.md, "Dependency rules" 5 and 6; M0 acceptance), or if tetraloma-element
# (or the crate named by --no-cubecl) depends on any cubecl crate (rule 8; M11 A13).
#
# Usage: scripts/check-workspace-deps.sh [--no-cubecl CRATE] [CARGO-TREE-ARGS...]
#   --no-cubecl CRATE  crate whose dependency tree must be free of cubecl (default: tetraloma-element);
#                      for example `--no-cubecl tetraloma-geometry` must fail.
#   Extra arguments go to the workspace `cargo tree`; for example `--features tetraloma-mpi-tests/mpi`
#   must fail.
#
# Exit status: 0 if no forbidden crate is found, 3 if one is, anything else on an error.
set -euo pipefail

cd "$(dirname "$0")/.."

no_cubecl="tetraloma-element"
tree_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-cubecl)
            if [[ $# -lt 2 || -z "$2" ]]; then
                echo "check-workspace-deps: --no-cubecl requires a crate name" >&2
                exit 2
            fi
            no_cubecl=$2
            shift 2
            ;;
        --no-cubecl=*)
            no_cubecl=${1#--no-cubecl=}
            if [[ -z "$no_cubecl" ]]; then
                echo "check-workspace-deps: --no-cubecl requires a crate name" >&2
                exit 2
            fi
            shift
            ;;
        *)
            tree_args+=("$1")
            shift
            ;;
    esac
done

forbidden='^(ndelement|.*(blas|lapack|mkl|netlib).*|accelerate-src|mpi(-.*)?)$'

crates=$(cargo tree --workspace --target all -e normal,build,dev --prefix none --format '{p}' ${tree_args[@]+"${tree_args[@]}"} |
    awk '{ print $1 }' | sort -u)
found=$(grep -E "$forbidden" <<<"$crates" || true)

if [[ -n "$found" ]]; then
    echo "check-workspace-deps: the workspace build depends on:" >&2
    while IFS= read -r crate; do
        echo "  $crate" >&2
    done <<<"$found"
    exit 3
fi

cubecl_crates=$(cargo tree -p "$no_cubecl" --target all -e normal,build,dev --prefix none --format '{p}' |
    awk '{ print $1 }' | sort -u)
cubecl_found=$(grep -E '^cubecl(-.*)?$' <<<"$cubecl_crates" || true)

if [[ -n "$cubecl_found" ]]; then
    echo "check-workspace-deps: $no_cubecl depends on cubecl:" >&2
    while IFS= read -r crate; do
        echo "  $crate" >&2
    done <<<"$cubecl_found"
    exit 3
fi

echo "check-workspace-deps: ok ($(wc -l <<<"$crates" | tr -d ' ') crates, none forbidden; $no_cubecl free of cubecl)"

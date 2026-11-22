#!/usr/bin/env bash
# Runs workspace CPU-runtime tests without CubeCL C14 thread-oversubscription collapse (ADR-0073):
#   - host-only crates (no `cubecl` in their dependency tree) run at default libtest parallelism;
#   - CubeCL-dependent crates run with `--test-threads=${TETRALOMA_CPU_TEST_THREADS:-1}`.
#
# Usage: scripts/test-cpu.sh [--dry-run] [-p CRATE]... [CARGO-TEST-ARGS...] [-- LIBTEST-ARGS...]
#   -p / --package CRATE  limit the run to the named workspace crate(s); default: whole workspace.
#   --dry-run             print the commands without running cargo test.
#
# Environment:
#   TETRALOMA_CPU_TEST_THREADS  libtest threads for CubeCL crates (default: 1).
set -euo pipefail

cd "$(dirname "$0")/.."

dry_run=0
packages=()
cargo_args=()
libtest_args=()
after_sep=0

while [[ $# -gt 0 ]]; do
    if [[ $after_sep -eq 1 ]]; then
        libtest_args+=("$1")
        shift
        continue
    fi
    case "$1" in
        --dry-run)
            dry_run=1
            shift
            ;;
        -p | --package)
            if [[ $# -lt 2 || -z "$2" ]]; then
                echo "test-cpu.sh: $1 requires a crate name" >&2
                exit 2
            fi
            packages+=("$2")
            shift 2
            ;;
        --package=*)
            pkg=${1#--package=}
            if [[ -z "$pkg" ]]; then
                echo "test-cpu.sh: --package requires a crate name" >&2
                exit 2
            fi
            packages+=("$pkg")
            shift
            ;;
        --)
            after_sep=1
            shift
            ;;
        -h | --help)
            sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            cargo_args+=("$1")
            shift
            ;;
    esac
done

threads=${TETRALOMA_CPU_TEST_THREADS:-1}
if ! [[ "$threads" =~ ^[1-9][0-9]*$ ]]; then
    echo "test-cpu.sh: TETRALOMA_CPU_TEST_THREADS must be a positive integer, got '$threads'" >&2
    exit 2
fi

# Classify selected workspace members into host-only vs CubeCL-dependent using cargo metadata.
classified=$(cargo metadata --format-version 1 | python3 -I -c '
import json, sys

meta = json.load(sys.stdin)
members = set(meta["workspace_members"])
pkgs = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n["dependencies"] for n in meta["resolve"]["nodes"]}
requested = sys.argv[1:]

has_cubecl_cache = {}
def has_cubecl(pid):
    if pid in has_cubecl_cache:
        return has_cubecl_cache[pid]
    # Guard against cycles (none in cargo, safe default).
    has_cubecl_cache[pid] = False
    name = pkgs[pid]["name"]
    if name == "cubecl" or name.startswith("cubecl-"):
        has_cubecl_cache[pid] = True
        return True
    for dep in nodes.get(pid, ()):
        if has_cubecl(dep):
            has_cubecl_cache[pid] = True
            return True
    return False

by_name = {pkgs[pid]["name"]: pid for pid in members}
if requested:
    selected = []
    for name in requested:
        if name not in by_name:
            sys.stderr.write(f"test-cpu.sh: unknown workspace crate: {name}\n")
            sys.exit(2)
        if name not in selected:
            selected.append(name)
else:
    selected = sorted(by_name.keys())

for name in selected:
    kind = "cubecl" if has_cubecl(by_name[name]) else "host"
    print(f"{kind} {name}")
' ${packages[@]+"${packages[@]}"})

host_crates=()
cubecl_crates=()
while IFS=' ' read -r kind name; do
    [[ -n "$kind" ]] || continue
    if [[ "$kind" == "host" ]]; then
        host_crates+=("$name")
    else
        cubecl_crates+=("$name")
    fi
done <<<"$classified"

run_cmd() {
    echo "+ $*" >&2
    if [[ $dry_run -eq 0 ]]; then
        "$@"
    fi
}

if [[ ${#host_crates[@]} -gt 0 ]]; then
    cmd=(cargo test)
    for c in "${host_crates[@]}"; do cmd+=(-p "$c"); done
    cmd+=(${cargo_args[@]+"${cargo_args[@]}"})
    if [[ ${#libtest_args[@]} -gt 0 ]]; then
        cmd+=(-- "${libtest_args[@]}")
    fi
    run_cmd "${cmd[@]}"
fi

if [[ ${#cubecl_crates[@]} -gt 0 ]]; then
    cmd=(cargo test)
    for c in "${cubecl_crates[@]}"; do cmd+=(-p "$c"); done
    cmd+=(${cargo_args[@]+"${cargo_args[@]}"})
    cmd+=(-- --test-threads="$threads" ${libtest_args[@]+"${libtest_args[@]}"})
    run_cmd "${cmd[@]}"
fi

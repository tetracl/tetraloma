#!/usr/bin/env bash
# Runs workspace CPU-runtime tests on the local Mac without CubeCL C14 thread-oversubscription
# collapse (ADR-0073):
#   - host-only crates (no `cubecl` or `cubecl-*` in their resolved dependency graph) run at
#     default libtest parallelism;
#   - CubeCL-dependent crates run with `--test-threads=${TETRALOMA_CPU_TEST_THREADS:-1}`.
# Intended for the local Mac (on Locust aarch64, `--test-threads=1` hits C17).
#
# Usage: scripts/test-cpu.sh [--dry-run] [--with-rdeps] [--print-cubecl] [-p CRATE]... [CARGO-TEST-ARGS...] [-- LIBTEST-ARGS...]
#   -p / --package CRATE  limit the run to the named workspace crate(s); default: whole workspace.
#   --with-rdeps          expand `-p` crates to transitive reverse workspace deps (default-feature graph).
#   --print-cubecl        print `-p CRATE` args for the selected CubeCL crates on stdout and exit.
#   --dry-run             print the commands without running cargo test.
#
# Notes:
#   - Rejects `--workspace`, `--all`, `--exclude`, attached `-pCRATE`, and `-- --test-threads`.
#   - Package-qualified `--features pkg/feat` or `--test NAME` require matching `-p CRATE`.
#   - Set `TETRALOMA_CPU_TEST_THREADS` (default: 1) to change CubeCL libtest thread count.
set -euo pipefail

script_path="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$script_path")/.."

dry_run=0
with_rdeps=0
print_cubecl=0
packages=()
cargo_args=()
libtest_args=()
after_sep=0

while [[ $# -gt 0 ]]; do
    if [[ $after_sep -eq 1 ]]; then
        case "$1" in
            --test-threads | --test-threads=*)
                echo "test-cpu.sh: set TETRALOMA_CPU_TEST_THREADS instead of passing '$1'" >&2
                exit 2
                ;;
        esac
        libtest_args+=("$1")
        shift
        continue
    fi
    case "$1" in
        --dry-run)
            dry_run=1
            shift
            ;;
        --with-rdeps)
            with_rdeps=1
            shift
            ;;
        --print-cubecl)
            print_cubecl=1
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
        -p*)
            echo "test-cpu.sh: use '-p CRATE' with a space, not '$1'" >&2
            exit 2
            ;;
        --workspace | --all | --exclude | --exclude=*)
            echo "test-cpu.sh: '$1' is not supported (omit -p for the whole workspace, or list crates with -p)" >&2
            exit 2
            ;;
        --)
            after_sep=1
            shift
            ;;
        -h | --help)
            sed -n '2,18p' "$script_path" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            cargo_args+=("$1")
            shift
            ;;
    esac
done

if [[ $with_rdeps -eq 1 && ${#packages[@]} -eq 0 ]]; then
    echo "test-cpu.sh: --with-rdeps requires at least one -p CRATE" >&2
    exit 2
fi

if [[ $print_cubecl -eq 0 && -n "${TETRALOMA_TEST_DEVICE:-}" && "${TETRALOMA_TEST_DEVICE}" != "cpu" ]]; then
    echo "test-cpu.sh: CPU-runtime script called with TETRALOMA_TEST_DEVICE=${TETRALOMA_TEST_DEVICE} (unset it or set it to cpu)" >&2
    exit 2
fi

threads=${TETRALOMA_CPU_TEST_THREADS:-1}
if ! [[ "$threads" =~ ^[1-9][0-9]*$ ]]; then
    echo "test-cpu.sh: TETRALOMA_CPU_TEST_THREADS must be a positive integer, got '$threads'" >&2
    exit 2
fi

# Classify selected workspace members into host-only vs CubeCL-dependent via reverse BFS from all
# `cubecl` / `cubecl-*` packages in the default-feature resolved graph (cycle-safe, order-independent).
classified=$(cargo metadata --format-version 1 | python3 -I -c '
from collections import deque
import json, sys

meta = json.load(sys.stdin)
members = set(meta["workspace_members"])
pkgs = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n["dependencies"] for n in meta["resolve"]["nodes"]}

with_rdeps = sys.argv[1] == "1"
requested = sys.argv[2:]

# Build reverse dependency edges over the resolved graph.
rdeps = {pid: [] for pid in nodes}
for pid, deps in nodes.items():
    for dep in deps:
        if dep in rdeps:
            rdeps[dep].append(pid)

# Reverse BFS from every cubecl / cubecl-* package to find every package that reaches CubeCL.
reaches_cubecl = set()
queue = deque()
for pid in nodes:
    name = pkgs[pid]["name"]
    if name == "cubecl" or name.startswith("cubecl-"):
        reaches_cubecl.add(pid)
        queue.append(pid)
while queue:
    cur = queue.popleft()
    for parent in rdeps.get(cur, ()):
        if parent not in reaches_cubecl:
            reaches_cubecl.add(parent)
            queue.append(parent)

by_name = {pkgs[pid]["name"]: pid for pid in members}
if requested:
    selected_ids = set()
    for name in requested:
        if name not in by_name:
            sys.stderr.write(f"test-cpu.sh: unknown workspace crate: {name}\n")
            sys.exit(2)
        selected_ids.add(by_name[name])
    if with_rdeps:
        q = deque(selected_ids)
        while q:
            cur = q.popleft()
            for parent in rdeps.get(cur, ()):
                if parent in members and parent not in selected_ids:
                    selected_ids.add(parent)
                    q.append(parent)
    selected = sorted(pkgs[pid]["name"] for pid in selected_ids)
else:
    selected = sorted(by_name.keys())

for name in selected:
    kind = "cubecl" if by_name[name] in reaches_cubecl else "host"
    print(f"{kind} {name}")
' "$with_rdeps" ${packages[@]+"${packages[@]}"})

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

if [[ $print_cubecl -eq 1 ]]; then
    out=()
    for c in ${cubecl_crates[@]+"${cubecl_crates[@]}"}; do
        out+=(-p "$c")
    done
    if [[ ${#out[@]} -gt 0 ]]; then
        printf '%s\n' "${out[*]}"
    fi
    exit 0
fi

no_fail_fast=0
for arg in ${cargo_args[@]+"${cargo_args[@]}"}; do
    if [[ "$arg" == "--no-fail-fast" ]]; then
        no_fail_fast=1
        break
    fi
done

run_cmd() {
    echo "+ $*" >&2
    if [[ $dry_run -eq 0 ]]; then
        "$@"
    fi
}

status=0
if [[ ${#host_crates[@]} -gt 0 ]]; then
    cmd=(cargo test)
    for c in "${host_crates[@]}"; do cmd+=(-p "$c"); done
    cmd+=(${cargo_args[@]+"${cargo_args[@]}"})
    if [[ ${#libtest_args[@]} -gt 0 ]]; then
        cmd+=(-- "${libtest_args[@]}")
    fi
    if run_cmd "${cmd[@]}"; then
        :
    else
        status=$?
        if [[ $no_fail_fast -eq 0 ]]; then
            exit "$status"
        fi
    fi
fi

if [[ ${#cubecl_crates[@]} -gt 0 ]]; then
    cmd=(cargo test)
    for c in "${cubecl_crates[@]}"; do cmd+=(-p "$c"); done
    cmd+=(${cargo_args[@]+"${cargo_args[@]}"})
    cmd+=(-- --test-threads="$threads" ${libtest_args[@]+"${libtest_args[@]}"})
    if run_cmd "${cmd[@]}"; then
        :
    else
        status=$?
    fi
fi

exit "$status"

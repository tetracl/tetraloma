#!/usr/bin/env python3
"""Generates the Basix oracle fixtures of M11 (docs/plans/M11.md, T1; docs/design.md, A.10, A.11).

Writes two JSON files with the pinned Basix wheel (PyPI fenics-basix==0.11.0, installed in .venv),
in Basix's own DOF order (the Rust tests apply Appendix A.11):

- tetraloma-refcell/tests/fixtures/gll_isaac.json:
    points of Basix's `gll_isaac` Lagrange elements on triangles, tetrahedra and prisms, p = 1..15.
- tetraloma-element/tests/fixtures/lagrange_basix.json:
    values and first derivatives at eight fixture points per cell for `Nodal(Gll)` on triangles,
    tetrahedra and prisms (p = 1..6), and `Nodal(Equispaced)` on prisms (p = 1..6) and pyramids
    (p = 1..3), none at the pyramid's apex.

Usage:
    .venv/bin/python scripts/fixtures/element/generate.py          # (re)write the fixtures
    .venv/bin/python scripts/fixtures/element/generate.py --check  # regenerate and compare within
                                                                   # tolerance (1e-15 points,
                                                                   # 1e-13 tabulations)
"""

import argparse
import json
import math
import sys
from pathlib import Path

try:
    import basix
    import numpy as np
except ImportError as e:
    sys.exit(f"generate.py: {e}; install the pin: .venv/bin/pip install fenics-basix==0.11.0")

PINNED = "0.11.0"
REPO = Path(__file__).resolve().parents[3]
GLL_ISAAC_PATH = REPO / "tetraloma-refcell" / "tests" / "fixtures" / "gll_isaac.json"
LAGRANGE_BASIX_PATH = REPO / "tetraloma-element" / "tests" / "fixtures" / "lagrange_basix.json"

POINT_TOL = 1e-15
TAB_TOL = 1e-13
NUM_FIXTURE_POINTS = 8

# Basix cell types and topological dimensions.
CELLS = {
    "Triangle": (basix.CellType.triangle, 2),
    "Tetrahedron": (basix.CellType.tetrahedron, 3),
    "Prism": (basix.CellType.prism, 3),
    "Pyramid": (basix.CellType.pyramid, 3),
}

# Eight reference points per cell: midpoint, interior points, face (3D), edge and vertex.
# Triangle and Tetrahedron match conformance/src/bin/export_fixtures.rs verbatim.
# None lies at the pyramid's apex (0, 0, 1).
FIXTURE_POINTS = {
    "Triangle": [
        [1.0 / 3.0, 1.0 / 3.0],  # midpoint
        [0.17, 0.23],            # interior
        [0.53, 0.29],            # interior
        [0.11, 0.71],            # interior
        [0.37, 0.0],             # edge 0 (y = 0)
        [0.0, 0.61],             # edge 1 (x = 0)
        [0.4, 0.6],              # edge 2 (x + y = 1)
        [1.0, 0.0],              # vertex 1
    ],
    "Tetrahedron": [
        [0.25, 0.25, 0.25],      # midpoint
        [0.13, 0.19, 0.29],      # interior
        [0.47, 0.17, 0.11],      # interior
        [0.31, 0.43, 0.0],       # face 0 (z = 0)
        [0.2, 0.3, 0.5],         # face 3 (x + y + z = 1)
        [0.63, 0.0, 0.0],        # edge 0 (y = z = 0)
        [0.0, 0.35, 0.65],       # edge 5 (x = 0, y + z = 1)
        [0.0, 0.0, 1.0],         # vertex 3
    ],
    "Prism": [
        [1.0 / 3.0, 1.0 / 3.0, 0.5],  # midpoint
        [0.17, 0.29, 0.23],           # interior
        [0.53, 0.19, 0.77],           # interior
        [0.23, 0.41, 0.0],            # triangular face 0 (z = 0)
        [0.35, 0.65, 0.61],           # quadrilateral face 3 (x + y = 1)
        [0.59, 0.0, 0.0],             # base edge 0 (y = z = 0)
        [0.0, 0.0, 0.83],             # vertical edge 2 (x = y = 0)
        [1.0, 0.0, 1.0],              # vertex 4
    ],
    "Pyramid": [
        [0.4, 0.4, 0.2],         # midpoint (mean of the five vertices)
        [0.17, 0.23, 0.35],      # interior
        [0.25, 0.15, 0.6],       # interior
        [0.31, 0.73, 0.0],       # quadrilateral base face 0 (z = 0)
        [0.65, 0.25, 0.35],      # triangular face 3 (x + z = 1)
        [0.59, 0.0, 0.0],        # base edge 0 (y = z = 0)
        [0.0, 0.0, 0.45],        # slanted edge 2 (x = y = 0, away from apex)
        [1.0, 1.0, 0.0],         # base vertex 3 (not the apex)
    ],
}

GLL_ISAAC_CELLS = ("Triangle", "Tetrahedron", "Prism")
GLL_ISAAC_DEGREES = range(1, 16)

TABULATION_CASES = {
    "Triangle": [("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7))],
    "Tetrahedron": [("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7))],
    "Prism": [
        ("Equispaced", basix.LagrangeVariant.equispaced, range(1, 7)),
        ("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7)),
    ],
    "Pyramid": [("Equispaced", basix.LagrangeVariant.equispaced, range(1, 4))],
}


def check_contiguous_dofs(el, label):
    flat = [d for dim_dofs in el.entity_dofs for ent in dim_dofs for d in ent]
    assert flat == list(range(el.dim)), f"{label}: non-contiguous entity_dofs {flat}"


def generate_gll_isaac():
    out = {}
    for cell_name in GLL_ISAAC_CELLS:
        ct, td = CELLS[cell_name]
        per_degree = {}
        for p in GLL_ISAAC_DEGREES:
            el = basix.create_element(basix.ElementFamily.P, ct, p, basix.LagrangeVariant.gll_isaac)
            check_contiguous_dofs(el, f"gll_isaac {cell_name} p={p}")
            pts = np.asarray(el.points, dtype=np.float64)
            assert pts.shape == (el.dim, td), f"{cell_name} p={p}: shape {pts.shape}"
            assert np.all(np.isfinite(pts)), f"{cell_name} p={p}: non-finite point"
            per_degree[str(p)] = pts.reshape(-1).tolist()
        out[cell_name] = per_degree
    return out


def generate_lagrange_basix():
    out = {}
    for cell_name, specs in TABULATION_CASES.items():
        ct, td = CELLS[cell_name]
        pts = np.asarray(FIXTURE_POINTS[cell_name], dtype=np.float64)
        assert pts.shape == (NUM_FIXTURE_POINTS, td), f"{cell_name}: points shape {pts.shape}"
        if cell_name == "Pyramid":
            assert np.all(pts[:, 2] < 1.0 - 1e-6), "Pyramid fixture point too close to apex"
        entry = {"points": pts.reshape(-1).tolist()}
        for variant_name, variant, degrees in specs:
            per_degree = {}
            for p in degrees:
                el = basix.create_element(basix.ElementFamily.P, ct, p, variant)
                check_contiguous_dofs(el, f"basix {cell_name} {variant_name} p={p}")
                tab = np.asarray(el.tabulate(1, pts), dtype=np.float64)
                assert tab.shape == (1 + td, NUM_FIXTURE_POINTS, el.dim, 1), (
                    f"{cell_name} {variant_name} p={p}: tab shape {tab.shape}"
                )
                tab = tab[:, :, :, 0]
                assert np.all(np.isfinite(tab)), f"{cell_name} {variant_name} p={p}: non-finite tab"
                # Self-check: partition of unity and zero derivative sum.
                val_err = float(np.max(np.abs(tab[0].sum(axis=1) - 1.0)))
                assert val_err < 1e-12, f"{cell_name} {variant_name} p={p}: sum(phi)-1 = {val_err}"
                for d in range(1, 1 + td):
                    scale = max(1.0, float(np.max(np.abs(tab[d]))))
                    d_err = float(np.max(np.abs(tab[d].sum(axis=1)))) / scale
                    assert d_err < 1e-12, (
                        f"{cell_name} {variant_name} p={p} deriv {d}: sum(dphi)/scale = {d_err}"
                    )
                per_degree[str(p)] = tab.reshape(-1).tolist()
            entry[variant_name] = per_degree
        out[cell_name] = entry
    return out


def write_compact_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")


def compare_float_list_abs(label, have, want, tol, errors):
    if not isinstance(have, list) or len(have) != len(want):
        errors.append(
            f"{label}: length mismatch (have {len(have) if isinstance(have, list) else type(have).__name__}, want {len(want)})"
        )
        return
    max_err = 0.0
    worst_idx = -1
    for i, (a, b) in enumerate(zip(have, want)):
        if not isinstance(a, (int, float)) or not math.isfinite(a):
            errors.append(f"{label}[{i}]: non-finite value {a!r}")
            return
        err = abs(a - b)
        if err > max_err:
            max_err = err
            worst_idx = i
    if max_err > tol:
        errors.append(
            f"{label}[{worst_idx}]: diff {max_err:.3e} > {tol:.1e} "
            f"(have {have[worst_idx]!r}, want {want[worst_idx]!r})"
        )


def compare_tabulation_blocks(label, td, have, want, tol, errors):
    if not isinstance(have, list) or len(have) != len(want):
        errors.append(
            f"{label}: length mismatch (have {len(have) if isinstance(have, list) else type(have).__name__}, want {len(want)})"
        )
        return
    nblocks = 1 + td
    assert len(want) % nblocks == 0
    block_len = len(want) // nblocks
    for d in range(nblocks):
        base = d * block_len
        have_b = have[base:base + block_len]
        want_b = want[base:base + block_len]
        scale = max(1.0, max((abs(v) for v in want_b), default=1.0))
        max_err = 0.0
        worst_idx = -1
        for i, (a, b) in enumerate(zip(have_b, want_b)):
            if not isinstance(a, (int, float)) or not math.isfinite(a):
                errors.append(f"{label} deriv {d}[{i}]: non-finite value {a!r}")
                return
            err = abs(a - b) / scale
            if err > max_err:
                max_err = err
                worst_idx = i
        if max_err > tol:
            errors.append(
                f"{label} deriv {d}[{worst_idx}]: rel diff {max_err:.3e} > {tol:.1e} "
                f"(have {have_b[worst_idx]!r}, want {want_b[worst_idx]!r})"
            )


def check_gll_isaac(fresh, errors):
    if not GLL_ISAAC_PATH.is_file():
        errors.append(f"missing {GLL_ISAAC_PATH}")
        return
    with open(GLL_ISAAC_PATH, encoding="utf-8") as f:
        have = json.load(f)
    if not isinstance(have, dict) or set(have.keys()) != set(fresh.keys()):
        errors.append(f"gll_isaac.json: cell keys differ (have {sorted(have.keys()) if isinstance(have, dict) else type(have).__name__}, want {sorted(fresh.keys())})")
        return
    for cell_name, want_degs in fresh.items():
        have_degs = have[cell_name]
        if not isinstance(have_degs, dict) or set(have_degs.keys()) != set(want_degs.keys()):
            errors.append(f"gll_isaac.json {cell_name}: degree keys differ")
            continue
        for deg, want_pts in want_degs.items():
            compare_float_list_abs(
                f"gll_isaac.json {cell_name} p={deg}",
                have_degs[deg],
                want_pts,
                POINT_TOL,
                errors,
            )


def check_lagrange_basix(fresh, errors):
    if not LAGRANGE_BASIX_PATH.is_file():
        errors.append(f"missing {LAGRANGE_BASIX_PATH}")
        return
    with open(LAGRANGE_BASIX_PATH, encoding="utf-8") as f:
        have = json.load(f)
    if not isinstance(have, dict) or set(have.keys()) != set(fresh.keys()):
        errors.append(f"lagrange_basix.json: cell keys differ")
        return
    for cell_name, want_entry in fresh.items():
        _, td = CELLS[cell_name]
        have_entry = have[cell_name]
        if not isinstance(have_entry, dict) or set(have_entry.keys()) != set(want_entry.keys()):
            errors.append(f"lagrange_basix.json {cell_name}: keys differ")
            continue
        compare_float_list_abs(
            f"lagrange_basix.json {cell_name} points",
            have_entry["points"],
            want_entry["points"],
            POINT_TOL,
            errors,
        )
        for key, want_val in want_entry.items():
            if key == "points":
                continue
            have_degs = have_entry[key]
            if not isinstance(have_degs, dict) or set(have_degs.keys()) != set(want_val.keys()):
                errors.append(f"lagrange_basix.json {cell_name} {key}: degree keys differ")
                continue
            for deg, want_tab in want_val.items():
                compare_tabulation_blocks(
                    f"lagrange_basix.json {cell_name} {key} p={deg}",
                    td,
                    have_degs[deg],
                    want_tab,
                    TAB_TOL,
                    errors,
                )


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument(
        "--check",
        action="store_true",
        help="regenerate and compare points to 1e-15 and tabulations to 1e-13",
    )
    args = parser.parse_args()

    version = basix.__version__
    if version != PINNED:
        sys.exit(
            f"generate.py: Basix {version}, need {PINNED}: "
            f".venv/bin/pip install fenics-basix=={PINNED}"
        )

    gll_isaac = generate_gll_isaac()
    lagrange_basix = generate_lagrange_basix()

    if not args.check:
        write_compact_json(GLL_ISAAC_PATH, gll_isaac)
        write_compact_json(LAGRANGE_BASIX_PATH, lagrange_basix)
        print(f"wrote {GLL_ISAAC_PATH}")
        print(f"wrote {LAGRANGE_BASIX_PATH}")
        return

    errors = []
    check_gll_isaac(gll_isaac, errors)
    check_lagrange_basix(lagrange_basix, errors)
    if errors:
        print("Basix fixtures differ from a fresh run (rerun generate.py and commit):", file=sys.stderr)
        for err in errors:
            print(f"  {err}", file=sys.stderr)
        sys.exit(1)
    print("Basix fixtures match a fresh run (gll_isaac <= 1e-15, lagrange_basix <= 1e-13)")


if __name__ == "__main__":
    main()

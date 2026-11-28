#!/usr/bin/env python3
"""Generates the Basix (M11) and Symfem (M12) element oracle fixtures (docs/design.md, A.10, A.11).

Writes three JSON files with the pinned Python wheels installed in .venv:
- fenics-basix==0.11.0 (Basix's own DOF order; the Rust tests apply Appendix A.11):
  - tetraloma-refcell/tests/fixtures/gll_isaac.json: gll_isaac points on triangles, tetrahedra and
    prisms, p = 1..15.
  - tetraloma-element/tests/fixtures/lagrange_basix.json: values and first derivatives at eight
    fixture points per cell for M11 A2 (none at the pyramid's apex).
- symfem==2025.12.0 (DefElement numbering, identical to tetraloma's, Appendix A.11):
  - tetraloma-element/tests/fixtures/hcurl_hdiv_dofs_symfem.json: sub-entity DOF counts for N1curl
    and RaviartThomas on triangles, quadrilaterals, tetrahedra and hexahedra, p = 1..8 (M12 A1).

Usage:
    .venv/bin/python scripts/fixtures/element/generate.py          # (re)write the fixtures
    .venv/bin/python scripts/fixtures/element/generate.py --check  # verify all three fixtures
"""

import argparse
from contextlib import contextmanager
import json
import sys
from pathlib import Path

try:
    import basix
    import numpy as np
    import symfem
    from symfem.functionals import BaseFunctional, IntegralMoment
except ImportError as e:
    sys.exit(
        f"generate.py: {e}; install the pins: "
        ".venv/bin/pip install fenics-basix==0.11.0 symfem==2025.12.0"
    )

PINNED_BASIX = "0.11.0"
PINNED_SYMFEM = "2025.12.0"
REPO = Path(__file__).resolve().parents[3]
GLL_ISAAC_PATH = REPO / "tetraloma-refcell" / "tests" / "fixtures" / "gll_isaac.json"
LAGRANGE_BASIX_PATH = REPO / "tetraloma-element" / "tests" / "fixtures" / "lagrange_basix.json"
SYMFEM_DOFS_PATH = (
    REPO / "tetraloma-element" / "tests" / "fixtures" / "hcurl_hdiv_dofs_symfem.json"
)
POINT_TOL, TAB_TOL, NUM_POINTS = 1e-15, 1e-13, 8

CELLS = {
    "Triangle": (basix.CellType.triangle, 2),
    "Tetrahedron": (basix.CellType.tetrahedron, 3),
    "Prism": (basix.CellType.prism, 3),
    "Pyramid": (basix.CellType.pyramid, 3),
}

# Symfem cell name, topological dimension, expected sub-entity counts, and (N1curl, RT) names.
# Symfem indexes all six families by polyset subdegree (order = p - 1 for superdegree p >= 1).
SYMFEM_CELLS = {
    "Triangle": ("triangle", 2, [3, 3, 1], {"N1curl": "N1curl", "RaviartThomas": "RT"}),
    "Quadrilateral": ("quadrilateral", 2, [4, 4, 1], {"N1curl": "RTCE", "RaviartThomas": "RTCF"}),
    "Tetrahedron": ("tetrahedron", 3, [4, 6, 4, 1], {"N1curl": "N1curl", "RaviartThomas": "RT"}),
    "Hexahedron": ("hexahedron", 3, [8, 12, 6, 1], {"N1curl": "NCE", "RaviartThomas": "NCF"}),
}
SYMFEM_FAMILIES = ("N1curl", "RaviartThomas")

# Eight points per cell (midpoint, interior, face on 3D, edge, vertex; none at pyramid apex).
# Triangle and Tetrahedron match conformance/src/bin/export_fixtures.rs verbatim.
FIXTURE_POINTS = {
    "Triangle": [
        [1.0 / 3.0, 1.0 / 3.0], [0.17, 0.23], [0.53, 0.29], [0.11, 0.71],
        [0.37, 0.0], [0.0, 0.61], [0.4, 0.6], [1.0, 0.0],
    ],
    "Tetrahedron": [
        [0.25, 0.25, 0.25], [0.13, 0.19, 0.29], [0.47, 0.17, 0.11], [0.31, 0.43, 0.0],
        [0.2, 0.3, 0.5], [0.63, 0.0, 0.0], [0.0, 0.35, 0.65], [0.0, 0.0, 1.0],
    ],
    "Prism": [
        [1.0 / 3.0, 1.0 / 3.0, 0.5], [0.17, 0.29, 0.23], [0.53, 0.19, 0.77], [0.23, 0.41, 0.0],
        [0.35, 0.65, 0.61], [0.59, 0.0, 0.0], [0.0, 0.0, 0.83], [1.0, 0.0, 1.0],
    ],
    "Pyramid": [
        [0.4, 0.4, 0.2], [0.17, 0.23, 0.35], [0.25, 0.15, 0.6], [0.31, 0.73, 0.0],
        [0.65, 0.25, 0.35], [0.59, 0.0, 0.0], [0.0, 0.0, 0.45], [1.0, 1.0, 0.0],
    ],
}

TABULATION_CASES = {
    "Triangle": [("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7))],
    "Tetrahedron": [("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7))],
    "Prism": [
        ("Equispaced", basix.LagrangeVariant.equispaced, range(1, 7)),
        ("Gll", basix.LagrangeVariant.gll_isaac, range(1, 7)),
    ],
    "Pyramid": [("Equispaced", basix.LagrangeVariant.equispaced, range(1, 4))],
}


def make_element(ct, p, variant):
    el = basix.create_element(basix.ElementFamily.P, ct, p, variant)
    flat = [d for dim_dofs in el.entity_dofs for ent in dim_dofs for d in ent]
    assert flat == list(range(el.dim)), f"{ct} {variant} p={p}: non-contiguous entity_dofs"
    return el


def generate_gll_isaac():
    out = {}
    for cell_name in ("Triangle", "Tetrahedron", "Prism"):
        ct, td = CELLS[cell_name]
        degs = {}
        for p in range(1, 16):
            el = make_element(ct, p, basix.LagrangeVariant.gll_isaac)
            pts = np.asarray(el.points, dtype=np.float64)
            assert pts.shape == (el.dim, td) and np.all(np.isfinite(pts))
            degs[str(p)] = pts.reshape(-1).tolist()
        out[cell_name] = degs
    return out


def generate_lagrange_basix():
    out = {}
    for cell_name, specs in TABULATION_CASES.items():
        ct, td = CELLS[cell_name]
        pts = np.asarray(FIXTURE_POINTS[cell_name], dtype=np.float64)
        assert pts.shape == (NUM_POINTS, td)
        if cell_name == "Pyramid":
            assert np.all(pts[:, 2] < 1.0 - 1e-6), "Pyramid fixture point too close to apex"
        entry = {"points": pts.reshape(-1).tolist()}
        for vname, variant, degrees in specs:
            degs = {}
            for p in degrees:
                el = make_element(ct, p, variant)
                tab = np.asarray(el.tabulate(1, pts), dtype=np.float64)
                assert tab.shape == (1 + td, NUM_POINTS, el.dim, 1) and np.all(np.isfinite(tab))
                tab = tab[:, :, :, 0]
                assert np.max(np.abs(tab[0].sum(axis=1) - 1.0)) < 1e-12
                for d in range(1, 1 + td):
                    scale = max(1.0, float(np.max(np.abs(tab[d]))))
                    assert np.max(np.abs(tab[d].sum(axis=1))) / scale < 1e-12
                degs[str(p)] = tab.reshape(-1).tolist()
            entry[vname] = degs
        out[cell_name] = entry
    return out


@contextmanager
def _symfem_entity_dofs_only():
    """Avoid eager symbolic basis inversion inside Symfem's IntegralMoment.__init__.

    Symfem's make_integral_moment_dofs passes lazy ElementBasisFunction instances so creating an
    element does not invert symbolic dual matrices, but IntegralMoment.__init__ eagerly evaluates
    len(self.f) and maps self.f on 2D faces of 3D cells (triggering symbolic rational LU inversion
    on e.g. RaviartThomas(quad, 6) for NCE_8). entity_dofs(d, e) only inspects each functional's
    entity tuple, so skipping the unused weight-function mapping preserves entity_dofs, space_dim
    and lagrange_superdegree identically while finishing in seconds.
    """
    orig_init = IntegralMoment.__init__

    def _fast_init(self, reference, f_in, dof, entity, mapping="identity", map_function=True):
        BaseFunctional.__init__(self, reference, entity, mapping)
        self.dof = dof

    IntegralMoment.__init__ = _fast_init
    try:
        yield
    finally:
        IntegralMoment.__init__ = orig_init


def _symfem_counts(symfem_cell, symfem_family, td, sub_counts, p):
    el = symfem.create_element(symfem_cell, symfem_family, p - 1)
    assert el.lagrange_superdegree == p, f"{symfem_family} {symfem_cell} p={p}: superdegree {el.lagrange_superdegree}"
    ref = el.reference
    assert ref.tdim == td and [ref.sub_entity_count(d) for d in range(td + 1)] == sub_counts
    all_dofs, counts = [], []
    for d in range(td + 1):
        row = []
        for e in range(sub_counts[d]):
            edofs = el.entity_dofs(d, e)
            all_dofs.extend(edofs)
            row.append(len(edofs))
        counts.append(row)
    assert sorted(all_dofs) == list(range(el.space_dim)), f"{symfem_family} {symfem_cell} p={p}: DOF partition"
    return counts


def generate_hcurl_hdiv_dofs_symfem():
    # 2D cells run through unmodified Symfem for all p = 1..=8; 3D cells cross-check p = 1, 2
    # against unmodified Symfem and use the fast IntegralMoment path for all p = 1..=8.
    out = {fam: {} for fam in SYMFEM_FAMILIES}
    unmodified_3d = {}
    for fam in SYMFEM_FAMILIES:
        for cell, (sc, td, sub, fams) in SYMFEM_CELLS.items():
            if td == 2:
                out[fam][cell] = {
                    str(p): _symfem_counts(sc, fams[fam], td, sub, p) for p in range(1, 9)
                }
            else:
                for p in (1, 2):
                    unmodified_3d[(fam, cell, p)] = _symfem_counts(sc, fams[fam], td, sub, p)

    with _symfem_entity_dofs_only():
        for fam in SYMFEM_FAMILIES:
            for cell, (sc, td, sub, fams) in SYMFEM_CELLS.items():
                if td != 3:
                    continue
                degs = {}
                for p in range(1, 9):
                    counts = _symfem_counts(sc, fams[fam], td, sub, p)
                    if (fam, cell, p) in unmodified_3d:
                        assert counts == unmodified_3d[(fam, cell, p)], f"{fam} {cell} p={p}: fast path mismatch"
                    degs[str(p)] = counts
                out[fam][cell] = degs
    return out


def check_array(label, have, want, tol, errors, nblocks=1):
    if not isinstance(have, list) or len(have) != len(want):
        errors.append(f"{label}: length mismatch")
        return
    if not all(isinstance(x, (int, float)) and not isinstance(x, bool) for x in have):
        errors.append(f"{label}: non-numeric entry")
        return
    a, b = np.asarray(have, dtype=np.float64), np.asarray(want, dtype=np.float64)
    if not np.all(np.isfinite(a)):
        errors.append(f"{label}: non-finite value")
        return
    for d, (ab, bb) in enumerate(zip(a.reshape(nblocks, -1), b.reshape(nblocks, -1))):
        scale = 1.0 if nblocks == 1 else max(1.0, float(np.max(np.abs(bb))))
        err = float(np.max(np.abs(ab - bb))) / scale
        if err > tol:
            errors.append(f"{label} block {d}: diff {err:.3e} > {tol:.1e}")


def check_tree(path, fresh, errors, tabulations=False):
    if not path.is_file():
        errors.append(f"missing {path}")
        return
    have = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(have, dict) or set(have) != set(fresh):
        errors.append(f"{path.name}: top-level keys differ")
        return
    for cell_name, want_c in fresh.items():
        have_c = have[cell_name]
        if not isinstance(have_c, dict) or set(have_c) != set(want_c):
            errors.append(f"{path.name} {cell_name}: keys differ")
            continue
        if not tabulations:
            for deg, want_pts in want_c.items():
                check_array(f"{path.name} {cell_name} p={deg}", have_c[deg], want_pts, POINT_TOL, errors)
            continue
        _, td = CELLS[cell_name]
        check_array(f"{path.name} {cell_name} points", have_c["points"], want_c["points"], POINT_TOL, errors)
        for key, want_degs in want_c.items():
            if key == "points":
                continue
            have_degs = have_c[key]
            if not isinstance(have_degs, dict) or set(have_degs) != set(want_degs):
                errors.append(f"{path.name} {cell_name} {key}: degree keys differ")
                continue
            for deg, want_tab in want_degs.items():
                check_array(
                    f"{path.name} {cell_name} {key} p={deg}",
                    have_degs[deg], want_tab, TAB_TOL, errors, nblocks=1 + td,
                )


def check_symfem_dofs(path, fresh, errors):
    if not path.is_file():
        errors.append(f"missing {path}")
        return
    have = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(have, dict) or set(have) != set(fresh):
        errors.append(f"{path.name}: family keys differ")
        return
    for fam_name, want_cells in fresh.items():
        have_cells = have[fam_name]
        if not isinstance(have_cells, dict) or set(have_cells) != set(want_cells):
            errors.append(f"{path.name} {fam_name}: cell keys differ")
            continue
        for cell_name, want_degs in want_cells.items():
            have_degs = have_cells[cell_name]
            if not isinstance(have_degs, dict) or set(have_degs) != set(want_degs):
                errors.append(f"{path.name} {fam_name} {cell_name}: degree keys differ")
                continue
            for deg, want_counts in want_degs.items():
                have_counts = have_degs[deg]
                valid = (
                    isinstance(have_counts, list)
                    and all(
                        isinstance(row, list)
                        and all(isinstance(x, int) and not isinstance(x, bool) for x in row)
                        for row in have_counts
                    )
                )
                if not valid or have_counts != want_counts:
                    errors.append(
                        f"{path.name} {fam_name} {cell_name} p={deg}: "
                        f"{have_counts!r} != {want_counts!r}"
                    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument(
        "--check",
        action="store_true",
        help="regenerate and verify Basix (1e-15 / 1e-13) and Symfem (exact integer) fixtures",
    )
    args = parser.parse_args()
    if basix.__version__ != PINNED_BASIX:
        sys.exit(
            f"generate.py: Basix {basix.__version__}, need {PINNED_BASIX}: "
            f".venv/bin/pip install fenics-basix=={PINNED_BASIX}"
        )
    if symfem.__version__ != PINNED_SYMFEM:
        sys.exit(
            f"generate.py: Symfem {symfem.__version__}, need {PINNED_SYMFEM}: "
            f".venv/bin/pip install symfem=={PINNED_SYMFEM}"
        )

    gll_isaac = generate_gll_isaac()
    lagrange_basix = generate_lagrange_basix()
    symfem_dofs = generate_hcurl_hdiv_dofs_symfem()
    if not args.check:
        for path, data in (
            (GLL_ISAAC_PATH, gll_isaac),
            (LAGRANGE_BASIX_PATH, lagrange_basix),
            (SYMFEM_DOFS_PATH, symfem_dofs),
        ):
            path.parent.mkdir(parents=True, exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                json.dump(data, f, sort_keys=True, separators=(",", ":"))
                f.write("\n")
            print(f"wrote {path}")
        return

    errors = []
    check_tree(GLL_ISAAC_PATH, gll_isaac, errors, tabulations=False)
    check_tree(LAGRANGE_BASIX_PATH, lagrange_basix, errors, tabulations=True)
    check_symfem_dofs(SYMFEM_DOFS_PATH, symfem_dofs, errors)
    if errors:
        print("Element fixtures differ from a fresh run (rerun generate.py and commit):", file=sys.stderr)
        for err in errors:
            print(f"  {err}", file=sys.stderr)
        sys.exit(1)
    print(
        "Element fixtures match a fresh run "
        "(gll_isaac <= 1e-15, lagrange_basix <= 1e-13, hcurl_hdiv_dofs_symfem exact)"
    )


if __name__ == "__main__":
    main()

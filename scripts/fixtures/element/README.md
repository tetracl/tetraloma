# Basix element oracle fixtures (M11)

`generate.py` writes the committed Basix oracle fixtures with the pinned Basix Python wheel,
`fenics-basix==0.11.0` from PyPI (`docs/plans/M11.md`, T1; `docs/design.md`, A.10 and A.11). It
refuses to run with any other Basix version.

```sh
python3.14 -m venv .venv && .venv/bin/pip install fenics-basix==0.11.0
.venv/bin/python scripts/fixtures/element/generate.py          # rewrite the fixtures
.venv/bin/python scripts/fixtures/element/generate.py --check  # regenerate and compare within
                                                               # tolerance (1e-15 points,
                                                               # 1e-13 tabulations)
```

## Files

Both files store points and basis functions in **Basix's own DOF order**; the Rust acceptance
tests apply the Appendix A.11 sub-entity conversion when comparing against tetraloma.

- `tetraloma-refcell/tests/fixtures/gll_isaac.json` (M11 A3):
  `{ "<Cell>": { "<p>": [f64; N(p) * tdim] } }` for `Triangle`, `Tetrahedron` and `Prism`,
  degrees `p = 1..=15`, flat node-major (`[node][coord]`). Exported from Basix's
  `LagrangeVariant.gll_isaac` Lagrange elements.
- `tetraloma-element/tests/fixtures/lagrange_basix.json` (M11 A2):
  `{ "<Cell>": { "points": [f64; 8 * tdim], "<Variant>": { "<p>": [f64; (1 + tdim) * 8 * dim] } } }`
  for:
  - `Triangle` and `Tetrahedron`: `Gll` (`LagrangeVariant.gll_isaac`), `p = 1..=6`;
  - `Prism`: `Equispaced` and `Gll`, `p = 1..=6`;
  - `Pyramid`: `Equispaced`, `p = 1..=3`.

  Tabulation entries are flat in `[derivative][point][basis]` order (block 0 is values, block
  `1 + d` is the partial derivative along reference coordinate `d`). The eight fixture points per
  cell cover the cell midpoint, interior points, face points (3D), edge points and a vertex; none
  lies at the pyramid's apex `(0, 0, 1)`. The triangle and tetrahedron fixture points match those
  of `conformance/src/bin/export_fixtures.rs`.

## Verification

Before writing, `generate.py` checks that Basix's `entity_dofs` are contiguous `0..dim`, every
point and tabulation entry is finite, values sum to 1 (partition of unity), and first derivatives
sum to 0 at every fixture point.

`--check` regenerates both datasets in memory and checks exact keys and array lengths, comparing
point coordinates to `1e-15` (absolute) and tabulations to `1e-13` (relative to
`max(1.0, max |block|)` per derivative block).

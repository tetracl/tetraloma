# Basix element oracle fixtures (M11)

`generate.py` writes the committed Basix oracle fixtures with the pinned Basix Python wheel,
`fenics-basix==0.11.0` from PyPI (`docs/plans/M11.md`, T1; `docs/design.md`, A.10 and A.11). It
refuses to run with any other Basix version.

```sh
python3.14 -m venv .venv && .venv/bin/pip install fenics-basix==0.11.0
.venv/bin/python scripts/fixtures/element/generate.py          # rewrite the fixtures
.venv/bin/python scripts/fixtures/element/generate.py --check  # compare points (1e-15) and
                                                               # tabulations (1e-13)
```

## Files

Both files store points and basis functions in Basix's DOF order; the Rust acceptance tests apply
the Appendix A.11 sub-entity conversion when comparing against tetraloma.

- `tetraloma-refcell/tests/fixtures/gll_isaac.json` (M11 A3):
  `{ "<Cell>": { "<p>": [f64; N(p) * tdim] } }` for `Triangle`, `Tetrahedron` and `Prism`,
  `p = 1..=15`, flat node-major (`[node][coord]`), from `LagrangeVariant.gll_isaac`.
- `tetraloma-element/tests/fixtures/lagrange_basix.json` (M11 A2):
  `{ "<Cell>": { "points": [f64; 8 * tdim], "<Variant>": { "<p>": [f64; (1 + tdim) * 8 * dim] } } }`
  for `Gll` on triangles, tetrahedra and prisms (`p = 1..=6`), and `Equispaced` on prisms
  (`p = 1..=6`) and pyramids (`p = 1..=3`). Tabulations are flat in `[derivative][point][basis]`
  order. The eight fixture points per cell cover the midpoint, interior, face (3D), edge and
  vertex; none lies at the pyramid's apex `(0, 0, 1)`. Triangle and tetrahedron points match
  `conformance/src/bin/export_fixtures.rs`.

## Verification

Before writing, `generate.py` verifies contiguous `entity_dofs`, finite values, partition of
unity, and zero derivative sums. `--check` regenerates in memory and checks exact keys and
lengths, comparing points to `1e-15` (absolute) and tabulations to `1e-13` (relative to
`max(1.0, max |block|)` per derivative block).

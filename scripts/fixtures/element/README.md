# Basix and Symfem element oracle fixtures (M11, M12)

`generate.py` writes the committed Basix and Symfem oracle fixtures with the pinned Python wheels,
`fenics-basix==0.11.0` and `symfem==2025.12.0` from PyPI (`docs/plans/M11.md`, T1; `docs/plans/M12.md`,
T1; `docs/design.md`, A.10 and A.11; ADR-0080). It refuses to run with any other version of either
package.

```sh
python3.14 -m venv .venv && .venv/bin/pip install fenics-basix==0.11.0 symfem==2025.12.0
.venv/bin/python scripts/fixtures/element/generate.py          # rewrite the fixtures
.venv/bin/python scripts/fixtures/element/generate.py --check  # verify all three fixtures
```

## Files

- `tetraloma-refcell/tests/fixtures/gll_isaac.json` (M11 A3, Basix):
  `{ "<Cell>": { "<p>": [f64; N(p) * tdim] } }` for `Triangle`, `Tetrahedron` and `Prism`,
  `p = 1..=15`, flat node-major (`[node][coord]`), from `LagrangeVariant.gll_isaac` in Basix's
  DOF order (the Rust tests apply Appendix A.11).
- `tetraloma-element/tests/fixtures/lagrange_basix.json` (M11 A2, Basix):
  `{ "<Cell>": { "points": [f64; 8 * tdim], "<Variant>": { "<p>": [f64; (1 + tdim) * 8 * dim] } } }`
  for `Gll` on triangles, tetrahedra and prisms (`p = 1..=6`), and `Equispaced` on prisms
  (`p = 1..=6`) and pyramids (`p = 1..=3`), in Basix's DOF order (Appendix A.11). Tabulations are
  flat in `[derivative][point][basis]` order. The eight fixture points per cell cover the midpoint,
  interior, face (3D), edge and vertex; none lies at the pyramid's apex `(0, 0, 1)`. Triangle and
  tetrahedron points match `conformance/src/bin/export_fixtures.rs`.
- `tetraloma-element/tests/fixtures/hcurl_hdiv_dofs_symfem.json` (M12 A1, Symfem):
  `{ "<Family>": { "<Cell>": { "<p>": [[usize; num_sub_entities(cell, d)]; tdim + 1] } } }`
  for `N1curl` (`N1curl` / `RTCE` / `NCE`) and `RaviartThomas` (`RT` / `RTCF` / `NCF`) on
  `Triangle`, `Quadrilateral`, `Tetrahedron` and `Hexahedron`, `p = 1..=8` (polyset superdegree;
  Symfem's `order = p - 1`), in DefElement sub-entity numbering (identical to tetraloma's,
  Appendix A.11).

## Verification

Before writing, `generate.py` verifies:

- Basix fixtures: contiguous `entity_dofs`, finite values, partition of unity, and zero derivative
  sums.
- Symfem fixture: `lagrange_superdegree == p`, exact `tdim` and sub-entity counts, exact partition
  of `0..space_dim`, unmodified Symfem on 2D cells (`p = 1..=8`), and agreement with unmodified
  Symfem on 3D cells at `p = 1, 2`.

`--check` regenerates in memory and checks exact keys and lengths, comparing Basix points to
`1e-15` (absolute), Basix tabulations to `1e-13` (relative to `max(1.0, max |block|)` per
derivative block), and Symfem sub-entity DOF counts for exact integer equality.

# ADR-0076: `tetraloma-device` re-exports `ServerError` for `tetraloma-parallel`

Status: Proposed
Date: 2026-11-23
Decision issue: #102
Raised by: M18.T0 (#92, pull request #101, Round 1 review findings A-F3 and F-F1)

## Context

`docs/design.md` leaves open how `tetraloma-parallel` names `cubecl::server::ServerError` for `ParallelError::Server(ServerError)`:

1. Section 11 (Listing S2c, ADR-0074) gives `tetraloma-parallel` the error enum `ParallelError` with the variant `Server(ServerError)` and `From<ServerError>`.
2. Section 18 (crate table and dependency rule 1) gives `tetraloma-parallel` no `cubecl` dependency (`tetraloma-comm`, `tetraloma-device`, `bytemuck`) and restricts `cubecl` dependencies to crates that define or launch kernels.
3. Section 7 (paragraph after Listing S1b, ADR-0026) and section 18 (crate table, row `tetraloma-device`) list `Client`, `CubeElement` (ADR-0026) and `ReduceOp` (ADR-0074) as re-exports of `tetraloma-device`, not `ServerError`.
4. `ParallelError::Server(ServerError)` is a public variant once M18.T5 unhides `ParallelError`, so `ServerError` is part of the public error surface rather than an internal workspace-only item.

## Options

1. **Keep the design as written.** `tetraloma-parallel` has no way to name `ServerError` for `ParallelError::Server(ServerError)` and `From<ServerError>` without violating either the crate table of section 18 or the hidden-item rule of dependency rule 7.
2. **Public re-export in `tetraloma-device` (recommended).** `tetraloma-device` publicly re-exports `cubecl::server::ServerError` beside `Client` and `CubeElement`, and `scripts/doc_audit.py`'s `FOREIGN_INLINED` adds `tetraloma_device/enum.ServerError.html`. `tetraloma-parallel` stays free of a direct `cubecl` dependency (same pattern as ADR-0026 item 12), and callers that match `DeviceError::Server` or `ParallelError::Server` can name `ServerError` through `tetraloma-device`.
3. **Hidden re-export in `tetraloma-device` under dependency rule 7.** `tetraloma-device` exposes `#[doc(hidden)] pub use cubecl::server::ServerError` and lists it under "Hidden items" in `docs/plans/M18.md`. Keeps `tetraloma-parallel` free of `cubecl` and leaves `FOREIGN_INLINED` unchanged, but labels a public variant's payload type as not part of the stable API.
4. **Add `cubecl` (without runtime features) to `tetraloma-parallel`.** Lets `tetraloma-parallel` import `cubecl::server::ServerError` directly, at the cost of widening dependency rule 1 beyond crates that define or launch kernels.

## Recommendation

Option 2. It follows ADR-0026 item 12 (`Client` and `CubeElement`), keeps `tetraloma-parallel` free of a direct `cubecl` dependency, and matches the fact that `ServerError` is already part of the public error surface of `DeviceError` and `ParallelError`.

## Changes if accepted

- **Design sections (`docs/design.md`):**
  - **Section 7, paragraph after Listing S1b:** list `ServerError (ADR-0076)` beside `Client` and `CubeElement` as re-exported at the root of `tetraloma-device` for `tetraloma-parallel`, which has no `cubecl` dependency.
  - **Section 18, crate table, row `tetraloma-device`:** list `ServerError (ADR-0076)` beside `Client`, `CubeElement` and `ReduceOp`.
- **Interfaces and tests:**
  - New public re-export: `tetraloma_device::ServerError` (`pub use cubecl::server::ServerError;`), documented with a book link as `Client` and `CubeElement` are.
  - `scripts/doc_audit.py`: add `"tetraloma_device/enum.ServerError.html"` to `FOREIGN_INLINED`.
- **Process documents:** `docs/adr/README.md` (the ADR-0076 row).
- **Plans and tasks affected:** `docs/plans/M18.md` (Decisions table, and T0's `Touches` adds `scripts/doc_audit.py` for `FOREIGN_INLINED`). M18.T0 (#92) and M18.T5 (#97) use `tetraloma_device::ServerError`.
- **New tasks:** none.

## Decision

*(Pending.)*

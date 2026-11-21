# M1: Gauss-Jacobi rules on the interval

Design: docs/design.md sections 3, 4, 7
Depends on: none
Milestone issue: #1

## Scope

Gauss-Jacobi points and weights on [0, 1] for 1 <= n <= 30, and the
reference tables that check them. Lobatto and Radau variants, n above 30 and
tables for beta other than 0 are left out.

## Acceptance

| ID | Criterion (from the roadmap) | Test | Enabled by |
| --- | --- | --- | --- |
| A1 | Monomials up to degree 2n - 1 integrate to relative error 1e-13 | accept::m1_exactness | T2 |
| A2 | Points and weights agree with the reference tables to 1e-13 | accept::m1_tables_agree | T2 |
| A3 | Points strictly inside (0, 1), increasing; weights positive | accept::m1_points_and_weights | T2 |
| A4 | n = 0, n = 31 and alpha = -1 return Unsupported | accept::m1_unsupported | T1 |
| A5 | Reference tables bit-identical to committed checksums | tests::tables_bit_identity | T3 |

## Tasks

### T0: acceptance tests

All rows of the table as ignored tests, each marked with the task that
enables it, against stubs of the public interface. The repository has no
crate yet, so this task also adds the minimal Cargo setup and proposes the
check commands for AGENTS.md.

- Sign-off needed: the check commands

### T1: Jacobi recurrence coefficients and input checks

- Needs: T0
- Touches: src/jacobi.rs, src/error.rs
- Size: about 250 lines
- Protected paths: none
- Sign-off needed: none
- Verification: full tier
- Trivial (skip review): no

### T2: Golub-Welsch points and weights

- Needs: T1, T3
- Touches: src/jacobi.rs, src/eigen.rs
- Size: about 400 lines
- Protected paths: none
- Sign-off needed: none
- Verification: full tier
- Trivial (skip review): no

### T3: reference tables and their check

- Needs: T0
- Touches: tables/, tools/make_tables.py, src/tables.rs
- Size: about 300 lines
- Protected paths: tables/ (tests::tables_bit_identity)
- Sign-off needed: number of stored digits
- Verification: full tier
- Trivial (skip review): no

## Verification set

The milestone tier runs the full test suite once on the development machine.
No remote runs.

## Decisions

| Date | Question | Answer (maintainer's words) | Issue |
| --- | --- | --- | --- |

## Deviations

| Task | What differed from the plan | Why | Pull request |
| --- | --- | --- | --- |

## Open questions

- How many digits should the stored tables carry? Needed before T3.

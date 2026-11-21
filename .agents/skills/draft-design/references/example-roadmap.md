# quadrules: roadmap

Milestones in intended order. Their state is on GitHub, not in this file.

## M1: Gauss-Jacobi rules on the interval

Depends on: none
Design: 3, 4, 7
Acceptance:
- For n = 1 to 30 and alpha in {0, 1, 2} with beta = 0, the rule integrates
  every monomial up to degree 2n - 1 against the Jacobi weight with relative
  error at most 1e-13.
- For the same n and alpha, points and weights agree with the reference
  tables to relative error at most 1e-13.
- Points lie strictly inside (0, 1) in increasing order, and all weights
  are positive.
- n = 0, n = 31 and alpha = -1 each return QuadError::Unsupported.
- The stored reference tables are bit-identical to the committed checksums.
Out of scope: Lobatto and Radau variants; n above 30; beta other than 0 in
the tables.
Runs: none

## M2: Rules on the triangle

Depends on: M1
Design: 3, 5
Acceptance:
- For degree = 1 to 30, the rule integrates every monomial x^i y^j with
  i + j <= degree with relative error at most 1e-13.
- For degree = 1 to 30, the rule has (degree / 2 + 1)^2 points, all inside
  the reference triangle.
- Weights are positive and sum to 1/2 with absolute error at most 1e-14.
- degree = 0 and degree = 31 each return QuadError::Unsupported.
Out of scope: symmetric rules with fewer points (design section 9).
Runs: none

## M3: Rules on the tetrahedron

Depends on: M1
Design: 3, 6
Acceptance:
- For degree = 1 to 30, the rule integrates every monomial x^i y^j z^k with
  i + j + k <= degree with relative error at most 1e-12.
- For degree = 1 to 30, the rule has (degree / 2 + 1)^3 points, all inside
  the reference tetrahedron.
- Weights are positive and sum to 1/6 with absolute error at most 1e-14.
- degree = 0 and degree = 31 each return QuadError::Unsupported.
Out of scope: symmetric rules with fewer points (design section 9).
Runs: none

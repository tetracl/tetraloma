# quadrules: design

## 1. Purpose

quadrules is a Rust library that provides numerical quadrature rules on the
interval, the triangle and the tetrahedron, for finite and boundary element
codes. Rules are computed on demand. Reference tables ship with the library
so that computed rules can be checked.

## 2. Glossary

- Rule: a set of points and weights on a reference cell.
- Degree: the highest total polynomial degree a rule integrates exactly.
- Reference interval: [0, 1].
- Reference triangle: vertices (0,0), (1,0), (0,1).
- Reference tetrahedron: vertices (0,0,0), (1,0,0), (0,1,0), (0,0,1).
- Jacobi weight: (1 - x)^alpha * x^beta on [0, 1].

## 3. Conventions

3.1 All public numerical values are f64.
3.2 Points are stored in one flat array, point after point: x0, y0, x1, y1, ...
3.3 Weights sum to the volume of the reference cell: 1, 1/2 and 1/6.
3.4 Public functions return Result<_, QuadError>. Library code never panics.
3.5 Comparisons with reference values use a relative tolerance of 1e-13
    unless a section states another.
3.6 No unsafe code.

## 4. Gauss-Jacobi rules on the interval

### Purpose
Rules for the Jacobi weight on [0, 1]. The triangle and tetrahedron rules
are built from them.

### Interface
    pub fn gauss_jacobi(n: usize, alpha: f64, beta: f64)
        -> Result<Rule1D, QuadError>

### Behaviour and invariants
- Returns n points and n weights.
- Exact for polynomials up to degree 2n - 1 against the Jacobi weight.
- Points lie strictly inside (0, 1) in increasing order. Weights are positive.
- Computed by the Golub-Welsch method from the three-term recurrence.
- Supported: 1 <= n <= 30 and alpha, beta > -1. Anything else returns
  QuadError::Unsupported.

### Out of scope
Gauss-Lobatto and Gauss-Radau variants.

## 5. Rules on the triangle

### Purpose
Rules of a requested degree on the reference triangle.

### Interface
    pub fn triangle(degree: usize) -> Result<Rule, QuadError>

### Behaviour and invariants
- Collapsed-coordinate product of gauss_jacobi(n, 1, 0) and
  gauss_jacobi(n, 0, 0), with n = degree / 2 + 1 (integer division).
- Returns n^2 points. Exact for all monomials of total degree <= degree.
- Points lie inside the triangle. Weights are positive and sum to 1/2.
- Supported: 1 <= degree <= 30.

### Out of scope
Symmetric rules with fewer points (see section 9).

## 6. Rules on the tetrahedron

### Purpose
Rules of a requested degree on the reference tetrahedron.

### Interface
    pub fn tetrahedron(degree: usize) -> Result<Rule, QuadError>

### Behaviour and invariants
- Collapsed-coordinate product of gauss_jacobi(n, 2, 0),
  gauss_jacobi(n, 1, 0) and gauss_jacobi(n, 0, 0), with n = degree / 2 + 1.
- Returns n^3 points. Exact for all monomials of total degree <= degree.
- Points lie inside the tetrahedron. Weights are positive and sum to 1/6.
- Supported: 1 <= degree <= 30.

### Out of scope
Symmetric rules with fewer points (see section 9).

## 7. Reference tables

### Purpose
Independent values to check computed rules against.

### Interface
    pub fn reference(n: usize, alpha: u8) -> Option<&'static Rule1D>

### Behaviour and invariants
- tables/ holds points and weights for n = 1 to 30, alpha in {0, 1, 2},
  beta = 0, computed in extended precision by tools/make_tables.py and
  rounded to f64.
- Stored values never change except through an ADR.

### Out of scope
Tables for beta other than 0.

## 8. Protected paths

| Path    | What must not change | Check                      |
|---------|----------------------|----------------------------|
| tables/ | Any stored value     | tests::tables_bit_identity |

## 9. Open questions

- Should symmetric rules with fewer points be added for the triangle and
  the tetrahedron? Not needed for M1 to M3.
- Should rules be usable at compile time? Undecided.

## 10. Decisions

The index of decisions is docs/adr/README.md. Text changed by a decision
carries an inline marker such as (ADR-0003).

//! Dense LU factorisation with partial pivoting, so tetraloma-refcell needs no linear-algebra
//! dependency (docs/design.md, "Tabulation and quadrature").

use crate::RefCellError;

// Design: section 26 ("Finite elements"), Listing E1.
/// Dense LU factorisation of a square `f64` matrix with partial pivoting: P A = L U.
///
/// L is unit lower triangular and U is upper triangular. [`DenseLu::factor`] builds the factors
/// once and [`DenseLu::solve`] applies them in place to any number of right-hand sides. See
/// [Orthonormal polynomial sets] for how tetraloma uses this solver when constructing reference
/// elements.
///
/// # Example
///
/// ```
/// use tetraloma_refcell::DenseLu;
///
/// // Solve A x = b for A = [[2, 1], [1, 3]] and b = [5, 10] -> x = [1, 3].
/// let lu = DenseLu::factor(vec![2.0, 1.0, 1.0, 3.0], 2).unwrap();
/// let mut rhs = [5.0, 10.0];
/// lu.solve(&mut rhs, 1);
/// assert!((rhs[0] - 1.0).abs() < 1e-15);
/// assert!((rhs[1] - 3.0).abs() < 1e-15);
/// ```
///
/// [Orthonormal polynomial sets]: https://tetracl.github.io/tetraloma/concepts/quadrature.html#orthonormal-polynomial-sets
#[derive(Clone, Debug)]
pub struct DenseLu {
    n: usize,
    lu: Vec<f64>,
    /// Row k of P A is row `perm[k]` of A.
    perm: Vec<usize>,
}

impl DenseLu {
    // Design: section 26 ("Finite elements"), Listing E1.
    /// Factors the row-major n × n matrix `a` with partial pivoting.
    ///
    /// See [Orthonormal polynomial sets] for where element construction uses this factorisation.
    ///
    /// # Errors
    ///
    /// [`RefCellError::Singular`] if a pivot is zero or at most n · ε times the largest entry
    /// of `a` in magnitude: the matrix is singular to working precision.
    ///
    /// # Panics
    ///
    /// If `a.len() != n * n` (or `n * n` overflows `usize`).
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_refcell::{DenseLu, RefCellError};
    ///
    /// let lu = DenseLu::factor(vec![0.0, 1.0, 1.0, 0.0], 2)?;
    /// let mut b = [3.0, 4.0];
    /// lu.solve(&mut b, 1);
    /// assert_eq!(b, [4.0, 3.0]);
    ///
    /// // A rank-deficient matrix returns Singular.
    /// assert_eq!(
    ///     DenseLu::factor(vec![1.0, 2.0, 2.0, 4.0], 2).unwrap_err(),
    ///     RefCellError::Singular
    /// );
    /// # Ok::<(), RefCellError>(())
    /// ```
    ///
    /// [Orthonormal polynomial sets]: https://tetracl.github.io/tetraloma/concepts/quadrature.html#orthonormal-polynomial-sets
    pub fn factor(mut a: Vec<f64>, n: usize) -> Result<Self, RefCellError> {
        assert_eq!(
            n.checked_mul(n),
            Some(a.len()),
            "DenseLu::factor: expected an {n} x {n} matrix, got {} entries",
            a.len()
        );
        let scale = a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let tiny = n as f64 * f64::EPSILON * scale;
        let mut perm: Vec<usize> = (0..n).collect();
        for k in 0..n {
            // The pivot: the largest entry of column k on or below the diagonal.
            let (p, max) = (k..n)
                .map(|i| (i, a[i * n + k].abs()))
                .fold((k, -1.0), |best, c| if c.1 > best.1 { c } else { best });
            if max <= tiny || max == 0.0 {
                return Err(RefCellError::Singular);
            }
            if p != k {
                for j in 0..n {
                    a.swap(k * n + j, p * n + j);
                }
                perm.swap(k, p);
            }
            let pivot = a[k * n + k];
            for i in k + 1..n {
                let l = a[i * n + k] / pivot;
                a[i * n + k] = l;
                if l != 0.0 {
                    for j in k + 1..n {
                        a[i * n + j] -= l * a[k * n + j];
                    }
                }
            }
        }
        Ok(Self { n, lu: a, perm })
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// Solves A X = B in place for the row-major n × `nrhs` right-hand sides `b`.
    ///
    /// On return, `b` holds X in the same row-major layout (`b[i * nrhs + j]` is row i of
    /// right-hand side j). Passing the n × n identity matrix with `nrhs == n` computes A⁻¹.
    /// See [Orthonormal polynomial sets].
    ///
    /// # Panics
    ///
    /// If `b.len() != n * nrhs` (or `n * nrhs` overflows `usize`).
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_refcell::DenseLu;
    ///
    /// // Invert [[2, 1], [1, 3]] by solving against the 2 × 2 identity.
    /// let lu = DenseLu::factor(vec![2.0, 1.0, 1.0, 3.0], 2).unwrap();
    /// let mut inv = [1.0, 0.0, 0.0, 1.0];
    /// lu.solve(&mut inv, 2);
    /// for (got, want) in inv.iter().zip([0.6, -0.2, -0.2, 0.4]) {
    ///     assert!((got - want).abs() < 1e-15);
    /// }
    /// ```
    ///
    /// [Orthonormal polynomial sets]: https://tetracl.github.io/tetraloma/concepts/quadrature.html#orthonormal-polynomial-sets
    pub fn solve(&self, b: &mut [f64], nrhs: usize) {
        let n = self.n;
        assert_eq!(
            n.checked_mul(nrhs),
            Some(b.len()),
            "DenseLu::solve: expected a {n} x {nrhs} right-hand side, got {} entries",
            b.len()
        );
        if n == 0 || nrhs == 0 {
            return;
        }
        // Permute: row k of P B.
        let mut x = Vec::with_capacity(b.len());
        for &r in &self.perm {
            x.extend_from_slice(&b[r * nrhs..(r + 1) * nrhs]);
        }
        // Forward substitution with the unit lower triangle.
        for i in 0..n {
            for k in 0..i {
                let l = self.lu[i * n + k];
                if l != 0.0 {
                    for j in 0..nrhs {
                        x[i * nrhs + j] -= l * x[k * nrhs + j];
                    }
                }
            }
        }
        // Back substitution with the upper triangle.
        for i in (0..n).rev() {
            for k in i + 1..n {
                let u = self.lu[i * n + k];
                if u != 0.0 {
                    for j in 0..nrhs {
                        x[i * nrhs + j] -= u * x[k * nrhs + j];
                    }
                }
            }
            let d = self.lu[i * n + i];
            for j in 0..nrhs {
                x[i * nrhs + j] /= d;
            }
        }
        b.copy_from_slice(&x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn matvec(a: &[f64], x: &[f64]) -> Vec<f64> {
        let n = x.len();
        (0..n)
            .map(|i| (0..n).map(|j| a[i * n + j] * x[j]).sum())
            .collect()
    }

    #[test]
    fn solves_a_known_system() {
        // A x = b with x = (1, 2, 3).
        let a = vec![4.0, -2.0, 1.0, -2.0, 4.0, -2.0, 1.0, -2.0, 4.0];
        let mut b = matvec(&a, &[1.0, 2.0, 3.0]);
        DenseLu::factor(a, 3).unwrap().solve(&mut b, 1);
        for (got, want) in b.iter().zip([1.0, 2.0, 3.0]) {
            assert!((got - want).abs() < 1e-15, "{b:?}");
        }
    }

    #[test]
    fn pivots_on_a_zero_leading_entry() {
        let a = vec![0.0, 1.0, 2.0, 1.0, 0.0, 3.0, 4.0, -3.0, 8.0];
        let x = [-1.0, 0.5, 2.0];
        let mut b = matvec(&a, &x);
        DenseLu::factor(a, 3).unwrap().solve(&mut b, 1);
        for (got, want) in b.iter().zip(x) {
            assert!((got - want).abs() < 1e-14, "{b:?}");
        }
    }

    #[test]
    fn several_right_hand_sides_give_the_inverse() {
        let a = vec![2.0, 1.0, 1.0, 3.0];
        let mut b = vec![1.0, 0.0, 0.0, 1.0];
        DenseLu::factor(a, 2).unwrap().solve(&mut b, 2);
        let want = [0.6, -0.2, -0.2, 0.4];
        for (got, w) in b.iter().zip(want) {
            assert!((got - w).abs() < 1e-15, "{b:?}");
        }
    }

    #[test]
    fn empty_system_and_zero_rhs() {
        let empty = DenseLu::factor(vec![], 0).unwrap();
        let mut b: [f64; 0] = [];
        empty.solve(&mut b, 0);
        empty.solve(&mut b, 3);

        let lu = DenseLu::factor(vec![2.0, 0.0, 0.0, 3.0], 2).unwrap();
        lu.solve(&mut b, 0);
    }

    #[test]
    fn rank_deficient_is_singular() {
        // The third row is the sum of the first two.
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 5.0, 7.0, 9.0];
        assert_eq!(DenseLu::factor(a, 3).unwrap_err(), RefCellError::Singular);
        assert_eq!(
            DenseLu::factor(vec![0.0; 4], 2).unwrap_err(),
            RefCellError::Singular
        );
    }

    proptest! {
        #[test]
        fn solves_diagonally_dominant_multi_rhs(
            n in 1_usize..=6,
            nrhs in 1_usize..=4,
            raw_a in proptest::collection::vec(-5.0_f64..5.0, 36),
            raw_b in proptest::collection::vec(-5.0_f64..5.0, 24),
        ) {
            // Make A strictly diagonally dominant, then cyclically shift rows so pivoting runs.
            let mut a = vec![0.0; n * n];
            for i in 0..n {
                let row = (i + 1) % n;
                let mut off = 0.0;
                for j in 0..n {
                    let v = raw_a[i * 6 + j];
                    a[row * n + j] = v;
                    if j != i {
                        off += v.abs();
                    }
                }
                a[row * n + i] = off + 2.0;
            }
            let b: Vec<f64> = raw_b[..n * nrhs].to_vec();
            let mut x = b.clone();
            let lu = DenseLu::factor(a.clone(), n).unwrap();
            lu.solve(&mut x, nrhs);

            let scale = a.iter().fold(1.0_f64, |m, v| m.max(v.abs()))
                * x.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
            for i in 0..n {
                for j in 0..nrhs {
                    let ax: f64 = (0..n).map(|k| a[i * n + k] * x[k * nrhs + j]).sum();
                    prop_assert!(
                        (ax - b[i * nrhs + j]).abs() < 1e-11 * scale,
                        "n = {n}, nrhs = {nrhs}, ({i}, {j}): {ax} vs {}",
                        b[i * nrhs + j]
                    );
                }
            }
        }
    }
}

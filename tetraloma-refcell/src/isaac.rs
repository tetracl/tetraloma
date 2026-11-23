//! Isaac's recursive Gauss–Lobatto–Legendre points on simplices (docs/design.md, section 6, A.5).
//!
//! T. Isaac, "Recursive, parameter-free, explicitly defined interpolation nodes for simplices",
//! SIAM J. Sci. Comput. 42(6), 2020. For each multi-index α = (α₀, …, α_d) with Σ α_j = n,
//! the barycentric coordinates b_d(α) are a convex combination of the (d − 1)-simplex coordinates
//! b_{d−1}(α_{\setminus j}) weighted by the 1D GLL coordinate x_{n, n − α_j} = 1 − x_{n, α_j}.
//! Because the 1D GLL points are symmetric and include the endpoints 0 and 1, the nodes are
//! symmetric under vertex permutations and restrict on every sub-entity to that sub-entity's
//! own GLL points.

use crate::jacobi::gll_points;

/// Precomputed 1D GLL tables up to a maximum degree, for evaluating Isaac's recursive simplex
/// points without recomputing the 1D nodes.
pub(crate) struct IsaacGll {
    /// `gll[k]` holds the `k + 1` GLL points on `[0, 1]` for `k = 1..=max_degree`, and `[0.5]`
    /// for `k = 0`.
    gll: Vec<Vec<f64>>,
}

impl IsaacGll {
    /// Precomputes the 1D GLL tables for degrees `0..=max_degree`.
    pub(crate) fn new(max_degree: usize) -> Self {
        let mut gll = Vec::with_capacity(max_degree + 1);
        gll.push(vec![0.5]);
        for k in 1..=max_degree {
            gll.push(gll_points(k + 1));
        }
        Self { gll }
    }

    /// The `degree + 1` 1D GLL points on `[0, 1]`, ascending.
    pub(crate) fn line(&self, degree: usize) -> &[f64] {
        &self.gll[degree]
    }

    /// The reference triangle point `(ξ₀, ξ₁)` for multi-index `(degree − i0 − i1, i0, i1)`.
    pub(crate) fn triangle(&self, i0: usize, i1: usize, degree: usize) -> [f64; 2] {
        debug_assert!(i0 + i1 <= degree);
        let b = self.barycentric_2([degree - i0 - i1, i0, i1]);
        [b[1], b[2]]
    }

    /// The reference tetrahedron point `(ξ₀, ξ₁, ξ₂)` for multi-index
    /// `(degree − i0 − i1 − i2, i0, i1, i2)`.
    pub(crate) fn tetrahedron(&self, i0: usize, i1: usize, i2: usize, degree: usize) -> [f64; 3] {
        debug_assert!(i0 + i1 + i2 <= degree);
        let b = self.barycentric_3([degree - i0 - i1 - i2, i0, i1, i2]);
        [b[1], b[2], b[3]]
    }

    fn barycentric_1(&self, a: [usize; 2]) -> [f64; 2] {
        let n = a[0] + a[1];
        if n == 0 {
            return [0.5, 0.5];
        }
        [self.gll[n][a[0]], self.gll[n][a[1]]]
    }

    fn barycentric_2(&self, a: [usize; 3]) -> [f64; 3] {
        let n = a[0] + a[1] + a[2];
        if n == 0 {
            return [1.0 / 3.0; 3];
        }
        let mut b = [0.0; 3];
        let mut wsum = 0.0;
        for j in 0..3 {
            let w = self.gll[n][n - a[j]];
            if w == 0.0 {
                continue;
            }
            let sub = match j {
                0 => self.barycentric_1([a[1], a[2]]),
                1 => self.barycentric_1([a[0], a[2]]),
                _ => self.barycentric_1([a[0], a[1]]),
            };
            let mut s = 0;
            for (k, bk) in b.iter_mut().enumerate() {
                if k != j {
                    *bk += w * sub[s];
                    s += 1;
                }
            }
            wsum += w;
        }
        [b[0] / wsum, b[1] / wsum, b[2] / wsum]
    }

    fn barycentric_3(&self, a: [usize; 4]) -> [f64; 4] {
        let n = a[0] + a[1] + a[2] + a[3];
        if n == 0 {
            return [0.25; 4];
        }
        let mut b = [0.0; 4];
        let mut wsum = 0.0;
        for j in 0..4 {
            let w = self.gll[n][n - a[j]];
            if w == 0.0 {
                continue;
            }
            let sub = match j {
                0 => self.barycentric_2([a[1], a[2], a[3]]),
                1 => self.barycentric_2([a[0], a[2], a[3]]),
                2 => self.barycentric_2([a[0], a[1], a[3]]),
                _ => self.barycentric_2([a[0], a[1], a[2]]),
            };
            let mut s = 0;
            for (k, bk) in b.iter_mut().enumerate() {
                if k != j {
                    *bk += w * sub[s];
                    s += 1;
                }
            }
            wsum += w;
        }
        [b[0] / wsum, b[1] / wsum, b[2] / wsum, b[3] / wsum]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barycentric_coordinates_sum_to_one_and_are_symmetric() {
        let table = IsaacGll::new(8);
        // Triangle: every permutation of (a0, a1, a2) permutes the barycentric coordinates.
        let tri_perms = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for p in 1..=8 {
            for i1 in 0..=p {
                for i0 in 0..=p - i1 {
                    let a = [p - i0 - i1, i0, i1];
                    let b = table.barycentric_2(a);
                    let sum: f64 = b.iter().sum();
                    assert!((sum - 1.0).abs() <= 1e-15, "tri p={p} a={a:?}: sum {sum}");
                    assert!(b.iter().all(|&x| (0.0..=1.0).contains(&x)));
                    for perm in tri_perms {
                        let pa = [a[perm[0]], a[perm[1]], a[perm[2]]];
                        let pb = table.barycentric_2(pa);
                        for k in 0..3 {
                            assert!(
                                (pb[k] - b[perm[k]]).abs() <= 1e-15,
                                "tri symmetry p={p} a={a:?} perm={perm:?}"
                            );
                        }
                    }
                }
            }
        }
        // Tetrahedron: cyclic and swap symmetries cover S_4.
        let tet_perms = [[1, 0, 2, 3], [0, 2, 1, 3], [0, 1, 3, 2], [3, 0, 1, 2]];
        for p in 1..=6 {
            for i2 in 0..=p {
                for i1 in 0..=p - i2 {
                    for i0 in 0..=p - i2 - i1 {
                        let a = [p - i0 - i1 - i2, i0, i1, i2];
                        let b = table.barycentric_3(a);
                        let sum: f64 = b.iter().sum();
                        assert!((sum - 1.0).abs() <= 1e-15, "tet p={p} a={a:?}: sum {sum}");
                        assert!(b.iter().all(|&x| (0.0..=1.0).contains(&x)));
                        for perm in tet_perms {
                            let pa = [a[perm[0]], a[perm[1]], a[perm[2]], a[perm[3]]];
                            let pb = table.barycentric_3(pa);
                            for k in 0..4 {
                                assert!(
                                    (pb[k] - b[perm[k]]).abs() <= 1e-15,
                                    "tet symmetry p={p} a={a:?} perm={perm:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn boundary_and_centroid_values() {
        let table = IsaacGll::new(6);
        // Triangle centroid at p = 3: (1/3, 1/3).
        let c3 = table.triangle(1, 1, 3);
        assert!((c3[0] - 1.0 / 3.0).abs() <= 1e-15);
        assert!((c3[1] - 1.0 / 3.0).abs() <= 1e-15);
        // Tetrahedron centroid at p = 4: (1/4, 1/4, 1/4).
        let c4 = table.tetrahedron(1, 1, 1, 4);
        for x in c4 {
            assert!((x - 0.25).abs() <= 1e-15);
        }
        // Edge restriction: a = (p - i, i, 0) equals 1D GLL.
        for p in 1..=6 {
            for i in 0..=p {
                let b = table.barycentric_2([p - i, i, 0]);
                assert!((b[1] - table.line(p)[i]).abs() <= 1e-15);
                assert_eq!(b[2], 0.0);
            }
        }
        // Face restriction on tetrahedron: a = (p - i0 - i1, i0, i1, 0) equals triangle Isaac.
        for p in 1..=6 {
            for i1 in 0..=p {
                for i0 in 0..=p - i1 {
                    let b3 = table.barycentric_3([p - i0 - i1, i0, i1, 0]);
                    let b2 = table.barycentric_2([p - i0 - i1, i0, i1]);
                    assert_eq!(b3[3], 0.0);
                    for k in 0..3 {
                        assert!((b3[k] - b2[k]).abs() <= 1e-15);
                    }
                }
            }
        }
    }
}

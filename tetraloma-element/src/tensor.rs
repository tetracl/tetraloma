// Design: section 26 ("Finite elements", "Tensor factors"), Listing E1.
// Stubs for M11.T0; implemented in M11.T7.

use tetraloma_refcell::NodeFamily;

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Factor1d {
    NodalH1(NodeFamily),
    MomentH1,
    LegendreL2,
}

#[doc(hidden)]
pub struct TensorFactors {
    pub factors: Vec<Vec<(Factor1d, u32)>>,
    pub lexicographic: Vec<Vec<usize>>,
}

impl TensorFactors {
    #[doc(hidden)]
    pub fn tabulate_1d(
        &self,
        component: usize,
        direction: usize,
        points: &[f64],
        nderivs: u32,
    ) -> Vec<f64> {
        let _ = (component, direction, points, nderivs);
        unimplemented!("pending M11.T7")
    }
}

#[cfg(test)]
mod tests {
    use tetraloma_dims::CellType;
    use tetraloma_refcell::{NodeFamily, tdim};

    use crate::{Element, ElementKey, ElementTabulation, Family, Variant};

    const TOL: f64 = 1e-12;

    /// Reference dense construction for tensor cells, wired in M11.T7 (section 26: "used there
    /// only by tests, for p <= 8").
    fn dense_tabulate(key: ElementKey, points: &[f64], nderivs: u32) -> ElementTabulation {
        let _ = (key, points, nderivs);
        unimplemented!("pending M11.T7")
    }

    fn tensor_grid(coords: &[f64], td: usize) -> Vec<f64> {
        let m = coords.len();
        let mut pts = Vec::with_capacity(m.pow(td as u32) * td);
        for idx in 0..m.pow(td as u32) {
            let mut rem = idx;
            for _ in 0..td {
                pts.push(coords[rem % m]);
                rem /= m;
            }
        }
        pts
    }

    /// M11 acceptance A11: on quadrilaterals and hexahedra, the product of the tensor factors
    /// reproduces the dense construction's values and first derivatives to 1e-12 for p <= 8.
    #[test]
    #[ignore = "pending M11.T7"]
    fn factors_reproduce_dense_construction() {
        let coords_1d = [0.0, 0.19, 0.63, 1.0];
        let variants = [
            Variant::Nodal(NodeFamily::Equispaced),
            Variant::Nodal(NodeFamily::Gll),
            Variant::Legendre,
        ];

        for cell in [CellType::Quadrilateral, CellType::Hexahedron] {
            let td = tdim(cell);
            let points = tensor_grid(&coords_1d, td);
            let npts = points.len() / td;
            let per_dir: Vec<Vec<f64>> = (0..td)
                .map(|d| (0..npts).map(|q| points[q * td + d]).collect())
                .collect();

            for variant in variants {
                for degree in 0..=8 {
                    let key = ElementKey {
                        family: Family::Lagrange,
                        cell,
                        degree,
                        variant,
                    };
                    let el = Element::get(key).unwrap();
                    let tf = el.tensor_factors().expect("tensor cell has tensor factors");
                    assert_eq!((tf.factors.len(), tf.factors[0].len()), (1, td));

                    let t1d: Vec<Vec<f64>> = (0..td)
                        .map(|d| tf.tabulate_1d(0, d, &per_dir[d], 1))
                        .collect();
                    let n_dir: Vec<usize> = t1d.iter().map(|t| t.len() / (2 * npts)).collect();
                    let dim = el.dim();
                    assert_eq!(n_dir.iter().product::<usize>(), dim);
                    assert_eq!(tf.lexicographic[0].len(), dim);

                    let dense = dense_tabulate(key, &points, 1);
                    assert_eq!(
                        (dense.nderivs, dense.num_points, dense.dim, dense.value_size),
                        (1, npts, dim, 1)
                    );

                    let mut got = vec![0.0; (1 + td) * npts * dim];
                    for q in 0..npts {
                        for lex in 0..dim {
                            let dof = tf.lexicographic[0][lex];
                            let mut rem = lex;
                            let mut a = [0usize; 3];
                            for d in 0..td {
                                a[d] = rem % n_dir[d];
                                rem /= n_dir[d];
                            }
                            let val_at = |d: usize, deriv: usize| {
                                t1d[d][(deriv * npts + q) * n_dir[d] + a[d]]
                            };
                            got[q * dim + dof] = (0..td).map(|d| val_at(d, 0)).product();
                            for deriv_dir in 0..td {
                                let prod: f64 = (0..td)
                                    .map(|d| val_at(d, usize::from(d == deriv_dir)))
                                    .product();
                                got[((1 + deriv_dir) * npts + q) * dim + dof] = prod;
                            }
                        }
                    }

                    let block = npts * dim;
                    for d in 0..=td {
                        let want = &dense.values[d * block..(d + 1) * block];
                        let actual = &got[d * block..(d + 1) * block];
                        let scale = want.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                        for (idx, (&g, &w)) in actual.iter().zip(want).enumerate() {
                            let err = (g - w).abs() / scale;
                            assert!(
                                err <= TOL,
                                "{key:?} deriv {d} entry {idx}: {g} vs {w} (rel err {err:e})"
                            );
                        }
                    }
                }
            }
        }
    }
}

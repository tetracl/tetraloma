//! Public orthonormal polynomial sets (`polyset`) and dense LU (`DenseLu`) tests.

use tetraloma_dims::CellType;
use tetraloma_refcell::{DenseLu, RefCellError, gauss_rule, num_lagrange_nodes, polyset, tdim};

const ALL_CELLS: [CellType; 8] = [
    CellType::Point,
    CellType::Interval,
    CellType::Triangle,
    CellType::Quadrilateral,
    CellType::Tetrahedron,
    CellType::Hexahedron,
    CellType::Prism,
    CellType::Pyramid,
];

const NON_POINT_CELLS: [CellType; 7] = [
    CellType::Interval,
    CellType::Triangle,
    CellType::Quadrilateral,
    CellType::Tetrahedron,
    CellType::Hexahedron,
    CellType::Prism,
    CellType::Pyramid,
];

#[test]
fn polyset_dim_matches_lagrange_node_counts() {
    for cell in ALL_CELLS {
        for degree in 0..=8 {
            assert_eq!(
                polyset::dim(cell, degree),
                num_lagrange_nodes(cell, degree),
                "{cell:?} p = {degree}"
            );
        }
    }
}

#[test]
fn polyset_is_orthonormal_under_gauss_rule() {
    for cell in NON_POINT_CELLS {
        for degree in 0..=6 {
            let rule = gauss_rule(cell, 2 * degree);
            let nb = polyset::dim(cell, degree);
            let npts = rule.weights.len();
            let table = polyset::tabulate(cell, degree, &rule.points, 0);
            assert_eq!(table.len(), nb * npts, "{cell:?} p = {degree}");

            for k in 0..nb {
                let row_k = &table[k * npts..(k + 1) * npts];
                for l in k..nb {
                    let row_l = &table[l * npts..(l + 1) * npts];
                    let integral: f64 = row_k
                        .iter()
                        .zip(row_l)
                        .zip(&rule.weights)
                        .map(|((a, b), w)| w * a * b)
                        .sum();
                    let want = if k == l { 1.0 } else { 0.0 };
                    assert!(
                        (integral - want).abs() < 1e-12,
                        "{cell:?} p = {degree}: <{k}, {l}> = {integral}"
                    );
                }
            }
        }
    }
}

#[test]
fn polyset_handles_point_cell_and_empty_points() {
    for nderivs in [0, 1] {
        assert_eq!(polyset::tabulate(CellType::Point, 0, &[], nderivs), [1.0]);
        assert_eq!(polyset::tabulate(CellType::Point, 3, &[], nderivs), [1.0]);
        for cell in NON_POINT_CELLS {
            assert!(
                polyset::tabulate(cell, 2, &[], nderivs).is_empty(),
                "{cell:?} nderivs = {nderivs}"
            );
        }
    }
}

#[test]
fn polyset_first_derivatives_have_expected_layout() {
    // On [0, 1], φ_0(t) = 1, φ_1(t) = √3 (2t − 1), so φ_0' = 0, φ_1' = 2√3.
    let pts = [0.0, 0.25, 1.0];
    let table = polyset::tabulate(CellType::Interval, 1, &pts, 1);
    let nb = polyset::dim(CellType::Interval, 1);
    let npts = pts.len();
    assert_eq!(nb, 2);
    assert_eq!(table.len(), (1 + tdim(CellType::Interval)) * nb * npts);

    let s3 = 3.0_f64.sqrt();
    for (q, &t) in pts.iter().enumerate() {
        // [deriv][basis][point]
        assert!((table[q] - 1.0).abs() < 1e-15);
        assert!((table[npts + q] - s3 * (2.0 * t - 1.0)).abs() < 1e-15);
        assert!(table[2 * npts + q].abs() < 1e-15);
        assert!((table[3 * npts + q] - 2.0 * s3).abs() < 1e-14);
    }
}

#[test]
#[should_panic(expected = "derivative order 2")]
fn polyset_panics_above_first_derivative() {
    let _ = polyset::tabulate(CellType::Triangle, 1, &[0.2, 0.3], 2);
}

#[test]
#[should_panic(expected = "point array")]
fn polyset_panics_on_bad_point_length() {
    let _ = polyset::tabulate(CellType::Triangle, 1, &[0.2, 0.3, 0.4], 0);
}

#[test]
#[should_panic(expected = "point array")]
fn polyset_point_cell_panics_on_non_empty_points() {
    let _ = polyset::tabulate(CellType::Point, 0, &[0.0], 0);
}

#[test]
fn dense_lu_solves_multiple_right_hand_sides_and_reports_singular() {
    // 3 x 3 matrix requiring pivoting, solved for 2 right-hand sides simultaneously.
    let a = vec![0.0, 2.0, 1.0, 1.0, -2.0, -3.0, -1.0, 1.0, 2.0];
    let want = [
        // row-major 3 x 2: columns are [-1, 2, 0] and [3, -1, 4]
        -1.0, 3.0, 2.0, -1.0, 0.0, 4.0,
    ];
    let mut b = vec![0.0; 6];
    for i in 0..3 {
        for j in 0..2 {
            b[i * 2 + j] = (0..3).map(|k| a[i * 3 + k] * want[k * 2 + j]).sum();
        }
    }
    let lu = DenseLu::factor(a, 3).unwrap();
    lu.solve(&mut b, 2);
    for (got, expected) in b.iter().zip(want) {
        assert!((got - expected).abs() < 1e-14, "got {b:?}, want {want:?}");
    }

    let singular = vec![1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 1.0, 1.0];
    assert_eq!(
        DenseLu::factor(singular, 3).unwrap_err(),
        RefCellError::Singular
    );
}

#[test]
#[should_panic(expected = "DenseLu::factor")]
fn dense_lu_factor_panics_on_wrong_length() {
    let _ = DenseLu::factor(vec![1.0, 2.0, 3.0], 2);
}

#[test]
#[should_panic(expected = "DenseLu::solve")]
fn dense_lu_solve_panics_on_wrong_length() {
    let lu = DenseLu::factor(vec![1.0, 0.0, 0.0, 1.0], 2).unwrap();
    let mut b = vec![1.0, 2.0, 3.0];
    lu.solve(&mut b, 2);
}

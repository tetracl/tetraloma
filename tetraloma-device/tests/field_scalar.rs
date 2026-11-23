//! Acceptance tests for `FieldScalar` (M18 A9).

use num_complex::Complex;
use tetraloma_device::{FieldScalar, Real};

fn assert_field_scalar<T: FieldScalar<Real = R>, R: Real>(want_parts: usize) {
    assert_eq!(T::PARTS, want_parts);
}

/// M18 A9: `tetraloma-device` exports `FieldScalar` for `f32`, `f64`, `Complex<f32>` and
/// `Complex<f64>`, with `PARTS` equal to 1 for real scalars and 2 for complex scalars.
#[test]
#[ignore = "pending M18.T8"]
fn parts_are_one_and_two() {
    assert_field_scalar::<f32, f32>(1);
    assert_field_scalar::<f64, f64>(1);
    assert_field_scalar::<Complex<f32>, f32>(2);
    assert_field_scalar::<Complex<f64>, f64>(2);
}

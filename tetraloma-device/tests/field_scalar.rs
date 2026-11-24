//! Acceptance and unit tests for `FieldScalar` (M18 A9).

use num_complex::Complex;
use proptest::prelude::*;
use tetraloma_device::{FieldScalar, Real};

fn assert_field_scalar<T: FieldScalar<Real = R>, R: Real>(want_parts: usize) {
    assert_eq!(T::PARTS, want_parts);
}

fn assert_layout_and_bounds<T: FieldScalar<Real = R>, R: Real>(want_parts: usize) {
    fn check_bounds<U: Copy + Default + bytemuck::Pod + Send + Sync + 'static>() {}
    check_bounds::<T>();
    assert_eq!(T::PARTS, want_parts);
    assert_eq!(size_of::<T>(), T::PARTS * size_of::<R>());
    assert_eq!(align_of::<T>(), align_of::<R>());
    assert!(
        bytemuck::bytes_of(&T::default()).iter().all(|&b| b == 0),
        "default value must be all-zero bytes"
    );
}

/// M18 A9: `tetraloma-device` exports `FieldScalar` for `f32`, `f64`, `Complex<f32>` and
/// `Complex<f64>`, with `PARTS` equal to 1 for real scalars and 2 for complex scalars.
#[test]
fn parts_are_one_and_two() {
    assert_field_scalar::<f32, f32>(1);
    assert_field_scalar::<f64, f64>(1);
    assert_field_scalar::<Complex<f32>, f32>(2);
    assert_field_scalar::<Complex<f64>, f64>(2);
}

/// Layout, alignment, zero-default and supertrait bounds hold for all four built-in scalars.
#[test]
fn layout_and_supertraits() {
    assert_layout_and_bounds::<f32, f32>(1);
    assert_layout_and_bounds::<f64, f64>(1);
    assert_layout_and_bounds::<Complex<f32>, f32>(2);
    assert_layout_and_bounds::<Complex<f64>, f64>(2);
}

/// `FieldScalar` is intentionally not sealed (Listing F2): external `Pod` types can implement it.
#[test]
fn external_type_can_implement_field_scalar() {
    #[repr(transparent)]
    #[derive(Clone, Copy, Default)]
    struct CustomReal(f32);

    // SAFETY: `CustomReal` is `#[repr(transparent)]` over `f32`, which is `Zeroable` and `Pod`,
    // has no padding bytes, and every bit pattern is valid.
    unsafe impl bytemuck::Zeroable for CustomReal {}
    // SAFETY: `CustomReal` is `#[repr(transparent)]` over `f32`, which is `Pod`.
    unsafe impl bytemuck::Pod for CustomReal {}

    impl FieldScalar for CustomReal {
        type Real = f32;
        const PARTS: usize = 1;
    }

    assert_layout_and_bounds::<CustomReal, f32>(1);
    let xs = [CustomReal(1.5), CustomReal(-2.25)];
    let raw: &[f32] = bytemuck::cast_slice(&xs);
    assert_eq!(raw, &[1.5, -2.25]);
}

proptest! {
    /// `bytemuck::cast_slice` reinterprets `Complex<f32>` as interleaved `[re, im]` pairs with
    /// bit-exact fidelity.
    #[test]
    fn complex_f32_cast_matches_interleaved_parts(
        pairs in prop::collection::vec((any::<f32>(), any::<f32>()), 0..64)
    ) {
        let zs: Vec<Complex<f32>> = pairs.iter().map(|&(r, i)| Complex::new(r, i)).collect();
        let parts: &[f32] = bytemuck::cast_slice(&zs);
        prop_assert_eq!(parts.len(), zs.len() * <Complex<f32> as FieldScalar>::PARTS);
        for (k, &(r, i)) in pairs.iter().enumerate() {
            prop_assert_eq!(parts[2 * k].to_bits(), r.to_bits());
            prop_assert_eq!(parts[2 * k + 1].to_bits(), i.to_bits());
        }
        let roundtrip: &[Complex<f32>] = bytemuck::cast_slice(parts);
        for (a, b) in roundtrip.iter().zip(&zs) {
            prop_assert_eq!(a.re.to_bits(), b.re.to_bits());
            prop_assert_eq!(a.im.to_bits(), b.im.to_bits());
        }
    }

    /// `bytemuck::cast_slice` reinterprets `Complex<f64>` as interleaved `[re, im]` pairs with
    /// bit-exact fidelity.
    #[test]
    fn complex_f64_cast_matches_interleaved_parts(
        pairs in prop::collection::vec((any::<f64>(), any::<f64>()), 0..64)
    ) {
        let zs: Vec<Complex<f64>> = pairs.iter().map(|&(r, i)| Complex::new(r, i)).collect();
        let parts: &[f64] = bytemuck::cast_slice(&zs);
        prop_assert_eq!(parts.len(), zs.len() * <Complex<f64> as FieldScalar>::PARTS);
        for (k, &(r, i)) in pairs.iter().enumerate() {
            prop_assert_eq!(parts[2 * k].to_bits(), r.to_bits());
            prop_assert_eq!(parts[2 * k + 1].to_bits(), i.to_bits());
        }
        let roundtrip: &[Complex<f64>] = bytemuck::cast_slice(parts);
        for (a, b) in roundtrip.iter().zip(&zs) {
            prop_assert_eq!(a.re.to_bits(), b.re.to_bits());
            prop_assert_eq!(a.im.to_bits(), b.im.to_bits());
        }
    }
}

//! `Real` (Listing B3), `FieldScalar` (Listing F2) and `check_precision` (Listing S1).

use cubecl::prelude::*;

use crate::device_error::{DeviceError, Result};
use crate::testing;

// Listing B3.
/// The scalar type of a mesh: `f32` or `f64`.
///
/// Write code that works in both precisions generic over `F: Real`. On the host, compute with
/// `f64`: [`widen`](Real::widen) converts a value to `f64` exactly, and
/// [`narrow`](Real::narrow) rounds an `f64` back to `F`. Do not call arithmetic methods such as
/// `sqrt`, `abs` or `min` on `F` in host code. `F` gets them from CubeCL's `Float`, whose
/// versions are meant for kernels and panic on the host.
/// [Precision](https://tetracl.github.io/tetraloma/concepts/precision.html#f32-and-f64)
/// explains what changes between the two types.
///
/// # Example
///
/// ```
/// use tetraloma_device::Real;
///
/// fn mean<F: Real>(xs: &[F]) -> F {
///     F::narrow(xs.iter().map(|x| x.widen()).sum::<f64>() / xs.len() as f64)
/// }
/// assert_eq!(mean(&[1.0f32, 2.0, 6.0]), 3.0);
/// assert_eq!(0.1f32.widen(), 0.1f32 as f64);
/// ```
pub trait Real: Float + CubeElement {
    /// The value as `f64`, exactly: every `f32` is an `f64` too. See
    /// [Precision](https://tetracl.github.io/tetraloma/concepts/precision.html#f32-and-f64).
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_device::Real;
    ///
    /// assert_eq!(0.1f32.widen(), f64::from(0.1f32));
    /// assert_eq!(0.1f64.widen(), 0.1);
    /// ```
    fn widen(self) -> f64;
    /// `x` rounded to the nearest value of `Self`: unchanged for `f64`, rounded for `f32`. See
    /// [Precision](https://tetracl.github.io/tetraloma/concepts/precision.html#f32-and-f64).
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_device::Real;
    ///
    /// assert_eq!(f32::narrow(0.1), 0.1f32);
    /// assert_eq!(f64::narrow(0.1), 0.1);
    /// // A round trip through f64 is exact.
    /// assert_eq!(f32::narrow(1.5f32.widen()), 1.5);
    /// ```
    fn narrow(x: f64) -> Self;
}

impl Real for f32 {
    fn widen(self) -> f64 {
        f64::from(self)
    }
    fn narrow(x: f64) -> Self {
        x as f32
    }
}

impl Real for f64 {
    fn widen(self) -> f64 {
        self
    }
    fn narrow(x: f64) -> Self {
        x
    }
}

// Design: "Grid functions and their evaluation", Listing F2; ADR-0074.
/// Real or complex coefficient scalar: `f32`, `f64`, `num_complex::Complex<f32>` or
/// `num_complex::Complex<f64>`.
///
/// A mesh stores geometry in a real scalar `F: Real`, while a function's coefficients may be real
/// or complex in the same underlying precision [`FieldScalar::Real`]. For these four types,
/// [`FieldScalar::PARTS`] is `1` for real scalars and `2` for complex scalars, and a slice `&[T]`
/// reinterprets directly as `PARTS` consecutive values of `T::Real` per entry (`[re, im]` for
/// complex values) via `bytemuck::Pod`. See [Coefficient
/// scalars](https://tetracl.github.io/tetraloma/concepts/precision.html#coefficient-scalars).
///
/// # Example
///
/// ```
/// use num_complex::Complex;
/// use tetraloma_device::FieldScalar;
///
/// fn real_len<T: FieldScalar>(num_coeffs: usize) -> usize {
///     num_coeffs * T::PARTS
/// }
///
/// assert_eq!(real_len::<f32>(4), 4);
/// assert_eq!(real_len::<f64>(4), 4);
/// assert_eq!(real_len::<Complex<f32>>(4), 8);
/// assert_eq!(real_len::<Complex<f64>>(4), 8);
///
/// // Complex values reinterpret directly as interleaved [re, im] real slices.
/// let zs = [Complex::new(1.0f64, -2.0), Complex::new(3.5, 4.25)];
/// let parts: &[f64] = bytemuck::cast_slice(&zs);
/// assert_eq!(parts, &[1.0, -2.0, 3.5, 4.25]);
/// ```
pub trait FieldScalar: Copy + Default + bytemuck::Pod + Send + Sync + 'static {
    /// The underlying real scalar type (`f32` or `f64`).
    type Real: Real;
    /// The number of real components per coefficient: `1` for real scalars and `2` for complex
    /// scalars.
    const PARTS: usize;
}

impl FieldScalar for f32 {
    type Real = f32;
    const PARTS: usize = 1;
}

impl FieldScalar for f64 {
    type Real = f64;
    const PARTS: usize = 1;
}

impl FieldScalar for num_complex::Complex<f32> {
    type Real = f32;
    const PARTS: usize = 2;
}

impl FieldScalar for num_complex::Complex<f64> {
    type Real = f64;
    const PARTS: usize = 2;
}

// Listing S1.
/// Checks that `client`'s device supports the scalar type `F`.
///
/// tetraloma calls it whenever a mesh is created, so a mesh of an unsupported type fails before
/// any kernel runs. Call it yourself to choose `F` for a device: Metal has no `f64`, the CPU
/// runtime and CUDA have both. [Compute
/// devices](https://tetracl.github.io/tetraloma/devices.html#f64-support) lists the backends.
///
/// # Errors
///
/// `DeviceError::UnsupportedPrecision`, naming the type and the device, if the device does not
/// support `F`.
///
/// # Example
///
/// ```
/// use tetraloma_device::{DeviceError, check_precision};
///
/// // `client`: a CubeCL client, for example `cubecl::Device::cpu()?.client()`.
/// # let client = tetraloma_testing::test_client();
/// check_precision::<f32>(&client).unwrap();
/// match check_precision::<f64>(&client) {
///     Ok(()) => {}
///     Err(DeviceError::UnsupportedPrecision { scalar, .. }) => assert_eq!(scalar, "f64"),
///     Err(e) => panic!("{e}"),
/// }
/// ```
pub fn check_precision<F: Real>(client: &Client) -> Result<()> {
    if testing::properties(client).supports_type(F::cube_type()) {
        return Ok(());
    }
    Err(DeviceError::UnsupportedPrecision {
        scalar: F::type_name(),
        device: client.name(),
    })
}

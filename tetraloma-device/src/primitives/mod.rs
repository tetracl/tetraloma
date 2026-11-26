// Listing S1b.
//! Parallel algorithms on device buffers, re-exported at the crate root: fill and copy, gather
//! and scatter, pack and unpack, histogram, reductions, prefix sums, a stable radix sort,
//! run-length encoding and grouping of sorted keys.
//!
//! tetraloma builds its meshes with them, and you may use them on buffers of your own. Every
//! primitive takes its launch shape from one [`LaunchPlan`](crate::LaunchPlan), uses no plane
//! operations, no 64-bit atomics and no floating-point atomics, and gives the same bits on every
//! run on a device
//! ([Precision](https://tetracl.github.io/tetraloma/concepts/precision.html#determinism)). An empty
//! input never launches a kernel.

use cubecl::prelude::*;

mod copy;
mod fill;
mod gather;
mod histogram;
mod pack;
mod radix_sort;
mod reduce;
mod scan;
mod segmented;
mod sort_keys;
mod unique;

pub use copy::copy;
pub use fill::fill;
pub(crate) use fill::launch_fill;
pub use gather::{gather, scatter};
pub use histogram::histogram;
pub use pack::{pack, unpack};
pub use radix_sort::{radix_sort, radix_sort_owned};
pub use reduce::reduce;
pub use scan::{exclusive_scan, inclusive_scan};
pub use segmented::segmented_reduce;
pub use sort_keys::SortKeys;
pub use unique::{Runs, Unique, run_length, unique_by_key, unique_by_key_runs};

mod sealed {
    /// Seals `DeviceScalar`. Its supertraits are the launch bounds the kernels need, which
    /// `DeviceScalar` thereby implies without naming them in its public declaration.
    pub trait Sealed: cubecl::frontend::LaunchArg<RuntimeArg = Self> {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
    impl Sealed for i32 {}
    impl Sealed for u32 {}
}

/// The scalar types of the arithmetic primitives: `f32`, `f64`, `i32` and `u32`.
///
/// The trait is sealed: no other type can implement it. [`reduce`], the prefix sums and
/// [`segmented_reduce`] combine values in a fixed order for each of these types
/// ([Precision](https://tetracl.github.io/tetraloma/concepts/precision.html#determinism)).
///
/// # Example
///
/// ```
/// use tetraloma_device::{DeviceBuffer, DeviceScalar, ReduceOp, reduce};
///
/// fn total<T: DeviceScalar>(x: &DeviceBuffer<T>) -> Vec<T> {
///     reduce(x, ReduceOp::Sum).unwrap().to_host().unwrap()
/// }
///
/// // `client`: a CubeCL client, for example `cubecl::Device::cpu()?.client()`.
/// # let client = tetraloma_testing_base::test_client();
/// assert_eq!(total(&DeviceBuffer::from_host(&client, &[1i32, 2, 3]).unwrap()), vec![6]);
/// assert_eq!(total(&DeviceBuffer::from_host(&client, &[1u32, 2, 3]).unwrap()), vec![6]);
/// ```
pub trait DeviceScalar: CubeElement + Numeric + sealed::Sealed {}

impl DeviceScalar for f32 {}
impl DeviceScalar for f64 {}
impl DeviceScalar for i32 {}
impl DeviceScalar for u32 {}

// Design: Listing S1b; ADR-0074.
pub use tetraloma_comm::ReduceOp;

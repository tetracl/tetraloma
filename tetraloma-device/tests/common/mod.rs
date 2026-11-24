//! Shared proptest, vector-size and length helpers of the tetraloma-device acceptance tests.
//!
//! Host references and primitive checks live in `tetraloma_testing::primitive_checks` (M18 T1).
//! Every integration test file includes this module; not every file uses every helper.
#![allow(dead_code, unused_macros, unused_imports)]

use cubecl::prelude::*;
use tetraloma_device::testing::{record_plans, with_vector_size};
use tetraloma_device::{KernelKind, LaunchPlan, VectorSizeOverride};

/// Cases per property test.
pub const PROP_CASES: u32 = 256;
/// Largest length of a property test.
pub const PROP_MAX_LEN: usize = 100_000;
/// The fixed lengths of the large tests.
pub const LARGE_LENS: [usize; 2] = [1_000_000, 10_000_000];
/// The lengths of the vector-size invariance tests.
pub const INVARIANCE_LENS: [usize; 6] = [0, 1, 15, 16, 17, 1_000_003];

/// The proptest configuration of every property test.
pub fn prop_config() -> proptest::test_runner::Config {
    proptest::test_runner::Config {
        cases: PROP_CASES,
        ..proptest::test_runner::Config::default()
    }
}

/// Lengths for property tests: small ones often, up to `PROP_MAX_LEN`.
pub fn prop_len() -> impl proptest::strategy::Strategy<Value = usize> {
    use proptest::prelude::*;
    prop_oneof![0usize..=64, 0usize..=PROP_MAX_LEN]
}

pub(crate) use tetraloma_testing::each_scalar;

/// The vector sizes of the invariance tests: 1 plus every power of two from 2 to 16 no wider
/// than the device's widest IO-optimized size for `elem_bytes`.
pub fn vector_sizes(client: &Client, elem_bytes: usize) -> Vec<usize> {
    let widest = client
        .io_optimized_vector_sizes(elem_bytes)
        .next()
        .unwrap_or(1);
    let mut sizes = vec![1];
    sizes.extend([2, 4, 8, 16].into_iter().filter(|&n| n <= widest));
    sizes
}

/// Runs `f` at vector size `n` and returns its result. Unless `expect_launch` is false (empty
/// inputs never launch), asserts that the first plan of `kind` ran at exactly `n`.
pub fn at_vector_size<R>(
    n: usize,
    kind: KernelKind,
    expect_launch: bool,
    f: impl FnOnce() -> R,
) -> R {
    let (result, plans): (R, Vec<(KernelKind, LaunchPlan)>) =
        record_plans(|| with_vector_size(VectorSizeOverride::Fixed(n), f));
    let first = plans.iter().find(|(k, _)| *k == kind);
    if expect_launch {
        let (_, plan) = first.unwrap_or_else(|| panic!("no {kind:?} plan was made"));
        assert_eq!(
            plan.vector_size, n,
            "the {kind:?} phase ran at vector size {}, not {n}",
            plan.vector_size
        );
    } else {
        assert!(first.is_none(), "an empty input made a {kind:?} plan");
    }
    result
}

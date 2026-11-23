//! Stub for `ParallelError` and its `From<ParallelError> for GridError` conversion
//! (M18.T0; ADR-0074).

use tetraloma_device::{GridError, ServerError};

// M18.T0 stub (ADR-0074, Listing S2c); implemented and unhidden in M18.T5.
/// Errors of index maps and halo exchanges. Not part of the stable API while hidden.
#[doc(hidden)]
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ParallelError {
    /// A size or index does not fit the type that must hold it.
    TooLarge,
    /// One allocation of `bytes` bytes exceeds the device's `max_page_size`.
    AllocationTooLarge {
        /// The size of the requested allocation in bytes.
        bytes: usize,
        /// The device's `max_page_size` in bytes.
        max: u64,
    },
    /// The device does not support the scalar type.
    UnsupportedPrecision {
        /// The name of the scalar type.
        scalar: &'static str,
        /// The name of the device.
        device: &'static str,
    },
    /// Two lengths that must agree differ.
    LengthMismatch {
        /// The required length.
        expected: usize,
        /// The length given.
        found: usize,
    },
    /// The compute server reported an error.
    Server(ServerError),
    /// An index is not below the length it indexes.
    IndexOutOfRange {
        /// The offending index.
        index: usize,
        /// The length it must be below.
        len: usize,
    },
    /// A message count or displacement exceeds `i32::MAX`.
    MessageTooLarge {
        /// The count or displacement, in elements.
        count: usize,
    },
    /// A collective operation failed on another rank.
    RemoteFailure {
        /// The lowest rank on which the operation failed.
        rank: usize,
    },
}

impl From<ParallelError> for GridError {
    fn from(_: ParallelError) -> Self {
        unimplemented!("pending M18.T5")
    }
}

#[cfg(test)]
mod tests {
    use super::ParallelError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `ParallelError`
    // breaks this test and points to
    // `tetraloma::pipeline::dual_graph::tests::foundation_errors_convert_to_their_grid_error_variants`.
    fn variant_index(error: &ParallelError) -> usize {
        match error {
            ParallelError::TooLarge => 0,
            ParallelError::AllocationTooLarge { .. } => 1,
            ParallelError::UnsupportedPrecision { .. } => 2,
            ParallelError::LengthMismatch { .. } => 3,
            ParallelError::Server(_) => 4,
            ParallelError::IndexOutOfRange { .. } => 5,
            ParallelError::MessageTooLarge { .. } => 6,
            ParallelError::RemoteFailure { .. } => 7,
        }
    }

    #[test]
    fn parallel_error_variant_list_is_exhaustive() {
        assert_eq!(variant_index(&ParallelError::TooLarge), 0);
    }
}

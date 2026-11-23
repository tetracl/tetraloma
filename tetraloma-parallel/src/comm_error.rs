//! Stub for `CommError` and its `From<CommError> for GridError` conversion (M18.T0; ADR-0074).
//! Moves to `tetraloma-comm` in M18.T2.

use tetraloma_device::GridError;

// M18.T0 stub (ADR-0074, Listing S2c); moves to tetraloma-comm and is unhidden in M18.T2.
/// Errors of the communication layer. Not part of the stable API while hidden.
#[doc(hidden)]
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CommError {
    /// A size or index does not fit the type that must hold it.
    TooLarge,
    /// Two lengths that must agree differ.
    LengthMismatch {
        /// The required length.
        expected: usize,
        /// The length given.
        found: usize,
    },
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

impl From<CommError> for GridError {
    fn from(_: CommError) -> Self {
        unimplemented!("pending M18.T2")
    }
}

#[cfg(test)]
mod tests {
    use super::CommError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `CommError`
    // breaks this test and points to
    // `tetraloma::pipeline::dual_graph::tests::foundation_errors_convert_to_their_grid_error_variants`.
    fn variant_index(error: &CommError) -> usize {
        match error {
            CommError::TooLarge => 0,
            CommError::LengthMismatch { .. } => 1,
            CommError::IndexOutOfRange { .. } => 2,
            CommError::MessageTooLarge { .. } => 3,
            CommError::RemoteFailure { .. } => 4,
        }
    }

    #[test]
    fn comm_error_variant_list_is_exhaustive() {
        assert_eq!(variant_index(&CommError::TooLarge), 0);
    }
}

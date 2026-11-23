//! Stub for `PartitionError` (M18.T0; ADR-0074).

// M18.T0 stub (ADR-0074, Listing S3); implemented and unhidden in M18.T3.
/// Errors of distributed graph partitioning. Not part of the stable API while hidden.
#[doc(hidden)]
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum PartitionError {
    /// A partitioner library returned a failing status code.
    PartitionerFailed {
        /// The partitioner's name.
        name: &'static str,
        /// The library's status or validation error code.
        code: i32,
    },
    /// A distributed partitioner requires an MPI communicator for more than one part.
    NeedsMpiCommunicator(&'static str),
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

#[cfg(test)]
mod tests {
    use super::PartitionError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `PartitionError`
    // breaks this test and points to
    // `tetraloma::pipeline::dual_graph::tests::foundation_errors_convert_to_their_grid_error_variants`.
    fn variant_index(error: &PartitionError) -> usize {
        match error {
            PartitionError::PartitionerFailed { .. } => 0,
            PartitionError::NeedsMpiCommunicator(_) => 1,
            PartitionError::TooLarge => 2,
            PartitionError::LengthMismatch { .. } => 3,
            PartitionError::IndexOutOfRange { .. } => 4,
            PartitionError::MessageTooLarge { .. } => 5,
            PartitionError::RemoteFailure { .. } => 6,
        }
    }

    #[test]
    fn partition_error_variant_list_is_exhaustive() {
        assert_eq!(variant_index(&PartitionError::TooLarge), 2);
    }
}

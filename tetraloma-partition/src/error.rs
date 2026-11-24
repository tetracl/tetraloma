// Design: "Partitioning", Listing S3; ADR-0074.
//! `PartitionError` and `Result` of `tetraloma-partition`.

use core::fmt;

use tetraloma_comm::CommError;

/// The result type of `tetraloma-partition`: `Ok(T)` or a [`PartitionError`].
///
/// Every fallible partitioner operation returns it, and a [`PartitionError`] converts into
/// `Box<dyn std::error::Error>`. The tutorial [Distributed
/// meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html#partitioners)
/// explains how partitioners report errors.
///
/// # Example
///
/// ```
/// use tetraloma_partition::{PartitionError, Result};
///
/// fn check_parts(nparts: usize) -> Result<usize> {
///     if nparts > 0 {
///         Ok(nparts)
///     } else {
///         Err(PartitionError::LengthMismatch { expected: 1, found: 0 })
///     }
/// }
/// assert_eq!(check_parts(2).unwrap(), 2);
/// assert!(matches!(
///     check_parts(0),
///     Err(PartitionError::LengthMismatch { expected: 1, found: 0 })
/// ));
/// ```
pub type Result<T, E = PartitionError> = core::result::Result<T, E>;

// Design: "Partitioning", Listing S3; ADR-0074.
/// Errors of distributed graph partitioning.
///
/// Each variant names one kind of failure, and its `Display` text says what went wrong. The enum
/// is `#[non_exhaustive]`, so a `match` on it needs a `_` arm. Collective operations agree on
/// their errors: the rank that failed returns its own error, and the other ranks return
/// [`RemoteFailure`](PartitionError::RemoteFailure). See [Distributed
/// meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html#partitioners).
///
/// # Example
///
/// ```
/// use tetraloma_partition::PartitionError;
///
/// let e = PartitionError::NeedsMpiCommunicator("ptscotch");
/// assert_eq!(
///     e.to_string(),
///     "the ptscotch partitioner needs an MPI communicator for more than one part"
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum PartitionError {
    // ADR-0029, item 6.
    /// A partitioner library returned the status `code`; tetraloma's own conditions use negative
    /// codes: −1 for ParMETIS with an empty rank, −2 for a self-loop, −3 for an asymmetric
    /// graph. With `name` `"metis"`, a code after validation is METIS's own status
    /// (`rstatus_et`: −2 input error, −3 out of memory, −4 other error).
    PartitionerFailed {
        /// The partitioner's name, as `"metis"`.
        name: &'static str,
        /// The library's status, or tetraloma's negative code.
        code: i32,
    },
    // ADR-0029, item 6.
    /// A distributed partitioner was asked for more than one part on a communicator without an
    /// MPI handle (`CommDyn::raw` is `None`), on any number of ranks.
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
    /// An MPI count or displacement of `count` elements exceeds `i32::MAX`, the limit of MPI's
    /// `int` counts.
    MessageTooLarge {
        /// The count or displacement, in elements.
        count: usize,
    },
    /// A collective operation failed on another rank; `rank` is the lowest failing rank, which
    /// returns its own error.
    RemoteFailure {
        /// The lowest rank on which the operation failed.
        rank: usize,
    },
}

impl fmt::Display for PartitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartitionError::PartitionerFailed { name, code } => {
                write!(f, "the {name} partitioner failed with status {code}")
            }
            PartitionError::NeedsMpiCommunicator(name) => write!(
                f,
                "the {name} partitioner needs an MPI communicator for more than one part"
            ),
            PartitionError::TooLarge => f.write_str("a size or index does not fit its type"),
            PartitionError::LengthMismatch { expected, found } => {
                write!(f, "length mismatch: expected {expected}, found {found}")
            }
            PartitionError::IndexOutOfRange { index, len } => {
                write!(f, "index {index} is out of range for length {len}")
            }
            PartitionError::MessageTooLarge { count } => write!(
                f,
                "a message count or displacement of {count} elements exceeds i32::MAX"
            ),
            PartitionError::RemoteFailure { rank } => {
                write!(f, "the operation failed on rank {rank}")
            }
        }
    }
}

impl std::error::Error for PartitionError {}

// Design: "Partitioning", Listing S3; ADR-0074.
impl From<CommError> for PartitionError {
    fn from(error: CommError) -> Self {
        match error {
            CommError::TooLarge => PartitionError::TooLarge,
            CommError::LengthMismatch { expected, found } => {
                PartitionError::LengthMismatch { expected, found }
            }
            CommError::IndexOutOfRange { index, len } => {
                PartitionError::IndexOutOfRange { index, len }
            }
            CommError::MessageTooLarge { count } => PartitionError::MessageTooLarge { count },
            CommError::RemoteFailure { rank } => PartitionError::RemoteFailure { rank },
            // Required by `#[non_exhaustive]` across crates; adding a variant breaks
            // `tetraloma_comm::error::tests::comm_error_variant_list_is_exhaustive` in
            // `tetraloma-comm`'s test build (it does not break this library build).
            _ => unreachable!("non-exhaustive CommError variant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PartitionError;
    use tetraloma_comm::CommError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `PartitionError`
    // breaks this test and points to `tetraloma::pipeline::dual_graph::partition_error` and to
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

    #[test]
    fn partition_error_display_and_error_trait() {
        let cases = [
            (
                PartitionError::PartitionerFailed {
                    name: "parmetis",
                    code: -1,
                },
                "the parmetis partitioner failed with status -1",
            ),
            (
                PartitionError::NeedsMpiCommunicator("ptscotch"),
                "the ptscotch partitioner needs an MPI communicator for more than one part",
            ),
            (
                PartitionError::TooLarge,
                "a size or index does not fit its type",
            ),
            (
                PartitionError::LengthMismatch {
                    expected: 4,
                    found: 2,
                },
                "length mismatch: expected 4, found 2",
            ),
            (
                PartitionError::IndexOutOfRange { index: 5, len: 3 },
                "index 5 is out of range for length 3",
            ),
            (
                PartitionError::MessageTooLarge { count: 1 << 31 },
                "a message count or displacement of 2147483648 elements exceeds i32::MAX",
            ),
            (
                PartitionError::RemoteFailure { rank: 3 },
                "the operation failed on rank 3",
            ),
        ];
        for (err, want) in cases {
            assert_eq!(err.to_string(), want);
            let dyn_err: &dyn std::error::Error = &err;
            assert!(dyn_err.source().is_none());
        }
    }

    #[test]
    fn comm_error_converts_to_matching_partition_error() {
        assert!(matches!(
            PartitionError::from(CommError::TooLarge),
            PartitionError::TooLarge
        ));
        assert!(matches!(
            PartitionError::from(CommError::LengthMismatch {
                expected: 11,
                found: 12,
            }),
            PartitionError::LengthMismatch {
                expected: 11,
                found: 12,
            }
        ));
        assert!(matches!(
            PartitionError::from(CommError::IndexOutOfRange { index: 13, len: 14 }),
            PartitionError::IndexOutOfRange { index: 13, len: 14 }
        ));
        assert!(matches!(
            PartitionError::from(CommError::MessageTooLarge { count: 15 }),
            PartitionError::MessageTooLarge { count: 15 }
        ));
        assert!(matches!(
            PartitionError::from(CommError::RemoteFailure { rank: 16 }),
            PartitionError::RemoteFailure { rank: 16 }
        ));
    }
}

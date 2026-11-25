// Design: "Distributed mesh model", Listing S2c; ADR-0074.
//! `CommError` and `Result` of `tetraloma-comm`.

use core::fmt;

/// The result type of `tetraloma-comm`: `Ok(T)` or a [`CommError`].
///
/// Every fallible communicator operation returns it, and a [`CommError`] converts into
/// `Box<dyn std::error::Error>` as well as `tetraloma`'s `GridError`. The tutorial [Distributed
/// meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html#building-on-a-communicator)
/// describes how communicators report errors.
///
/// # Example
///
/// ```
/// use tetraloma_comm::{CommError, Result};
///
/// fn check_rank(rank: usize, size: usize) -> Result<usize> {
///     if rank < size {
///         Ok(rank)
///     } else {
///         Err(CommError::IndexOutOfRange { index: rank, len: size })
///     }
/// }
/// assert_eq!(check_rank(0, 2).unwrap(), 0);
/// assert!(matches!(
///     check_rank(2, 2),
///     Err(CommError::IndexOutOfRange { index: 2, len: 2 })
/// ));
/// ```
pub type Result<T, E = CommError> = core::result::Result<T, E>;

// Design: Listing S2c; ADR-0074.
/// Errors of the communication layer.
///
/// Each variant names one kind of failure, and its `Display` text says what went wrong. The enum
/// is `#[non_exhaustive]`, so a `match` on it needs a `_` arm. Collective operations agree on
/// their errors: the rank that failed returns its own error, and the other ranks return
/// [`RemoteFailure`](CommError::RemoteFailure). See [Distributed
/// meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html#building-on-a-communicator).
///
/// # Example
///
/// ```
/// use tetraloma_comm::CommError;
///
/// let e = CommError::RemoteFailure { rank: 2 };
/// assert_eq!(e.to_string(), "the operation failed on rank 2");
/// ```
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

impl fmt::Display for CommError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommError::TooLarge => f.write_str("a size or index does not fit its type"),
            CommError::LengthMismatch { expected, found } => {
                write!(f, "length mismatch: expected {expected}, found {found}")
            }
            CommError::IndexOutOfRange { index, len } => {
                write!(f, "index {index} is out of range for length {len}")
            }
            CommError::MessageTooLarge { count } => write!(
                f,
                "a message count or displacement of {count} elements exceeds i32::MAX"
            ),
            CommError::RemoteFailure { rank } => {
                write!(f, "the operation failed on rank {rank}")
            }
        }
    }
}

impl std::error::Error for CommError {}

#[cfg(test)]
mod tests {
    use super::CommError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `CommError`
    // breaks this test and points to `From<CommError> for GridError` (in `tetraloma-device`
    // until M18.T6, then `tetraloma-topology`), `From<CommError> for PartitionError` (in
    // `tetraloma-partition`), `From<CommError> for ParallelError` (in `tetraloma-parallel`),
    // and to
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

    #[test]
    fn comm_error_display_and_error_trait() {
        let cases = [
            (CommError::TooLarge, "a size or index does not fit its type"),
            (
                CommError::LengthMismatch {
                    expected: 4,
                    found: 2,
                },
                "length mismatch: expected 4, found 2",
            ),
            (
                CommError::IndexOutOfRange { index: 5, len: 3 },
                "index 5 is out of range for length 3",
            ),
            (
                CommError::MessageTooLarge { count: 1 << 31 },
                "a message count or displacement of 2147483648 elements exceeds i32::MAX",
            ),
            (
                CommError::RemoteFailure { rank: 3 },
                "the operation failed on rank 3",
            ),
        ];
        for (err, want) in cases {
            assert_eq!(err.to_string(), want);
            let dyn_err: &dyn std::error::Error = &err;
            assert!(dyn_err.source().is_none());
        }
    }
}

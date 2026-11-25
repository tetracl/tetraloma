// Design: "Distributed mesh model", Listing S2c; ADR-0074.
//! `ParallelError` and the crate-private `Result` alias of `tetraloma-parallel`.

use core::fmt;

use tetraloma_comm::CommError;
use tetraloma_device::{DeviceError, GridError, ServerError};

/// Crate-private result alias for `tetraloma-parallel` functions while
/// `tetraloma_parallel::Result` remains an alias of `GridError` until M18.T6.
pub(crate) type Result<T, E = ParallelError> = core::result::Result<T, E>;

// Design: "Distributed mesh model", Listing S2c; ADR-0074.
/// Errors of index maps and halo exchanges.
///
/// Each variant names one kind of failure, and its `Display` text says what went wrong. The enum
/// is `#[non_exhaustive]`, so a `match` on it needs a `_` arm. Collective operations agree on
/// their errors: the rank that failed returns its own error, and the other ranks return
/// [`RemoteFailure`](ParallelError::RemoteFailure). Every `ParallelError` converts into
/// `tetraloma`'s `GridError` through `From`, so `?` passes it through functions returning
/// `tetraloma::Result`. See [Halo
/// exchange](https://tetracl.github.io/tetraloma/tutorials/halo-exchange.html).
///
/// # Example
///
/// ```
/// use tetraloma_parallel::ParallelError;
///
/// let e = ParallelError::RemoteFailure { rank: 2 };
/// assert_eq!(e.to_string(), "the operation failed on rank 2");
/// ```
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
        /// The name of the scalar type, as `"f64"`.
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

impl fmt::Display for ParallelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParallelError::TooLarge => f.write_str("a size or index does not fit its type"),
            ParallelError::AllocationTooLarge { bytes, max } => write!(
                f,
                "an allocation of {bytes} bytes exceeds the device's max_page_size of {max} bytes"
            ),
            ParallelError::UnsupportedPrecision { scalar, device } => {
                write!(f, "{device} does not support {scalar}")
            }
            ParallelError::LengthMismatch { expected, found } => {
                write!(f, "length mismatch: expected {expected}, found {found}")
            }
            ParallelError::Server(error) => write!(f, "compute server error: {error}"),
            ParallelError::IndexOutOfRange { index, len } => {
                write!(f, "index {index} is out of range for length {len}")
            }
            ParallelError::MessageTooLarge { count } => write!(
                f,
                "a message count or displacement of {count} elements exceeds i32::MAX"
            ),
            ParallelError::RemoteFailure { rank } => {
                write!(f, "the operation failed on rank {rank}")
            }
        }
    }
}

impl std::error::Error for ParallelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParallelError::Server(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ServerError> for ParallelError {
    fn from(error: ServerError) -> Self {
        ParallelError::Server(error)
    }
}

// Design: "Distributed mesh model", Listing S2c; ADR-0074.
impl From<CommError> for ParallelError {
    fn from(error: CommError) -> Self {
        match error {
            CommError::TooLarge => ParallelError::TooLarge,
            CommError::LengthMismatch { expected, found } => {
                ParallelError::LengthMismatch { expected, found }
            }
            CommError::IndexOutOfRange { index, len } => {
                ParallelError::IndexOutOfRange { index, len }
            }
            CommError::MessageTooLarge { count } => ParallelError::MessageTooLarge { count },
            CommError::RemoteFailure { rank } => ParallelError::RemoteFailure { rank },
            // Required by `#[non_exhaustive]` across crates; adding a variant breaks
            // `tetraloma_comm::error::tests::comm_error_variant_list_is_exhaustive` in
            // `tetraloma-comm`'s test build (it does not break this library build).
            _ => unreachable!("non-exhaustive CommError variant"),
        }
    }
}

// Design: "Distributed mesh model", Listing S2c; ADR-0074.
impl From<DeviceError> for ParallelError {
    fn from(error: DeviceError) -> Self {
        match error {
            DeviceError::TooLarge => ParallelError::TooLarge,
            DeviceError::AllocationTooLarge { bytes, max } => {
                ParallelError::AllocationTooLarge { bytes, max }
            }
            DeviceError::UnsupportedPrecision { scalar, device } => {
                ParallelError::UnsupportedPrecision { scalar, device }
            }
            DeviceError::LengthMismatch { expected, found } => {
                ParallelError::LengthMismatch { expected, found }
            }
            DeviceError::Server(error) => ParallelError::Server(error),
            // Required by `#[non_exhaustive]` across crates; adding a variant breaks
            // `tetraloma_device::device_error::tests::device_error_variant_list_is_exhaustive` in
            // `tetraloma-device`'s test build (it does not break this library build).
            _ => unreachable!("non-exhaustive DeviceError variant"),
        }
    }
}

// Design: section 7; ADR-0074. Moves to tetraloma-topology with `GridError` in M18.T6.
impl From<ParallelError> for GridError {
    fn from(error: ParallelError) -> Self {
        match error {
            ParallelError::TooLarge => GridError::TooLarge,
            ParallelError::AllocationTooLarge { bytes, max } => {
                GridError::AllocationTooLarge { bytes, max }
            }
            ParallelError::UnsupportedPrecision { scalar, device } => {
                GridError::UnsupportedPrecision { scalar, device }
            }
            ParallelError::LengthMismatch { expected, found } => {
                GridError::LengthMismatch { expected, found }
            }
            ParallelError::Server(error) => GridError::Server(error),
            ParallelError::IndexOutOfRange { index, len } => {
                GridError::IndexOutOfRange { index, len }
            }
            ParallelError::MessageTooLarge { count } => GridError::MessageTooLarge { count },
            ParallelError::RemoteFailure { rank } => GridError::RemoteFailure { rank },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ParallelError;
    use tetraloma_comm::CommError;
    use tetraloma_device::{DeviceError, GridError, ServerError};

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `ParallelError`
    // breaks this test and points to `From<ParallelError> for GridError` (in `tetraloma-parallel`
    // until M18.T6, then `tetraloma-topology`) and to
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
        assert_eq!(
            variant_index(&ParallelError::Server(ServerError::TornDown)),
            4
        );
    }

    #[test]
    fn parallel_error_display_error_and_from_server() {
        let cases = [
            (
                ParallelError::TooLarge,
                "a size or index does not fit its type",
            ),
            (
                ParallelError::AllocationTooLarge {
                    bytes: 2048,
                    max: 1024,
                },
                "an allocation of 2048 bytes exceeds the device's max_page_size of 1024 bytes",
            ),
            (
                ParallelError::UnsupportedPrecision {
                    scalar: "f64",
                    device: "metal",
                },
                "metal does not support f64",
            ),
            (
                ParallelError::LengthMismatch {
                    expected: 4,
                    found: 2,
                },
                "length mismatch: expected 4, found 2",
            ),
            (
                ParallelError::IndexOutOfRange { index: 5, len: 3 },
                "index 5 is out of range for length 3",
            ),
            (
                ParallelError::MessageTooLarge { count: 1 << 31 },
                "a message count or displacement of 2147483648 elements exceeds i32::MAX",
            ),
            (
                ParallelError::RemoteFailure { rank: 3 },
                "the operation failed on rank 3",
            ),
        ];
        for (err, want) in cases {
            assert_eq!(err.to_string(), want);
            let dyn_err: &dyn std::error::Error = &err;
            assert!(dyn_err.source().is_none());
        }

        let server = ServerError::TornDown;
        let server_display = server.to_string();
        let expected_display = format!("compute server error: {server_display}");
        let err = ParallelError::from(server);
        assert!(matches!(err, ParallelError::Server(ServerError::TornDown)));
        assert_eq!(err.to_string(), expected_display);
        let dyn_err: &dyn std::error::Error = &err;
        assert_eq!(
            dyn_err.source().expect("Server source").to_string(),
            server_display
        );
    }

    #[test]
    fn comm_and_device_errors_convert_to_matching_parallel_error() {
        let comm_cases = [
            CommError::TooLarge,
            CommError::LengthMismatch {
                expected: 11,
                found: 12,
            },
            CommError::IndexOutOfRange { index: 13, len: 14 },
            CommError::MessageTooLarge { count: 15 },
            CommError::RemoteFailure { rank: 16 },
        ];
        for e in comm_cases {
            let p = ParallelError::from(e.clone());
            assert_eq!(p.to_string(), e.to_string());
        }
        assert!(matches!(
            ParallelError::from(CommError::TooLarge),
            ParallelError::TooLarge
        ));
        assert!(matches!(
            ParallelError::from(CommError::LengthMismatch {
                expected: 11,
                found: 12,
            }),
            ParallelError::LengthMismatch {
                expected: 11,
                found: 12,
            }
        ));
        assert!(matches!(
            ParallelError::from(CommError::IndexOutOfRange { index: 13, len: 14 }),
            ParallelError::IndexOutOfRange { index: 13, len: 14 }
        ));
        assert!(matches!(
            ParallelError::from(CommError::MessageTooLarge { count: 15 }),
            ParallelError::MessageTooLarge { count: 15 }
        ));
        assert!(matches!(
            ParallelError::from(CommError::RemoteFailure { rank: 16 }),
            ParallelError::RemoteFailure { rank: 16 }
        ));

        let device_cases = [
            DeviceError::TooLarge,
            DeviceError::AllocationTooLarge { bytes: 21, max: 22 },
            DeviceError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-a",
            },
            DeviceError::LengthMismatch {
                expected: 23,
                found: 24,
            },
            DeviceError::Server(ServerError::TornDown),
        ];
        for e in device_cases {
            let p = ParallelError::from(e.clone());
            assert_eq!(p.to_string(), e.to_string());
        }
        assert!(matches!(
            ParallelError::from(DeviceError::TooLarge),
            ParallelError::TooLarge
        ));
        assert!(matches!(
            ParallelError::from(DeviceError::AllocationTooLarge { bytes: 21, max: 22 }),
            ParallelError::AllocationTooLarge { bytes: 21, max: 22 }
        ));
        assert!(matches!(
            ParallelError::from(DeviceError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-a",
            }),
            ParallelError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-a",
            }
        ));
        assert!(matches!(
            ParallelError::from(DeviceError::LengthMismatch {
                expected: 23,
                found: 24,
            }),
            ParallelError::LengthMismatch {
                expected: 23,
                found: 24,
            }
        ));
        assert!(matches!(
            ParallelError::from(DeviceError::Server(ServerError::TornDown)),
            ParallelError::Server(ServerError::TornDown)
        ));
    }

    #[test]
    fn parallel_error_converts_to_matching_grid_error() {
        let cases = [
            ParallelError::TooLarge,
            ParallelError::AllocationTooLarge { bytes: 31, max: 32 },
            ParallelError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-b",
            },
            ParallelError::LengthMismatch {
                expected: 33,
                found: 34,
            },
            ParallelError::Server(ServerError::TornDown),
            ParallelError::IndexOutOfRange { index: 35, len: 36 },
            ParallelError::MessageTooLarge { count: 37 },
            ParallelError::RemoteFailure { rank: 38 },
        ];
        for e in cases {
            let g = GridError::from(e.clone());
            assert_eq!(g.to_string(), e.to_string());
        }
    }
}

// Design: "Device model and backends", Listing S1; ADR-0074.
//! `DeviceError` and the crate-private `Result` alias of `tetraloma-device`.

use core::fmt;

use cubecl::server::ServerError;

/// Crate-private result alias for `tetraloma-device` functions while `tetraloma_device::Result`
/// remains an alias of `GridError` until M18.T6.
pub(crate) type Result<T, E = DeviceError> = core::result::Result<T, E>;

// Design: "Device model and backends", Listing S1; ADR-0074.
/// Errors of the device layer.
///
/// Each variant names one kind of failure, and its `Display` text says what went wrong. The enum
/// is `#[non_exhaustive]`, so a `match` on it needs a `_` arm. Every `DeviceError` converts into
/// `tetraloma`'s `GridError` through `From`, so `?` passes it through functions returning
/// `tetraloma::Result`. See [Device
/// problems](https://tetracl.github.io/tetraloma/troubleshooting.html#device-problems).
///
/// # Example
///
/// ```
/// use tetraloma_device::DeviceError;
///
/// let e = DeviceError::AllocationTooLarge { bytes: 2048, max: 1024 };
/// assert_eq!(
///     e.to_string(),
///     "an allocation of 2048 bytes exceeds the device's max_page_size of 1024 bytes"
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DeviceError {
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
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceError::TooLarge => f.write_str("a size or index does not fit its type"),
            DeviceError::AllocationTooLarge { bytes, max } => write!(
                f,
                "an allocation of {bytes} bytes exceeds the device's max_page_size of {max} bytes"
            ),
            DeviceError::UnsupportedPrecision { scalar, device } => {
                write!(f, "{device} does not support {scalar}")
            }
            DeviceError::LengthMismatch { expected, found } => {
                write!(f, "length mismatch: expected {expected}, found {found}")
            }
            DeviceError::Server(error) => write!(f, "compute server error: {error}"),
        }
    }
}

impl std::error::Error for DeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DeviceError::Server(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ServerError> for DeviceError {
    fn from(error: ServerError) -> Self {
        DeviceError::Server(error)
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceError;
    use cubecl::server::{Collective, ServerError};

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `DeviceError`
    // breaks this test and points to `From<DeviceError> for GridError` (in `tetraloma-device`
    // until M18.T6, then `tetraloma-topology`) and to
    // `tetraloma::pipeline::dual_graph::tests::foundation_errors_convert_to_their_grid_error_variants`.
    fn variant_index(error: &DeviceError) -> usize {
        match error {
            DeviceError::TooLarge => 0,
            DeviceError::AllocationTooLarge { .. } => 1,
            DeviceError::UnsupportedPrecision { .. } => 2,
            DeviceError::LengthMismatch { .. } => 3,
            DeviceError::Server(_) => 4,
        }
    }

    #[test]
    fn device_error_variant_list_is_exhaustive() {
        assert_eq!(variant_index(&DeviceError::TooLarge), 0);
        // Exercises the public `ServerError` re-export used by `tetraloma-parallel`.
        let err = DeviceError::Server(crate::ServerError::TornDown);
        assert_eq!(variant_index(&err), 4);
    }

    #[test]
    fn device_error_display_error_and_from_server() {
        let cases = [
            (
                DeviceError::TooLarge,
                "a size or index does not fit its type",
            ),
            (
                DeviceError::AllocationTooLarge {
                    bytes: 2048,
                    max: 1024,
                },
                "an allocation of 2048 bytes exceeds the device's max_page_size of 1024 bytes",
            ),
            (
                DeviceError::UnsupportedPrecision {
                    scalar: "f64",
                    device: "metal",
                },
                "metal does not support f64",
            ),
            (
                DeviceError::LengthMismatch {
                    expected: 4,
                    found: 2,
                },
                "length mismatch: expected 4, found 2",
            ),
        ];
        for (err, want) in cases {
            assert_eq!(err.to_string(), want);
            let dyn_err: &dyn std::error::Error = &err;
            assert!(dyn_err.source().is_none());
        }

        let server = ServerError::no_device_transport(Collective::AllReduce);
        let expected_display = format!("compute server error: {server}");
        let err = DeviceError::from(server);
        assert!(matches!(
            err,
            DeviceError::Server(ServerError::NoDeviceTransport {
                operation: Collective::AllReduce,
                ..
            })
        ));
        assert_eq!(err.to_string(), expected_display);
        let dyn_err: &dyn std::error::Error = &err;
        assert!(dyn_err.source().is_some());
    }
}

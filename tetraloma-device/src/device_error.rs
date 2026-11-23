//! Stub for `DeviceError` and its `From<DeviceError> for GridError` conversion (M18.T0; ADR-0074).

use cubecl::server::ServerError;

use crate::GridError;

// M18.T0 stub (ADR-0074, design section 7); implemented and unhidden in M18.T4.
/// Errors of the device layer. Not part of the stable API while hidden.
#[doc(hidden)]
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
}

impl From<DeviceError> for GridError {
    fn from(_: DeviceError) -> Self {
        unimplemented!("pending M18.T4")
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceError;

    // M18 A4: exhaustive match without a wildcard arm. Adding a variant to `DeviceError`
    // breaks this test and points to
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
        // Rule 7 named test for the hidden `ServerError` re-export used by `tetraloma-parallel`.
        let err = DeviceError::Server(crate::ServerError::TornDown);
        assert_eq!(variant_index(&err), 4);
    }
}

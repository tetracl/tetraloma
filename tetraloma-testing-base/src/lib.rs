// Design: "Workspace layout", "Testing and validation"; ADR-0074, ADR-0075.
//! Device and parallel test support below the mesh: the test device selected by
//! [`TEST_DEVICE_VAR`] ([`test_client`], [`test_client_at`], [`supports`], [`bitwise_device`]),
//! precision tolerances ([`TOL_F32`], [`TOL_F64`]), random ghost layouts ([`ghost_layout`],
//! [`ghost_layout_sized`]), and the shared primitive and scatter checks ([`primitive_checks`],
//! [`scatter_checks`]).
//!
//! This crate lives in the repository and is not published. It is for the developers of tetraloma
//! to test the foundation crates without depending on a mesh crate. Application programs open
//! devices directly through `cubecl::Device`, as [Compute devices] in the [user book] explains.
//!
//! [user book]: https://tetracl.github.io/tetraloma/
//! [Compute devices]: https://tetracl.github.io/tetraloma/devices.html#choosing-a-device-at-run-time

pub use tetraloma_testing::comm_checks::{ghost_layout, ghost_layout_sized};
#[cfg(feature = "cpu")]
pub use tetraloma_testing::cpu_client;
pub use tetraloma_testing::{
    TEST_DEVICE_VAR, TOL_F32, TOL_F64, bitwise_device, primitive_checks, scatter_checks, supports,
    test_client, test_client_at,
};

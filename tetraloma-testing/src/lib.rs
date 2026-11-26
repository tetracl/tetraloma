// Design: "Cargo features", "Dependency rules"; ADR-0059, ADR-0060, ADR-0074.
//! Test support for tetraloma: the test device chosen by `TETRALOMA_TEST_DEVICE`, test meshes,
//! tolerance helpers, a threaded communicator, the Python oracles and the rank-count guard of the
//! MPI runs ([`check_world_size`]).
//!
//! This crate lives in the repository and is not published. It re-exports `tetraloma-testing-base`
//! (and `tetraloma-comm-testing`) alongside the mesh generators and oracles for tests above the
//! foundation. A program of your own opens its device through `cubecl::Device` and builds its
//! meshes with the mesh builder or a file reader, as the [user book] shows from [A first program]
//! on. The API examples name the test mesh each hidden line makes, so you can follow them without
//! this crate.
//!
//! The variable `TETRALOMA_TEST_DEVICE` (for example `cpu`, `metal`, `vulkan` or `cuda`) picks the
//! device of the tests; [Compute devices] describes the same choice for programs. The crate's
//! features forward `tetraloma-testing-base`'s and CubeCL's runtime features; besides
//! `tetraloma-testing-base` and this crate, only tetraloma-bench and tetraloma-examples enable
//! them. tetraloma-device's feature `gpu-aware-cuda` is empty and reserved.
//!
//! [user book]: https://tetracl.github.io/tetraloma/
//! [A first program]: https://tetracl.github.io/tetraloma/getting-started/first-program.html
//! [Compute devices]: https://tetracl.github.io/tetraloma/devices.html#choosing-a-device-at-run-time

pub mod io_parallel;
mod meshes;
pub mod oracle;
pub mod parallel;
pub mod partition;
pub mod reference;

pub use meshes::{
    HostBlock, HostChunk, HostChunkBlock, HostMesh, HostTags, ball, book, circle, cube_box,
    hemisphere, hex_prism_pyramid_box, interval_box, mixed_box, mixed_surface, mobius_strip,
    sphere, square_box, triangle_strip, y_junction,
};
#[cfg(feature = "cpu")]
pub use tetraloma_testing_base::cpu_client;
pub use tetraloma_testing_base::{
    EXPECT_RANKS_VAR, TEST_DEVICE_VAR, TOL_F32, TOL_F64, ThreadComm, bitwise_device,
    check_world_size, check_world_size_against, comm_checks, each_scalar, primitive_checks,
    scatter_checks, supports, test_client, test_client_at,
};

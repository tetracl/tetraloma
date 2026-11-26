// Design: "Workspace layout", "Testing and validation"; ADR-0074, ADR-0075.
//! Host-only test support below the mesh: the in-process threaded communicator ([`ThreadComm`]),
//! communicator checks ([`comm_checks`]), and the rank-count guard ([`check_world_size`],
//! [`check_world_size_against`], [`EXPECT_RANKS_VAR`]).
//!
//! This crate lives in the repository and is not published. It is for the developers of tetraloma
//! to test distributed code without depending on CubeCL or any mesh crate. Application programs
//! use `Serial` or `Mpi` as shown in the [Distributed meshes] tutorial of the [user book].
//!
//! [user book]: https://tetracl.github.io/tetraloma/
//! [Distributed meshes]: https://tetracl.github.io/tetraloma/tutorials/distributed.html

mod comm;
pub mod comm_checks;
mod world_size;

pub use comm::ThreadComm;
pub use world_size::{EXPECT_RANKS_VAR, check_world_size, check_world_size_against};

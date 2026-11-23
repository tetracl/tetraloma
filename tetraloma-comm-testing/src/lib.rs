// Design: "Workspace layout", "Testing and validation"; ADR-0074, ADR-0075.
//! Host-only test support below the mesh: the in-process threaded communicator ([`ThreadComm`])
//! and the rank-count guard ([`check_world_size`], [`check_world_size_against`],
//! [`EXPECT_RANKS_VAR`]).
//!
//! This crate lives in the repository and is not published. It is for the developers of tetraloma
//! to test distributed code without depending on a mesh crate. Application programs use `Serial`
//! or `Mpi` as shown in the [Distributed meshes] tutorial of the [user book].
//!
//! [user book]: https://tetracl.github.io/tetraloma/
//! [Distributed meshes]: https://tetracl.github.io/tetraloma/tutorials/distributed.html

pub use tetraloma_testing::{
    EXPECT_RANKS_VAR, ThreadComm, check_world_size, check_world_size_against,
};

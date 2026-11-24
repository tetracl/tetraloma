// Design: "Dependency rules" 5 (no MPI dependency without the feature `mpi`) and 9 (host-only,
// no CubeCL or mesh dependency); Listing S2c; ADR-0074.
//! Host-only communicators and neighbourhood exchange: the communication layer of tetraloma.
//!
//! - [`CommDyn`], [`Comm`] and [`CommExt`] are the interface of a communicator, over the element
//!   types [`DType`] / [`CommScalar`] and reduction operations [`ReduceOp`].
//! - [`Serial`] is the communicator of one process; with the feature `mpi`, `Mpi` wraps an MPI
//!   communicator.
//! - [`Neighbourhood`] lists the ranks a rank exchanges data with, backed by an MPI
//!   distributed-graph communicator when built on `Mpi`.
//! - [`owner_hash`] hashes entity keys consistently across platforms, and [`local_gpu_index`]
//!   returns a rank's index within its node.
//! - [`CommError`] and [`Result`] are the error and result types of this crate.
//!
//! Without the feature `mpi` the crate does not depend on MPI, and it does not depend on CubeCL or
//! any mesh crate. `tetraloma-parallel` re-exports these items except [`Result`], and the facade
//! crate `tetraloma` re-exports the communicators and [`CommError`]. The tutorials [Distributed
//! meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html) and [Halo
//! exchange](https://tetracl.github.io/tetraloma/tutorials/halo-exchange.html) show how to use
//! them.

mod comm;
mod counts;
mod error;
#[cfg(feature = "mpi")]
mod graph;
mod hash;
mod local_gpu;
#[cfg(feature = "mpi")]
mod mpi;
mod serial;

#[doc(hidden)]
pub use comm::agree;
pub use comm::{Comm, CommDyn, CommExt, CommScalar, DType, Neighbourhood, ReduceOp};
pub use error::{CommError, Result};
pub use hash::owner_hash;
pub use local_gpu::local_gpu_index;
#[cfg(feature = "mpi")]
pub use mpi::Mpi;
pub use serial::Serial;

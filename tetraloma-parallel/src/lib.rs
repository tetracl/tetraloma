// Design: "Dependency rules" 5 (no MPI dependency without the feature `mpi`); Listing S2c;
// milestones M2 (the communication core), M6 (`Mpi`, `Scatterer`) and M18 (`tetraloma-comm`,
// ADR-0074).
//! Index maps, halo exchange and re-exported communicators: the parallel layer of tetraloma.
//!
//! - [`CommDyn`], [`Comm`] and [`CommExt`] are the interface of a communicator, defined in
//!   `tetraloma-comm` and re-exported here. [`Serial`] is the communicator of one process; with
//!   the feature `mpi`, `Mpi` wraps an MPI communicator.
//! - [`IndexMap`] says which rank owns each index of a distributed set of entities, and which
//!   indices each rank holds as ghosts.
//! - [`Scatterer`] runs the halo exchange of an index map on device arrays: forward from the
//!   owners to the ghosts, and reverse from the ghosts to their owners.
//! - [`Neighbourhood`] lists the ranks a rank exchanges data with.
//! - [`ParallelError`] and [`Result`] report errors of index maps and halo exchanges.
//!
//! Without the feature `mpi` the crate does not depend on MPI. The facade crate `tetraloma`
//! re-exports its types and communicators; `tetraloma::Result` is the alias of `GridError`, not
//! of [`ParallelError`]. The tutorials [Distributed
//! meshes](https://tetracl.github.io/tetraloma/tutorials/distributed.html) and [Halo
//! exchange](https://tetracl.github.io/tetraloma/tutorials/halo-exchange.html) show how to use
//! it.

mod error;
mod index_map;
mod scatterer;

pub use error::{ParallelError, Result};
pub use index_map::IndexMap;
pub use scatterer::{ReverseOp, ScatterNumeric, Scatterer, Transport};
#[cfg(feature = "mpi")]
pub use tetraloma_comm::Mpi;
pub use tetraloma_comm::{
    Comm, CommDyn, CommError, CommExt, CommScalar, DType, Neighbourhood, ReduceOp, Serial,
    local_gpu_index, owner_hash,
};
// Design: ADR-0026, item 15; ADR-0074; hidden item (dependency rule 7), re-exported for
// workspace crates above `tetraloma-parallel` (`tetraloma-topology`, `tetraloma-io`).
#[doc(hidden)]
pub use tetraloma_comm::agree;

#[cfg(test)]
mod tests {
    use super::*;
    use tetraloma_testing_base::ThreadComm;

    #[test]
    fn hidden_agree_reexport_agrees_on_parallel_error() {
        let out = ThreadComm::run(3, |comm| {
            let local: Result<usize> = if comm.rank() == 1 {
                Err(ParallelError::TooLarge)
            } else {
                Ok(comm.rank())
            };
            crate::agree(&comm, local)
        });
        assert!(matches!(
            out[0],
            Err(ParallelError::RemoteFailure { rank: 1 })
        ));
        assert!(matches!(out[1], Err(ParallelError::TooLarge)));
        assert!(matches!(
            out[2],
            Err(ParallelError::RemoteFailure { rank: 1 })
        ));
    }
}

//! Multi-rank communicator checks on 1–4 ranks of `ThreadComm`, testing `tetraloma-comm` directly
//! via `tetraloma-comm-testing` without CubeCL or any mesh crate (docs/plans/M18.md, T7).

use tetraloma_comm::CommDyn;
use tetraloma_comm_testing::{ThreadComm, comm_checks};

const SEEDS: [u64; 4] = [0x1, 0x2a, 0xdead_beef, 0x9e37_79b9_7f4a_7c15];

#[test]
fn communicator_checks_on_one_to_four_thread_ranks() {
    for n in 1..=4 {
        let ranks = ThreadComm::run(n, |comm| {
            assert_eq!(comm.size(), n);
            if n == 1 {
                comm_checks::matches_serial(&comm);
            } else {
                comm_checks::wrong_counts(&comm);
            }
            comm_checks::identities(&comm);
            comm_checks::ring(&comm);
            for seed in SEEDS {
                comm_checks::neighbour_exchange(&comm, seed);
            }
            comm.rank()
        });
        assert_eq!(ranks, (0..n).collect::<Vec<_>>());
    }
}

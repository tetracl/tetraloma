//! Multi-rank test of `Block` on a distributed ring graph on 1–4 ranks of `ThreadComm`, testing
//! `tetraloma-partition` directly via `tetraloma-comm-testing` without CubeCL or any mesh crate
//! (docs/plans/M18.md, T7).

use tetraloma_comm::{CommDyn, Serial};
use tetraloma_comm_testing::ThreadComm;
use tetraloma_partition::{Block, DistGraph, PartitionError, Partitioner};

/// A ring graph of `total` vertices split evenly (remainder on early ranks) across `nranks`,
/// returning `rank`'s distributed slice.
fn ring_graph(total: usize, rank: usize, nranks: usize) -> DistGraph {
    assert!(total >= 3, "a ring needs at least three vertices");
    let mut vtxdist = Vec::with_capacity(nranks + 1);
    vtxdist.push(0_i64);
    for r in 0..nranks {
        let count = total / nranks + usize::from(r < total % nranks);
        vtxdist.push(vtxdist[r] + count as i64);
    }
    let begin = vtxdist[rank] as usize;
    let end = vtxdist[rank + 1] as usize;
    let mut xadj = Vec::with_capacity(end - begin + 1);
    let mut adjncy = Vec::with_capacity(2 * (end - begin));
    xadj.push(0_i64);
    for v in begin..end {
        let prev = ((v + total - 1) % total) as i64;
        let next = ((v + 1) % total) as i64;
        adjncy.push(prev.min(next));
        adjncy.push(prev.max(next));
        xadj.push(adjncy.len() as i64);
    }
    DistGraph {
        vtxdist,
        xadj,
        adjncy,
        vwgt: None,
    }
}

#[test]
fn block_partitions_ring_graph_on_one_to_four_thread_ranks() {
    for total in [12_usize, 17] {
        let serial_graph = ring_graph(total, 0, 1);
        for nparts in 1..=4 {
            let expected = Block.partition(&Serial, &serial_graph, nparts).unwrap();
            for nranks in 1..=4 {
                let per_rank = ThreadComm::run(nranks, |comm| {
                    let g = ring_graph(total, comm.rank(), comm.size());
                    let parts = Block.partition(&comm, &g, nparts).unwrap();
                    let begin = g.vtxdist[comm.rank()] as usize;
                    let end = g.vtxdist[comm.rank() + 1] as usize;
                    assert_eq!(parts.len(), end - begin);
                    parts
                });
                let gathered: Vec<i32> = per_rank.into_iter().flatten().collect();
                assert_eq!(
                    gathered, expected,
                    "total={total}, nparts={nparts}, nranks={nranks}"
                );
            }
        }
    }
}

#[test]
fn block_agrees_on_invalid_ring_partition_across_ranks() {
    for nranks in 2..=4 {
        ThreadComm::run(nranks, |comm| {
            let g = ring_graph(12, comm.rank(), comm.size());
            let nparts = if comm.rank() == 0 { 0 } else { nranks };
            let err = Block.partition(&comm, &g, nparts).unwrap_err();
            if comm.rank() == 0 {
                assert!(
                    matches!(
                        err,
                        PartitionError::LengthMismatch {
                            expected: 1,
                            found: 0
                        }
                    ),
                    "rank 0: unexpected {err}"
                );
            } else {
                assert!(
                    matches!(err, PartitionError::RemoteFailure { rank: 0 }),
                    "rank {}: unexpected {err}",
                    comm.rank()
                );
            }
        });
    }
}

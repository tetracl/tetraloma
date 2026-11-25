//! Stages 2 and 3: the facet dual graph of the input cells, and its partition
//! (docs/design.md, "Parallel creation pipeline"; ADR-0026, item 2).

use std::collections::HashMap;

use tetraloma_device::{GridError, Result};
use tetraloma_parallel::{CommDyn, CommExt};
use tetraloma_partition::{DistGraph, PartitionError, Partitioner};

use super::rendezvous::{Outbox, rendezvous_rank};
use super::{BlockSpec, Cells, agree, facet_keys};

/// Stage 2: the dual graph of `cells`, whose vertices are the cells in rank order and whose
/// edges join cells that share a facet. Facet keys (sorted vertex origin ids) meet at their
/// rendezvous rank, which sends every pair of cells with the same key to the first cell's
/// rank. The weight of a vertex is the Lagrange node count of its cell's type and degree, the
/// cost model of ADR-0029, item 2. Collective.
///
/// # Errors
///
/// Errors of the exchanges.
pub(crate) fn dual_graph(
    comm: &dyn CommDyn,
    blocks: &[BlockSpec],
    cells: &Cells,
) -> Result<DistGraph> {
    let p = comm.size();
    let mut vtxdist = vec![0_i64];
    for n in comm.all_gather(cells.len() as i64) {
        vtxdist.push(vtxdist.last().expect("non-empty") + n);
    }
    let first = vtxdist[comm.rank()];

    let mut out = Outbox::new(p);
    for i in 0..cells.len() {
        for key in facet_keys(cells, i, blocks) {
            out.push(rendezvous_rank(&key, p), &[&[first + i as i64], &key]);
        }
    }
    let inbox = out.exchange(comm)?;

    // Every pair of cells with one key; the order of the pairs does not matter, as each row
    // is sorted below.
    let mut by_key: HashMap<&[i64], Vec<i64>> = HashMap::new();
    for (_, r) in inbox.records() {
        by_key.entry(&r[1..]).or_default().push(r[0]);
    }
    let rank_of = |g: i64| vtxdist.partition_point(|&x| x <= g) - 1;
    let mut pairs = Outbox::new(p);
    for group in by_key.values().filter(|g| g.len() > 1) {
        for &a in group {
            for &b in group.iter().filter(|&&b| b != a) {
                pairs.push(rank_of(a), &[&[a, b]]);
            }
        }
    }
    let inbox = pairs.exchange(comm)?;

    let mut rows: Vec<Vec<i64>> = vec![Vec::new(); cells.len()];
    for (_, r) in inbox.records() {
        rows[(r[0] - first) as usize].push(r[1]);
    }
    let mut xadj = vec![0_i64];
    let mut adjncy = Vec::new();
    for mut row in rows {
        row.sort_unstable();
        row.dedup();
        adjncy.extend_from_slice(&row);
        xadj.push(adjncy.len() as i64);
    }
    let vwgt = cells
        .block
        .iter()
        .map(|&b| blocks[b as usize].num_nodes() as i64)
        .collect();
    Ok(DistGraph {
        vtxdist,
        xadj,
        adjncy,
        vwgt: Some(vwgt),
    })
}

/// Stage 2 for `Block`, which reads only the vertex distribution: the graph of `n` local cells
/// without edges or weights. Collective.
pub(crate) fn vertices_only(comm: &dyn CommDyn, n: usize) -> DistGraph {
    let mut vtxdist = vec![0_i64];
    for m in comm.all_gather(n as i64) {
        vtxdist.push(vtxdist.last().expect("non-empty") + m);
    }
    DistGraph {
        vtxdist,
        xadj: vec![0; n + 1],
        adjncy: Vec::new(),
        vwgt: None,
    }
}

/// Stage 3: the destination rank of every local cell, from `partitioner` with one part per
/// rank. Collective.
///
/// # Errors
///
/// The partitioner's errors; `LengthMismatch` unless there is one destination per local graph
/// vertex, and `IndexOutOfRange` for a destination outside the ranks; agreed.
pub(crate) fn partition(
    comm: &dyn CommDyn,
    partitioner: &dyn Partitioner,
    graph: &DistGraph,
) -> Result<Vec<i32>> {
    let p = comm.size();
    let n = graph.xadj.len() - 1;
    let local = partitioner
        .partition(comm, graph, p)
        .map_err(partition_error)
        .and_then(|dest| check_destinations(&dest, n, p).map(|()| dest));
    agree(comm, local)
}

/// Converts a [`PartitionError`] into its corresponding [`GridError`] variant.
///
/// Crate-private because `tetraloma-topology` does not depend on `tetraloma-partition`.
// Design: "Partitioning", Listing S3; ADR-0074, ADR-0075.
pub(crate) fn partition_error(error: PartitionError) -> GridError {
    match error {
        PartitionError::PartitionerFailed { name, code } => {
            GridError::PartitionerFailed { name, code }
        }
        PartitionError::NeedsMpiCommunicator(name) => GridError::NeedsMpiCommunicator(name),
        PartitionError::TooLarge => GridError::TooLarge,
        PartitionError::LengthMismatch { expected, found } => {
            GridError::LengthMismatch { expected, found }
        }
        PartitionError::IndexOutOfRange { index, len } => GridError::IndexOutOfRange { index, len },
        PartitionError::MessageTooLarge { count } => GridError::MessageTooLarge { count },
        PartitionError::RemoteFailure { rank } => GridError::RemoteFailure { rank },
        // Required by `#[non_exhaustive]` across crates; adding a variant breaks
        // `tetraloma_partition::error::tests::partition_error_variant_list_is_exhaustive` in
        // `tetraloma-partition`'s test build (it does not break this library build).
        _ => unreachable!("non-exhaustive PartitionError variant"),
    }
}

/// `LengthMismatch` unless `dest` has `n` entries, `IndexOutOfRange` for an entry outside
/// `0..p`.
pub(crate) fn check_destinations(dest: &[i32], n: usize, p: usize) -> Result<()> {
    if dest.len() != n {
        return Err(GridError::LengthMismatch {
            expected: n,
            found: dest.len(),
        });
    }
    let in_range = |d: i32| usize::try_from(d).is_ok_and(|d| d < p);
    if let Some(&bad) = dest.iter().find(|&&d| !in_range(d)) {
        return Err(GridError::IndexOutOfRange {
            index: usize::try_from(bad).unwrap_or(usize::MAX),
            len: p,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::input::read;
    use crate::pipeline::testing::{OwnedInput, host_adjacency, host_cells, host_facets, meshes};
    use std::collections::BTreeMap;
    use tetraloma_partition::Block;
    use tetraloma_testing::ThreadComm;

    /// The graph equals the serial facet graph of the whole mesh, in cell origin ids, and the
    /// weight of every cell is its node count (ADR-0029, item 2).
    #[test]
    fn graph_equals_the_facet_graph() {
        for (name, h) in meshes() {
            let nodes = host_cells(&h);
            let want: BTreeMap<i64, (Vec<i64>, i64)> = host_adjacency(&host_facets(&h))
                .into_iter()
                .map(|(id, adj)| (id, (adj, nodes[&id].1.len() as i64)))
                .collect();
            for size in 1..=4 {
                let parts = ThreadComm::run(size, |comm| {
                    let input = OwnedInput::of_chunk(&h.chunk(comm.rank(), size));
                    let read = read(&comm, &input.input()).unwrap();
                    let g = dual_graph(&comm, &read.blocks, &read.cells).unwrap();
                    assert_eq!(g.vtxdist.len(), size + 1);
                    assert_eq!(g.xadj.len(), read.cells.len() + 1);
                    (read.cells.ids, g)
                });
                // Graph vertex → cell origin id.
                let ids: Vec<i64> = parts.iter().flat_map(|(ids, _)| ids.clone()).collect();
                let mut got = BTreeMap::new();
                for (cells, g) in &parts {
                    let vwgt = g.vwgt.as_ref().expect("weights");
                    assert_eq!(vwgt.len(), cells.len());
                    for (i, &id) in cells.iter().enumerate() {
                        let row = &g.adjncy[g.xadj[i] as usize..g.xadj[i + 1] as usize];
                        let mut adj: Vec<i64> = row.iter().map(|&x| ids[x as usize]).collect();
                        adj.sort_unstable();
                        got.insert(id, (adj, vwgt[i]));
                    }
                }
                assert_eq!(got, want, "{name} on {size} ranks");
            }
        }
    }

    /// A partitioner that returns too few destinations, or one out of range, gives an agreed
    /// error.
    #[test]
    fn bad_destinations_are_agreed_errors() {
        struct Bad(i32);
        impl Partitioner for Bad {
            fn name(&self) -> &str {
                "bad"
            }
            fn partition(
                &self,
                comm: &dyn CommDyn,
                g: &DistGraph,
                _: usize,
            ) -> tetraloma_partition::Result<Vec<i32>> {
                let n = g.xadj.len() - 1;
                Ok(match (comm.rank(), self.0) {
                    (1, -1) => vec![0; n + 1],
                    (1, d) => vec![d; n],
                    _ => vec![0; n],
                })
            }
        }
        for (bad, want_local) in [(-1, "length"), (3, "range"), (-4, "range")] {
            let out = ThreadComm::run(3, |comm| {
                let g = DistGraph {
                    vtxdist: vec![0, 1, 2, 3],
                    xadj: vec![0, 0],
                    adjncy: vec![],
                    vwgt: None,
                };
                partition(&comm, &Bad(bad), &g)
            });
            assert!(matches!(out[0], Err(GridError::RemoteFailure { rank: 1 })));
            assert!(matches!(out[2], Err(GridError::RemoteFailure { rank: 1 })));
            match (want_local, &out[1]) {
                (
                    "length",
                    Err(GridError::LengthMismatch {
                        expected: 1,
                        found: 2,
                    }),
                ) => {}
                ("range", Err(GridError::IndexOutOfRange { len: 3, .. })) => {}
                (_, other) => panic!("{bad}: {other:?}"),
            }
        }
        let ok = ThreadComm::run(2, |comm| {
            let g = DistGraph {
                vtxdist: vec![0, 2, 4],
                xadj: vec![0, 0, 0],
                adjncy: vec![],
                vwgt: None,
            };
            partition(&comm, &Block, &g).unwrap()
        });
        assert_eq!(ok, [vec![0, 0], vec![1, 1]]);
    }

    #[test]
    fn partition_error_maps_every_variant_and_pipeline_agrees() {
        assert!(matches!(
            partition_error(PartitionError::PartitionerFailed {
                name: "metis",
                code: -41,
            }),
            GridError::PartitionerFailed {
                name: "metis",
                code: -41,
            }
        ));
        assert!(matches!(
            partition_error(PartitionError::NeedsMpiCommunicator("ptscotch")),
            GridError::NeedsMpiCommunicator("ptscotch")
        ));
        assert!(matches!(
            partition_error(PartitionError::TooLarge),
            GridError::TooLarge
        ));
        assert!(matches!(
            partition_error(PartitionError::LengthMismatch {
                expected: 42,
                found: 43,
            }),
            GridError::LengthMismatch {
                expected: 42,
                found: 43,
            }
        ));
        assert!(matches!(
            partition_error(PartitionError::IndexOutOfRange { index: 44, len: 45 }),
            GridError::IndexOutOfRange { index: 44, len: 45 }
        ));
        assert!(matches!(
            partition_error(PartitionError::MessageTooLarge { count: 46 }),
            GridError::MessageTooLarge { count: 46 }
        ));
        assert!(matches!(
            partition_error(PartitionError::RemoteFailure { rank: 47 }),
            GridError::RemoteFailure { rank: 47 }
        ));

        struct Failing;
        impl Partitioner for Failing {
            fn name(&self) -> &str {
                "failing"
            }
            fn partition(
                &self,
                comm: &dyn CommDyn,
                g: &DistGraph,
                _: usize,
            ) -> tetraloma_partition::Result<Vec<i32>> {
                if comm.rank() == 1 {
                    Err(PartitionError::PartitionerFailed {
                        name: "failing",
                        code: -9,
                    })
                } else {
                    Ok(vec![0; g.xadj.len() - 1])
                }
            }
        }
        let out = ThreadComm::run(2, |comm| {
            let g = DistGraph {
                vtxdist: vec![0, 1, 2],
                xadj: vec![0, 0],
                adjncy: vec![],
                vwgt: None,
            };
            partition(&comm, &Failing, &g)
        });
        assert!(matches!(out[0], Err(GridError::RemoteFailure { rank: 1 })));
        assert!(matches!(
            out[1],
            Err(GridError::PartitionerFailed {
                name: "failing",
                code: -9,
            })
        ));
    }

    /// M18 A4: every variant of `CommError`, `DeviceError`, `ParallelError` (through `From`) and
    /// `PartitionError` (through `partition_error`) converts into the exact `GridError` variant
    /// and fields produced before the milestone.
    #[test]
    fn foundation_errors_convert_to_their_grid_error_variants() {
        use cubecl::server::{Collective, ServerError};
        use tetraloma_device::DeviceError;
        use tetraloma_parallel::{CommError, ParallelError};

        macro_rules! check {
            ($actual:expr, $expected:pat) => {
                let actual = $actual;
                assert!(matches!(actual, $expected), "got {actual:?}");
            };
        }

        // CommError (5 variants).
        check!(GridError::from(CommError::TooLarge), GridError::TooLarge);
        check!(
            GridError::from(CommError::LengthMismatch {
                expected: 11,
                found: 12
            }),
            GridError::LengthMismatch {
                expected: 11,
                found: 12
            }
        );
        check!(
            GridError::from(CommError::IndexOutOfRange { index: 13, len: 14 }),
            GridError::IndexOutOfRange { index: 13, len: 14 }
        );
        check!(
            GridError::from(CommError::MessageTooLarge { count: 15 }),
            GridError::MessageTooLarge { count: 15 }
        );
        check!(
            GridError::from(CommError::RemoteFailure { rank: 16 }),
            GridError::RemoteFailure { rank: 16 }
        );

        // DeviceError (5 variants).
        check!(GridError::from(DeviceError::TooLarge), GridError::TooLarge);
        check!(
            GridError::from(DeviceError::AllocationTooLarge { bytes: 21, max: 22 }),
            GridError::AllocationTooLarge { bytes: 21, max: 22 }
        );
        check!(
            GridError::from(DeviceError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-a"
            }),
            GridError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-a"
            }
        );
        check!(
            GridError::from(DeviceError::LengthMismatch {
                expected: 23,
                found: 24
            }),
            GridError::LengthMismatch {
                expected: 23,
                found: 24
            }
        );
        check!(
            GridError::from(DeviceError::Server(ServerError::no_device_transport(
                Collective::AllReduce,
            ))),
            GridError::Server(ServerError::NoDeviceTransport {
                operation: Collective::AllReduce,
                ..
            })
        );

        // ParallelError (8 variants).
        check!(
            GridError::from(ParallelError::TooLarge),
            GridError::TooLarge
        );
        check!(
            GridError::from(ParallelError::AllocationTooLarge { bytes: 31, max: 32 }),
            GridError::AllocationTooLarge { bytes: 31, max: 32 }
        );
        check!(
            GridError::from(ParallelError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-b"
            }),
            GridError::UnsupportedPrecision {
                scalar: "f64",
                device: "device-b"
            }
        );
        check!(
            GridError::from(ParallelError::LengthMismatch {
                expected: 33,
                found: 34
            }),
            GridError::LengthMismatch {
                expected: 33,
                found: 34
            }
        );
        check!(
            GridError::from(ParallelError::Server(ServerError::no_device_transport(
                Collective::Send,
            ))),
            GridError::Server(ServerError::NoDeviceTransport {
                operation: Collective::Send,
                ..
            })
        );
        check!(
            GridError::from(ParallelError::IndexOutOfRange { index: 35, len: 36 }),
            GridError::IndexOutOfRange { index: 35, len: 36 }
        );
        check!(
            GridError::from(ParallelError::MessageTooLarge { count: 37 }),
            GridError::MessageTooLarge { count: 37 }
        );
        check!(
            GridError::from(ParallelError::RemoteFailure { rank: 38 }),
            GridError::RemoteFailure { rank: 38 }
        );

        // PartitionError (7 variants).
        check!(
            partition_error(PartitionError::PartitionerFailed {
                name: "metis",
                code: -41
            }),
            GridError::PartitionerFailed {
                name: "metis",
                code: -41
            }
        );
        check!(
            partition_error(PartitionError::NeedsMpiCommunicator("ptscotch")),
            GridError::NeedsMpiCommunicator("ptscotch")
        );
        check!(
            partition_error(PartitionError::TooLarge),
            GridError::TooLarge
        );
        check!(
            partition_error(PartitionError::LengthMismatch {
                expected: 42,
                found: 43
            }),
            GridError::LengthMismatch {
                expected: 42,
                found: 43
            }
        );
        check!(
            partition_error(PartitionError::IndexOutOfRange { index: 44, len: 45 }),
            GridError::IndexOutOfRange { index: 44, len: 45 }
        );
        check!(
            partition_error(PartitionError::MessageTooLarge { count: 46 }),
            GridError::MessageTooLarge { count: 46 }
        );
        check!(
            partition_error(PartitionError::RemoteFailure { rank: 47 }),
            GridError::RemoteFailure { rank: 47 }
        );
    }
}

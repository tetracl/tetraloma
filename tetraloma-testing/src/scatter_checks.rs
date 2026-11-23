//! Shared checks of [`Scatterer::forward`](tetraloma::Scatterer::forward) and
//! [`Scatterer::reverse`](tetraloma::Scatterer::reverse) against a host reference on random ghost
//! layouts (docs/plans/M18.md, T1; ADR-0075).
//!
//! `tetraloma/tests/scatter_threads.rs` and `tetraloma-foundation-tests` call these checks on
//! [`ThreadComm`](crate::ThreadComm), and the `foundation_scatter` target of `tetraloma-mpi-tests`
//! calls them on `Mpi`. Every check function is collective: every rank of `comm` calls it.

use std::fmt::Debug;

use tetraloma::{CommDyn, DeviceBuffer, GridError, IndexMap, ReverseOp, ScatterNumeric, Transport};
use tetraloma_device::Client;

use crate::comm_checks::ghost_layout_sized;

/// Default seeds of the scatter acceptance checks (M18 A7 and A8).
pub const SEEDS: [u64; 3] = [1, 2, 3];

/// The scalars under test, with the host arithmetic of the reference.
pub trait Val: ScatterNumeric + PartialEq + Debug {
    /// Converts a small host integer to this scalar.
    fn of(v: i64) -> Self;
    /// Host addition matching `ReverseOp::Add`.
    fn plus(self, o: Self) -> Self;
    /// Strict ordering matching `ReverseOp::Min` and `ReverseOp::Max`.
    fn below(&self, o: &Self) -> bool;
}

macro_rules! val {
    ($($t:ty),*) => {$(
        impl Val for $t {
            fn of(v: i64) -> Self {
                v as $t
            }
            fn plus(self, o: Self) -> Self {
                self + o
            }
            fn below(&self, o: &Self) -> bool {
                self < o
            }
        }
    )*};
}
val!(f32, f64, i32, u32);

/// One rank's [`IndexMap`] of the random ghost layout of a seed, together with the global layout.
pub struct Layout {
    /// This rank's index map.
    pub map: IndexMap,
    /// Number of owned indices per rank.
    pub owned: Vec<usize>,
    /// Per-rank ghost lists as `(global_index, owner_rank)` in ghost order.
    pub ghosts: Vec<Vec<(i64, i32)>>,
}

impl Layout {
    /// Builds the deterministic ghost layout of `seed` (at most `max_owned` owned indices per
    /// rank) and constructs this rank's [`IndexMap`].
    pub fn new(comm: &dyn CommDyn, seed: u64, max_owned: u64) -> Layout {
        let (owned, ghosts) = ghost_layout_sized(comm.size(), seed, max_owned);
        let mine = &ghosts[comm.rank()];
        let map = IndexMap::with_ghosts(
            comm,
            owned[comm.rank()],
            mine.iter().map(|g| g.0).collect(),
            mine.iter().map(|g| g.1).collect(),
        )
        .unwrap();
        Layout { map, owned, ghosts }
    }
}

/// The value the owner of global index `g` holds in component `c`.
fn owner_value(g: i64, c: usize) -> i64 {
    3 * g + c as i64 + 1
}

/// The value rank `s` holds in component `c` of its ghost of `g`.
fn ghost_value(g: i64, s: usize, c: usize) -> i64 {
    5 * g + 7 * s as i64 + c as i64 + 2
}

/// This rank's input: owner values on owned indices, ghost values on ghosts.
pub fn input<T: Val>(comm: &dyn CommDyn, l: &Layout, block: usize) -> Vec<T> {
    let r = comm.rank();
    let first = l.map.owned_range().0;
    let mut x = Vec::with_capacity(l.map.num_local() * block);
    for i in 0..l.owned[r] {
        x.extend((0..block).map(|c| T::of(owner_value(first + i as i64, c))));
    }
    for &(g, _) in &l.ghosts[r] {
        x.extend((0..block).map(|c| T::of(ghost_value(g, r, c))));
    }
    x
}

/// Checks `Scatterer::forward` and every [`ReverseOp`] against the host reference for scalar `T`
/// and block size `block`.
pub fn check<T: Val>(comm: &dyn CommDyn, client: &Client, l: &Layout, block: usize) {
    let (r, n) = (comm.rank(), comm.size());
    let at = format!(
        "rank {r} of {n}, block {block}, {}",
        std::any::type_name::<T>()
    );
    let first = l.map.owned_range().0;
    let num_owned = l.owned[r];
    let x0 = input::<T>(comm, l, block);
    let scatterer = l.map.scatterer(client, block).unwrap();
    assert_eq!(scatterer.transport(), Transport::HostStaged);

    // Forward: ghosts take their owner's values.
    let mut x = DeviceBuffer::from_host(client, &x0).unwrap();
    scatterer.forward(&mut x).unwrap();
    let mut want = x0.clone();
    for (k, &(g, _)) in l.ghosts[r].iter().enumerate() {
        for c in 0..block {
            want[(num_owned + k) * block + c] = T::of(owner_value(g, c));
        }
    }
    assert_eq!(x.to_host().unwrap(), want, "{at}: forward");

    // Reverse: contributions by ascending neighbour rank (each rank ghosts an index once).
    for op in [
        ReverseOp::Add,
        ReverseOp::Min,
        ReverseOp::Max,
        ReverseOp::Insert,
    ] {
        let mut x = DeviceBuffer::from_host(client, &x0).unwrap();
        scatterer.reverse(&mut x, op).unwrap();
        let mut want = x0.clone();
        for i in 0..num_owned {
            let g = first + i as i64;
            let holders: Vec<usize> = (0..n)
                .filter(|&s| l.ghosts[s].iter().any(|&(h, _)| h == g))
                .collect();
            if holders.is_empty() {
                continue;
            }
            for c in 0..block {
                let own = T::of(owner_value(g, c));
                let mut contributions = holders.iter().map(|&s| T::of(ghost_value(g, s, c)));
                want[i * block + c] = match op {
                    ReverseOp::Add => contributions.fold(own, T::plus),
                    ReverseOp::Min => {
                        contributions.fold(own, |a, b| if b.below(&a) { b } else { a })
                    }
                    ReverseOp::Max => {
                        contributions.fold(own, |a, b| if a.below(&b) { b } else { a })
                    }
                    ReverseOp::Insert => contributions.next().unwrap(),
                };
            }
        }
        assert_eq!(x.to_host().unwrap(), want, "{at}: reverse {op:?}");
    }
}

/// Runs [`check`] for `f32`, `i32`, `u32` and `f64` on the test device. On devices without `f64`,
/// asserts that `forward` still succeeds (it moves `u32` words) and `reverse` returns
/// `UnsupportedPrecision`.
pub fn check_all_types(comm: &dyn CommDyn, l: &Layout, block: usize) {
    let client = crate::test_client();
    check::<f32>(comm, &client, l, block);
    check::<i32>(comm, &client, l, block);
    check::<u32>(comm, &client, l, block);
    if crate::supports::<f64>(&client) {
        check::<f64>(comm, &client, l, block);
    } else {
        let s = l.map.scatterer(&client, block).unwrap();
        let mut x = DeviceBuffer::from_host(&client, &input::<f64>(comm, l, block)).unwrap();
        // f64 moves as u32 words, so forward works on every device.
        s.forward(&mut x).unwrap();
        assert!(matches!(
            s.reverse(&mut x, ReverseOp::Add),
            Err(GridError::UnsupportedPrecision { scalar: "f64", .. })
        ));
    }
}

/// Checks `seed`'s ghost layout (up to 20 owned indices per rank) across every block size in
/// `blocks` and every supported scalar type.
pub fn check_seed(comm: &dyn CommDyn, seed: u64, blocks: &[usize]) {
    let layout = Layout::new(comm, seed, 20);
    for &block in blocks {
        check_all_types(comm, &layout, block);
    }
}

/// Runs [`check_seed`] for every seed in `seeds` and every block size in `blocks`.
pub fn check_seeds(comm: &dyn CommDyn, seeds: &[u64], blocks: &[usize]) {
    for &seed in seeds {
        check_seed(comm, seed, blocks);
    }
}

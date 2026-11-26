//! Checks of a communicator's `CommDyn` and `CommExt` methods (re-exported from
//! `tetraloma-comm-testing`), random ghost layouts, and `IndexMap` checks, shared by the
//! `ThreadComm` tests and the MPI target `comm` (docs/plans/M6.md, T2, T3 and T5).
//! Every check function is collective: every rank of `comm` calls it.

pub use tetraloma_comm_testing::comm_checks::{
    identities, matches_serial, neighbour_exchange, ring, wrong_counts,
};

use tetraloma_parallel::{CommDyn, CommExt, IndexMap, ParallelError};

/// The random index-map layout of `seed` on `n` ranks: per rank the owned count, and its ghosts
/// as (global index, owner) in ghost order. Every rank computes the same layout.
pub fn ghost_layout(n: usize, seed: u64) -> (Vec<usize>, Vec<Vec<(i64, i32)>>) {
    ghost_layout_sized(n, seed, 20)
}

/// [`ghost_layout`] with fewer than `max_owned` owned indices per rank.
pub fn ghost_layout_sized(
    n: usize,
    seed: u64,
    max_owned: u64,
) -> (Vec<usize>, Vec<Vec<(i64, i32)>>) {
    let mut state = seed;
    let mut next = move |m: u64| {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) % m.max(1)) as usize
    };
    let owned: Vec<usize> = (0..n).map(|_| next(max_owned)).collect();
    let first: Vec<usize> = owned
        .iter()
        .scan(0, |at, &c| {
            let f = *at;
            *at += c;
            Some(f)
        })
        .collect();
    let ghosts = (0..n)
        .map(|r| {
            let mut g: Vec<(i64, i32)> = (0..n)
                .filter(|&s| s != r)
                .flat_map(|s| (0..owned[s]).map(move |i| (s, i)))
                .filter(|_| next(3) == 0)
                .map(|(s, i)| ((first[s] + i) as i64, s as i32))
                .collect();
            for i in (1..g.len()).rev() {
                g.swap(i, next(i as u64 + 1));
            }
            g
        })
        .collect();
    (owned, ghosts)
}

/// `IndexMap::with_ghosts` on the layout of `seed`: owned ranges, ghosts, the symmetry of the
/// forward neighbourhoods, and the send and receive lists against a host reference.
pub fn index_map_layout(comm: &dyn CommDyn, seed: u64) {
    let (r, n) = (comm.rank(), comm.size());
    let (owned, ghosts) = ghost_layout(n, seed);
    let first: i64 = owned[..r].iter().sum::<usize>() as i64;
    let mine = &ghosts[r];
    let map = IndexMap::with_ghosts(
        comm,
        owned[r],
        mine.iter().map(|g| g.0).collect(),
        mine.iter().map(|g| g.1).collect(),
    )
    .unwrap();
    let at = format!("rank {r} of {n}, seed {seed:#x}");
    assert_eq!(map.owned_range(), (first, first + owned[r] as i64), "{at}");
    assert_eq!(
        map.size_global(),
        owned.iter().sum::<usize>() as i64,
        "{at}"
    );
    assert_eq!(map.num_local(), owned[r] + mine.len(), "{at}");
    assert_eq!(
        map.ghosts(),
        mine.iter().map(|g| g.0).collect::<Vec<_>>(),
        "{at}"
    );

    // Send lists: for each rank s ghosting this rank's indices, in ascending s, the local
    // indices in s's ghost order.
    let (mut dsts, mut counts, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for (s, theirs) in ghosts.iter().enumerate() {
        let want: Vec<u32> = theirs
            .iter()
            .filter(|g| g.1 as usize == r)
            .map(|g| (g.0 - first) as u32)
            .collect();
        if !want.is_empty() {
            dsts.push(s);
            counts.push(want.len());
            idx.extend(want);
        }
    }
    let (d, c, i) = map.send_lists();
    assert_eq!(
        (d, c, i),
        (&dsts[..], &counts[..], &idx[..]),
        "{at}: send lists"
    );

    // Receive lists: owners in ascending order, ghost positions ascending within each.
    let (mut srcs, mut counts, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for s in 0..n {
        let want: Vec<u32> = (0..mine.len())
            .filter(|&k| mine[k].1 as usize == s)
            .map(|k| (owned[r] + k) as u32)
            .collect();
        if !want.is_empty() {
            srcs.push(s);
            counts.push(want.len());
            idx.extend(want);
        }
    }
    let (s, c, i) = map.recv_lists();
    assert_eq!(
        (s, c, i),
        (&srcs[..], &counts[..], &idx[..]),
        "{at}: receive lists"
    );

    // Symmetry: s sends to r exactly when r receives from s.
    let sends: Vec<u64> = (0..n)
        .map(|s| u64::from(map.send_lists().0.contains(&s)))
        .collect();
    let (all, _) = comm.all_to_all_v(&sends, &vec![1; n]).unwrap();
    for (s, &sent) in all.iter().enumerate() {
        assert_eq!(
            sent == 1,
            map.recv_lists().0.contains(&s),
            "{at}: symmetry with {s}"
        );
    }

    // A clone shares the pattern.
    let copy = map.clone();
    assert_eq!(copy.send_lists(), map.send_lists());
    assert_eq!(copy.ghost_owners(), map.ghost_owners());
}

/// `with_ghosts` without ghosts equals `new`, and a request for an index the owner does not
/// own is an agreed `IndexOutOfRange` on the owner. Needs at least two ranks for the error.
pub fn index_map_basics(comm: &dyn CommDyn) {
    let (r, n) = (comm.rank(), comm.size());
    let a = IndexMap::new(comm, r + 2).unwrap();
    let b = IndexMap::with_ghosts(comm, r + 2, vec![], vec![]).unwrap();
    assert_eq!(
        (a.owned_range(), a.size_global(), a.num_ghosts()),
        (b.owned_range(), b.size_global(), b.num_ghosts())
    );
    assert!(b.send_lists().0.is_empty() && b.recv_lists().0.is_empty());
    if n < 2 {
        return;
    }
    // Rank 0 asks rank 1 for an index far past the global size.
    let (ghosts, owners) = if r == 0 {
        (vec![1000], vec![1])
    } else {
        (vec![], vec![])
    };
    match IndexMap::with_ghosts(comm, 1, ghosts, owners) {
        Err(ParallelError::IndexOutOfRange { index: 1000, len }) if r == 1 => assert_eq!(len, n),
        Err(ParallelError::RemoteFailure { rank: 1 }) if r != 1 => {}
        Err(e) => panic!("rank {r}: unexpected {e}"),
        Ok(_) => panic!("rank {r}: an unowned index was accepted"),
    }
}

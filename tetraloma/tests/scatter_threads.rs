//! `Scatterer` on `ThreadComm` with 1–4 ranks (docs/plans/M6.md, T7): forward and every
//! `ReverseOp` against a host reference on random ghost layouts, the scatter-consistency
//! property, and bitwise repeatability.

use proptest::prelude::*;
use tetraloma::{CommDyn, DeviceBuffer, IndexMap, ParallelError, ReverseOp};
use tetraloma_testing::ThreadComm;
use tetraloma_testing::scatter_checks::{self, Layout, input};

#[test]
fn forward_and_reverse_match_the_host_reference() {
    for n in 1..=4 {
        for seed in scatter_checks::SEEDS {
            ThreadComm::run(n, |comm| {
                scatter_checks::check_seed(&comm, seed, &[1, 3]);
            });
        }
    }
}

#[test]
fn maps_without_ghosts_and_bad_inputs() {
    ThreadComm::run(2, |comm| {
        let client = tetraloma_testing::test_client();
        // IndexMap::new has no exchange; with_ghosts without ghosts has an empty one.
        for map in [
            IndexMap::new(&comm, 3).unwrap(),
            IndexMap::with_ghosts(&comm, 3, vec![], vec![]).unwrap(),
        ] {
            let s = map.scatterer(&client, 2).unwrap();
            let host: Vec<i32> = (0..6).collect();
            let mut x = DeviceBuffer::from_host(&client, &host).unwrap();
            s.forward(&mut x).unwrap();
            s.reverse(&mut x, ReverseOp::Add).unwrap();
            assert_eq!(x.to_host().unwrap(), host);
            let mut short = DeviceBuffer::from_host(&client, &host[..5]).unwrap();
            assert!(matches!(
                s.forward(&mut short),
                Err(ParallelError::LengthMismatch {
                    expected: 6,
                    found: 5
                })
            ));
            assert!(matches!(
                map.scatterer(&client, 0),
                Err(ParallelError::LengthMismatch { .. })
            ));
        }
    });
}

#[test]
fn clones_and_second_scatterers_agree() {
    ThreadComm::run(3, |comm| {
        let client = tetraloma_testing::test_client();
        let l = Layout::new(&comm, 7, 20);
        let x0 = input::<i32>(&comm, &l, 2);
        let run = |map: &IndexMap| {
            let mut x = DeviceBuffer::from_host(&client, &x0).unwrap();
            map.scatterer(&client, 2)
                .unwrap()
                .reverse(&mut x, ReverseOp::Add)
                .unwrap();
            x.to_host().unwrap()
        };
        // The first scatterer fills the device cache, the second reuses it, the clone has none.
        let a = run(&l.map);
        assert_eq!(run(&l.map), a);
        assert_eq!(run(&l.map.clone()), a);
    });
}

#[test]
fn two_runs_are_bitwise_equal() {
    for n in 1..=4 {
        ThreadComm::run(n, |comm| {
            let client = tetraloma_testing::test_client();
            let l = Layout::new(&comm, 11, 200);
            // Values whose sums round, so a change of order would show.
            let x0: Vec<f32> = (0..l.map.num_local() * 3)
                .map(|i| 0.1 + (i as f32).sqrt() * 1e-3 * (comm.rank() + 1) as f32)
                .collect();
            let run = || {
                let mut x = DeviceBuffer::from_host(&client, &x0).unwrap();
                let s = l.map.scatterer(&client, 3).unwrap();
                s.reverse(&mut x, ReverseOp::Add).unwrap();
                s.forward(&mut x).unwrap();
                x.to_host().unwrap()
            };
            let bits = |v: Vec<f32>| v.into_iter().map(f32::to_bits).collect::<Vec<_>>();
            assert_eq!(bits(run()), bits(run()));
        });
    }
}

/// One scatterer reused across calls and scalar types (its buffers are kept per type,
/// ADR-0059): repeated forward and reverse calls on the same input give the bitwise result of
/// a fresh scatterer, with f64 calls (where supported) and `Insert` in between.
#[test]
fn repeated_calls_on_one_scatterer_are_bitwise_equal() {
    for n in 1..=4 {
        ThreadComm::run(n, |comm| {
            let client = tetraloma_testing::test_client();
            let l = Layout::new(&comm, 23, 300);
            let x0: Vec<f32> = (0..l.map.num_local() * 3)
                .map(|i| 0.1 + (i as f32).sqrt() * 1e-3 * (comm.rank() + 1) as f32)
                .collect();
            let y0: Vec<f64> = x0.iter().map(|&v| f64::from(v) * 1.5).collect();
            let f64_ok = tetraloma_testing::supports::<f64>(&client);
            let bits = |v: Vec<f32>| v.into_iter().map(f32::to_bits).collect::<Vec<_>>();
            let run = |s: &tetraloma::Scatterer| {
                let mut x = DeviceBuffer::from_host(&client, &x0).unwrap();
                s.reverse(&mut x, ReverseOp::Add).unwrap();
                s.forward(&mut x).unwrap();
                s.reverse(&mut x, ReverseOp::Max).unwrap();
                bits(x.to_host().unwrap())
            };
            let other = |s: &tetraloma::Scatterer| {
                if f64_ok {
                    let mut y = DeviceBuffer::from_host(&client, &y0).unwrap();
                    s.reverse(&mut y, ReverseOp::Add).unwrap();
                    s.forward(&mut y).unwrap();
                }
                let mut x = DeviceBuffer::from_host(&client, &x0).unwrap();
                s.reverse(&mut x, ReverseOp::Insert).unwrap();
            };
            let want = run(&l.map.scatterer(&client, 3).unwrap());
            let s = l.map.scatterer(&client, 3).unwrap();
            for _ in 0..3 {
                assert_eq!(run(&s), want, "rank {} of {n}", comm.rank());
                other(&s);
            }
        });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// IndexMap scatter consistency: after a forward, every ghost equals its owner; a reverse
    /// `Add` of ones counts the holders of each owned index; a reverse `Insert` of the rank
    /// gives the lowest ghosting rank. Up to 10^5 indices.
    #[test]
    fn consistency(n in 1..=4_usize, seed in any::<u64>(), scale in 0..=4_u32, block in 1..=3_usize) {
        let max_owned = (10_u64.pow(scale) * 25 / n as u64).max(1);
        ThreadComm::run(n, |comm| {
            let client = tetraloma_testing::test_client();
            let l = Layout::new(&comm, seed, max_owned);
            let (r, local) = (comm.rank(), l.map.num_local());
            let first = l.map.owned_range().0;
            let s = l.map.scatterer(&client, block).unwrap();

            let ids: Vec<i32> = (0..local * block)
                .map(|e| if e / block < l.owned[r] { (first as usize * block + e) as i32 } else { -1 })
                .collect();
            let mut x = DeviceBuffer::from_host(&client, &ids).unwrap();
            s.forward(&mut x).unwrap();
            let x = x.to_host().unwrap();
            for (k, &(g, _)) in l.ghosts[r].iter().enumerate() {
                for c in 0..block {
                    assert_eq!(x[(l.owned[r] + k) * block + c], (g as usize * block + c) as i32);
                }
            }

            let mut ones = DeviceBuffer::from_host(&client, &vec![1_u32; local * block]).unwrap();
            s.reverse(&mut ones, ReverseOp::Add).unwrap();
            let mut ranks = DeviceBuffer::from_host(&client, &vec![r as u32; local * block]).unwrap();
            s.reverse(&mut ranks, ReverseOp::Insert).unwrap();
            let (ones, ranks) = (ones.to_host().unwrap(), ranks.to_host().unwrap());
            let mut holders = vec![Vec::new(); l.owned[r]];
            for (t, theirs) in l.ghosts.iter().enumerate() {
                for &(g, o) in theirs {
                    if o as usize == r {
                        holders[(g - first) as usize].push(t as u32);
                    }
                }
            }
            for e in 0..local * block {
                let i = e / block;
                let (count, lowest) = match holders.get(i) {
                    Some(h) => (1 + h.len() as u32, h.first().copied().unwrap_or(r as u32)),
                    None => (1, r as u32),
                };
                assert_eq!(ones[e], count, "rank {r}: Add at {e}");
                assert_eq!(ranks[e], lowest, "rank {r}: Insert at {e}");
            }
        });
    }
}

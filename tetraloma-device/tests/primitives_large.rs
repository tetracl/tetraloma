//! M2 acceptance: the primitives against CPU references at the fixed sizes 10^6 and 10^7.

use crate::common::LARGE_LENS;

use tetraloma_device::*;
use tetraloma_testing_base::primitive_checks::*;
use tetraloma_testing_base::test_client;

#[test]
fn fill_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_fill(&client, len);
    }
}

#[test]
fn copy_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_copy(&client, len);
    }
}

#[test]
fn reduce_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_reduce(&client, len);
    }
}

#[test]
fn segmented_reduce_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_segmented_reduce(&client, len);
    }
}

#[test]
fn scan_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_scan(&client, len);
    }
}

#[test]
fn gather_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_gather(&client, len);
    }
}

#[test]
fn scatter_large() {
    let client = test_client();
    for len in LARGE_LENS {
        check_scatter(&client, len);
    }
}

#[test]
fn histogram_large() {
    let client = test_client();
    let mut rng = SplitMix::new(LARGE_SEED);
    for len in LARGE_LENS {
        for num_bins in [256u32, 100_000] {
            let values_host: Vec<u32> = (0..len)
                .map(|_| rng.below(u64::from(num_bins) + 7) as u32)
                .collect();
            let got = histogram(&upload(&client, &values_host), num_bins).expect("histogram");
            let mut want = vec![0u32; num_bins as usize];
            for &v in &values_host {
                if let Some(c) = want.get_mut(v as usize) {
                    *c += 1;
                }
            }
            assert_eq!(
                got.to_host().expect("read"),
                want,
                "histogram {len} into {num_bins}"
            );
        }
    }
}

#[test]
fn sort_large() {
    let client = test_client();
    let mut rng = SplitMix::new(LARGE_SEED);
    for len in LARGE_LENS {
        check_radix_sort_with_rng(&client, len, &mut rng);
    }
}

#[test]
fn run_length_large() {
    let client = test_client();
    let mut rng = SplitMix::new(LARGE_SEED);
    for len in LARGE_LENS {
        let keys_host = run_keys(len, 2, &mut rng);
        let runs = run_length(&upload_keys(&client, &keys_host, 2)).expect("run_length");
        let (want_keys, want_offsets) = reference_runs(&keys_host, 2);
        assert!(
            runs.keys.to_host().expect("read") == want_keys,
            "run keys of {len}"
        );
        assert!(
            runs.offsets.to_host().expect("read") == want_offsets,
            "run offsets of {len}"
        );
    }
}

#[test]
fn unique_large() {
    let client = test_client();
    let mut rng = SplitMix::new(LARGE_SEED);
    for len in LARGE_LENS {
        let mut keys_host = random_keys(len, 1, 1 << 18, &mut rng);
        keys_host.sort_unstable();
        let payload_host: Vec<u32> = (0..len).map(|_| rng.next_u32()).collect();
        let unique = unique_by_key(
            &upload_keys(&client, &keys_host, 1),
            &upload(&client, &payload_host),
        )
        .expect("unique_by_key");
        let (want_keys, offsets) = reference_runs(&keys_host, 1);
        let want_first: Vec<u32> = offsets[..offsets.len() - 1]
            .iter()
            .map(|&o| payload_host[o as usize])
            .collect();
        let mut want_group = vec![0u32; len];
        for g in 0..offsets.len() - 1 {
            want_group[offsets[g] as usize..offsets[g + 1] as usize].fill(g as u32);
        }
        assert!(
            unique.keys.to_host().expect("read") == want_keys,
            "unique keys of {len}"
        );
        assert!(
            unique.first.to_host().expect("read") == want_first,
            "first of {len}"
        );
        assert!(
            unique.group.to_host().expect("read") == want_group,
            "group of {len}"
        );
    }
}

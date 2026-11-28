//! M13 acceptance A6 on `ThreadComm` (1–4 ranks); the MPI target lives in
//! `tetraloma-mpi-tests` (registered in M13.T4).

use tetraloma_testing::{ThreadComm, facet_rule};

#[test]
#[ignore = "pending M13.T4"]
fn interior_facet_points_match_with_rule_permutation() {
    for n in [1, 2, 3, 4] {
        ThreadComm::run(n, |comm| {
            let client = tetraloma_testing::test_client();
            facet_rule::check(&comm, &client);
        });
    }
}

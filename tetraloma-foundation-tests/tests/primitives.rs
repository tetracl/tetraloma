//! M18 A6: fill, copy, reduce, scan, radix sort, gather and segmented reduce on 10^6 elements
//! against host references, calling the shared checks of `tetraloma-testing-base`.

use tetraloma_device::Client;
use tetraloma_testing_base::primitive_checks::{
    check_copy, check_fill, check_gather, check_radix_sort, check_reduce, check_scan,
    check_segmented_reduce,
};
use tetraloma_testing_base::test_client;

const LEN: usize = 1_000_000;

#[test]
fn primitives_match_host_references_at_1e6() {
    let client: Client = test_client();
    check_fill(&client, LEN);
    check_copy(&client, LEN);
    check_reduce(&client, LEN);
    check_scan(&client, LEN);
    check_radix_sort(&client, LEN);
    check_gather(&client, LEN);
    check_segmented_reduce(&client, LEN);
}

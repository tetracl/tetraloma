//! M18 A7: `Scatterer::forward` and `Scatterer::reverse` with `Add`, `Min`, `Max` and `Insert`
//! against a host reference on 1–4 ranks of `ThreadComm`, for `f32`, `f64`, `i32` and `u32` and
//! block sizes 1–3, calling the shared checks of `tetraloma-testing-base`.

use tetraloma_comm_testing::ThreadComm;
use tetraloma_parallel::CommDyn;
use tetraloma_testing_base::scatter_checks;

#[test]
fn scatter_matches_host_reference_on_thread_ranks() {
    for n in 1..=4 {
        ThreadComm::run(n, |comm| {
            assert_eq!(comm.size(), n);
            scatter_checks::check_seeds(&comm, &scatter_checks::SEEDS, &[1, 2, 3]);
        });
    }
}

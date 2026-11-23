//! M18 A8: `Scatterer::forward` and `Scatterer::reverse` with `Add`, `Min`, `Max` and `Insert`
//! against a host reference on 1–4 MPI ranks, for `f32`, `f64`, `i32` and `u32` and block sizes
//! 1–3, calling the same check function as `tetraloma-foundation-tests` (A7).

use std::process::ExitCode;

use tetraloma_testing::scatter_checks;

fn main() -> ExitCode {
    tetraloma_mpi_tests::run("foundation_scatter", |comm| {
        scatter_checks::check_seeds(comm, &scatter_checks::SEEDS, &[1, 2, 3]);
    })
}

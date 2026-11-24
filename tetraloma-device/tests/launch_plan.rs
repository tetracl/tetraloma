//! M2 acceptance: "on the CPU runtime, `LaunchPlan` uses one unit per hardware thread and blocks
//! of whole 128-byte lines; with injected device properties, no plan exceeds
//! `max_units_per_cube`, `max_cube_dim` or `max_cube_count` (property test)". On CPUs the unit
//! count follows `VectorPolicy::cpu_units` (ADR-0017).

use crate::common;

use cubecl::ir::DeviceProperties;
use cubecl::prelude::*;
use proptest::prelude::*;
use tetraloma_device::testing::{
    with_cpu_block_bytes, with_cpu_units, with_performance_cores, with_properties, with_vector_size,
};
use tetraloma_device::{
    CpuUnits, DeviceBuffer, DeviceError, KernelKind, LaunchPlan, VECTOR_PAD, VectorPolicy,
    VectorSizeOverride,
};
use tetraloma_testing::primitive_checks::{SplitMix, TestScalar};
use tetraloma_testing::test_client;

const KINDS: [KernelKind; 13] = [
    KernelKind::Fill,
    KernelKind::Copy,
    KernelKind::Reduce,
    KernelKind::ScanTile,
    KernelKind::SortDigits,
    KernelKind::Gather,
    KernelKind::Pack,
    KernelKind::TopologyKeys,
    KernelKind::PermutationInfo,
    KernelKind::EVector,
    KernelKind::GeomFactors,
    KernelKind::PullBack,
    KernelKind::Scalar,
];

/// Cube count of a plan as (x, y, z).
fn cube_count(plan: &LaunchPlan) -> (u32, u32, u32) {
    match plan.cube_count {
        CubeCount::Static(x, y, z) => (x, y, z),
        CubeCount::Dynamic(_) => panic!("a plan has a dynamic cube count"),
    }
}

/// Units launched, times items_per_unit: the vectors the launch can cover.
fn covered(plan: &LaunchPlan) -> u128 {
    let (x, y, z) = cube_count(plan);
    [x, y, z, plan.cube_dim.num_elems(), plan.items_per_unit]
        .into_iter()
        .map(u128::from)
        .product()
}

/// The checks every plan must pass under the properties `props`.
fn check_plan(plan: &LaunchPlan, props: &DeviceProperties, items: usize, max_vector: usize) {
    let hw = &props.hardware;
    let (x, y, z) = cube_count(plan);
    let dim = plan.cube_dim;
    assert!(
        x >= 1 && y >= 1 && z >= 1,
        "empty cube count {:?}",
        (x, y, z)
    );
    assert!(
        x <= hw.max_cube_count.0 && y <= hw.max_cube_count.1 && z <= hw.max_cube_count.2,
        "cube count {:?} exceeds max_cube_count {:?}",
        (x, y, z),
        hw.max_cube_count
    );
    assert!(
        dim.x <= hw.max_cube_dim.0 && dim.y <= hw.max_cube_dim.1 && dim.z <= hw.max_cube_dim.2,
        "cube dim {dim:?} exceeds max_cube_dim {:?}",
        hw.max_cube_dim
    );
    assert!(
        dim.num_elems() >= 1 && dim.num_elems() <= hw.max_units_per_cube,
        "cube dim {dim:?} exceeds max_units_per_cube {}",
        hw.max_units_per_cube
    );
    let n = plan.vector_size;
    assert!(
        n.is_power_of_two() && VECTOR_PAD.is_multiple_of(n) && n <= max_vector,
        "vector size {n} with max_vector {max_vector}"
    );
    assert!(plan.items_per_unit >= 1, "items_per_unit is 0");
    let scalars = u128::from(plan.num_vectors) * n as u128;
    let padded = items.max(1).next_multiple_of(VECTOR_PAD) as u128;
    assert!(
        scalars >= items as u128 && scalars <= padded,
        "{} vectors of {n} for {items} items",
        plan.num_vectors
    );
    let covered = covered(plan);
    assert!(
        covered >= u128::from(plan.num_vectors),
        "the launch covers {covered} vectors, fewer than {}",
        plan.num_vectors
    );
    assert!(
        covered <= u128::from(u32::MAX),
        "cube count x cube_dim x items_per_unit = {covered} exceeds u32"
    );
}

#[test]
fn cpu_plan_one_unit_per_thread_whole_lines() {
    let client = test_client();
    let mut cases: Vec<(&str, DeviceProperties)> = Vec::new();
    let props = client.properties().clone();
    if props.hardware.num_cpu_cores.is_some() {
        println!(
            "CPU runtime: max_units_per_cube {}, available_parallelism {:?}",
            props.hardware.max_units_per_cube,
            std::thread::available_parallelism()
        );
        cases.push(("the CPU runtime", props.clone()));
    } else {
        println!(
            "{} is not the CPU runtime: injected CPU properties only",
            client.name()
        );
    }
    // Injected CPU properties, so the rule is also checked on GPU test devices.
    let mut cpu = props;
    cpu.hardware.num_cpu_cores = Some(12);
    cpu.hardware.max_units_per_cube = 12;
    cpu.hardware.max_cube_dim = (12, 12, 12);
    cpu.hardware.max_cube_count = (u32::MAX, u32::MAX, u32::MAX);
    cpu.hardware.load_width = 128;
    cpu.hardware.vector_register_count = Some(32);
    cases.push(("injected CPU properties", cpu));

    for (what, props) in cases {
        let threads = props.hardware.max_units_per_cube;
        // The units setting: the policy's, or forced; and a performance-core count below the
        // threads, above them, or none.
        let settings = [None, Some(CpuUnits::All), Some(CpuUnits::Performance)];
        let cores = [Some((threads * 3 / 4).max(1)), Some(threads + 4), None];
        for (units_setting, perf_cores) in settings
            .into_iter()
            .flat_map(|u| cores.into_iter().map(move |c| (u, c)))
        {
            let what = format!("{what}, units {units_setting:?}, performance cores {perf_cores:?}");
            with_properties(props.clone(), || {
                with_performance_cores(perf_cores, || match units_setting {
                    Some(u) => with_cpu_units(u, || check_cpu_plans(&client, &props, &what)),
                    None => check_cpu_plans(&client, &props, &what),
                })
            });
        }
    }
}

/// Every kind, element size, vector size and length under the current overrides: one unit per
/// hardware thread, or per performance core where the units say so and the count is smaller
/// (ADR-0017); blocks of whole 128-byte lines of about the policy's size.
fn check_cpu_plans(client: &Client, props: &DeviceProperties, what: &str) {
    let threads = props.hardware.max_units_per_cube;
    let policy = VectorPolicy::for_client(client);
    // The performance-core count in effect, read through a plan forced to `Performance`.
    let perf = with_cpu_units(CpuUnits::Performance, || {
        LaunchPlan::elementwise(client, KernelKind::Fill, 1, 4, 16, 1)
    })
    .unwrap()
    .cube_dim
    .num_elems();
    assert!(
        perf <= threads,
        "{what}: {perf} performance cores of {threads}"
    );
    for kind in KINDS {
        for elem_bytes in [4usize, 8] {
            let units = match policy.cpu_units(kind, elem_bytes) {
                Some(CpuUnits::Performance) => perf,
                Some(CpuUnits::All) => threads,
                None => panic!("{what}: a CPU class has CPU units"),
            };
            for n in [1usize, 2, 4, 8, 16] {
                for items in [1usize, 1000, 1_000_000, 10_000_003] {
                    let plan = with_vector_size(VectorSizeOverride::Fixed(n), || {
                        LaunchPlan::elementwise(client, kind, items, elem_bytes, 16, 1)
                    })
                    .unwrap_or_else(|e| panic!("{what}: {kind:?} {items}: {e}"));
                    check_plan(&plan, props, items, 16);
                    assert_eq!(
                        plan.cube_dim.num_elems(),
                        units,
                        "{what}: {kind:?} {elem_bytes}: one unit per thread of the policy's units"
                    );
                    let target = policy
                        .cpu_block_bytes(kind, elem_bytes)
                        .expect("a CPU class has CPU blocks");
                    let block = plan.items_per_unit as usize * plan.vector_size * elem_bytes;
                    assert!(
                        block.is_multiple_of(128) && (target / 2 < block && block <= target),
                        "{what}: {kind:?}, {elem_bytes}-byte items at vector size {}: a block \
                         of {block} bytes is not about the policy's {target} bytes of whole \
                         128-byte lines",
                        plan.vector_size
                    );
                }
            }
        }
    }
}

/// Properties with arbitrary launch limits.
fn injected(
    base: &DeviceProperties,
    cpu: bool,
    units: u32,
    dim: (u32, u32, u32),
    count: (u32, u32, u32),
    load_width: u32,
) -> DeviceProperties {
    let mut props = base.clone();
    let hw = &mut props.hardware;
    hw.num_cpu_cores = cpu.then_some(units);
    hw.max_units_per_cube = units;
    hw.max_cube_dim = dim;
    hw.max_cube_count = count;
    hw.load_width = load_width;
    hw.vector_register_count = cpu.then_some(32);
    props
}

fn limit() -> impl Strategy<Value = u32> {
    prop_oneof![
        1u32..=8,
        1u32..=65_535,
        Just(65_535u32),
        Just(i32::MAX as u32)
    ]
}

fn override_value() -> impl Strategy<Value = Option<VectorSizeOverride>> {
    prop_oneof![
        Just(None),
        Just(Some(VectorSizeOverride::Scalar)),
        Just(Some(VectorSizeOverride::Policy)),
        Just(Some(VectorSizeOverride::Widest)),
        prop::sample::select(vec![1usize, 2, 4, 8, 16])
            .prop_map(|n| Some(VectorSizeOverride::Fixed(n))),
    ]
}

proptest! {
    #![proptest_config(common::prop_config())]

    #[test]
    fn prop_plan_respects_injected_limits(
        cpu in any::<bool>(),
        units in prop_oneof![1u32..=16, 1u32..=1024],
        dim in (1u32..=1024, 1u32..=1024, 1u32..=64),
        count in (limit(), limit(), limit()),
        load_width in prop::sample::select(vec![128u32, 256, 512]),
        items in prop_oneof![0usize..=10_000, 0usize..=100_000_000, 0usize..=(1usize << 34)],
        elem_bytes in prop::sample::select(vec![4usize, 8]),
        max_vector in prop::sample::select(vec![1usize, 2, 4, 8, 16]),
        live in 1usize..=8,
        kind in prop::sample::select(KINDS.to_vec()),
        vector in override_value(),
        block in prop::option::of(prop::sample::select(vec![4usize << 10, 16 << 10, 64 << 10])),
        cpu_units in prop::option::of(prop::sample::select(vec![CpuUnits::All, CpuUnits::Performance])),
        perf_cores in prop::option::of(prop::option::of(0u32..=1100)),
    ) {
        let client = test_client();
        let props = injected(client.properties(), cpu, units, dim, count, load_width);
        let plan = with_properties(props.clone(), || {
            let make = || LaunchPlan::elementwise(&client, kind, items, elem_bytes, max_vector, live);
            let make = || match vector {
                Some(v) => with_vector_size(v, make),
                None => make(),
            };
            let make = || match block {
                Some(b) => with_cpu_block_bytes(b, make),
                None => make(),
            };
            let make = || match cpu_units {
                Some(u) => with_cpu_units(u, make),
                None => make(),
            };
            match perf_cores {
                Some(p) => with_performance_cores(p, make),
                None => make(),
            }
        });
        // Forced performance cores below every limit set cube_dim on CPUs (ADR-0017).
        if let (Ok(plan), true, Some(CpuUnits::Performance), Some(Some(p))) =
            (&plan, cpu, cpu_units, perf_cores)
            && p >= 1
            && p < units.min(dim.0)
        {
            prop_assert_eq!(plan.cube_dim.num_elems(), p);
        }
        if let (Ok(plan), Some(b), true) = (&plan, block, cpu) {
            let bytes = plan.items_per_unit as usize * plan.vector_size * elem_bytes;
            prop_assert!(bytes.is_multiple_of(128) && bytes <= b.max(128), "block {bytes} for {b}");
        }
        match plan {
            Ok(plan) => check_plan(&plan, &props, items, max_vector),
            Err(DeviceError::TooLarge) => {
                // A plan must exist when even one vector of one scalar per cube fits.
                let padded = items.max(1).next_multiple_of(VECTOR_PAD) as u128;
                let cubes = u128::from(count.0) * u128::from(count.1) * u128::from(count.2);
                prop_assert!(
                    padded > 1u128 << 30 || padded > cubes,
                    "TooLarge for {items} items with max_cube_count {count:?}"
                );
            }
            Err(other) => prop_assert!(false, "unexpected error {other}"),
        }
    }
}

// `LaunchPlan::per_group` (ADR-0042, item 4; docs/plans/M9h.md, T5).

/// The checks every `per_group` plan must pass under `props`: those of `elementwise` plans,
/// plus whole groups per cube, group slots that cover the vectors, and `units_per_group` the
/// largest power of two within `units` and the limits (at most 256 on GPUs; on CPUs, made
/// with `CpuUnits::All`, the hardware threads, ADR-0045).
fn check_group_plan(
    plan: &LaunchPlan,
    props: &DeviceProperties,
    groups: usize,
    units: usize,
    max_vector: usize,
) {
    check_plan(plan, props, groups, max_vector);
    let hw = &props.hardware;
    let limit = hw.max_units_per_cube.min(hw.max_cube_dim.0).max(1);
    let gpu = hw.num_cpu_cores.is_none();
    let cap = if gpu { limit.min(256) } else { limit };
    let within = units.min(cap as usize);
    let upg = plan.units_per_group;
    assert_eq!(
        upg as usize,
        1 << within.ilog2(),
        "{upg} units per group for {units} wanted within {cap}"
    );
    let dim = plan.cube_dim.num_elems();
    assert_eq!(dim % upg, 0, "cube_dim {dim} is not whole groups of {upg}");
    assert_eq!(dim, cap / upg * upg, "a cube holds as many groups as fit");
    if gpu {
        assert_eq!(plan.items_per_unit, 1);
    }
    let (x, y, z) = cube_count(plan);
    let slots = [x, y, z, dim / upg, plan.items_per_unit]
        .into_iter()
        .map(u128::from)
        .product::<u128>();
    assert!(
        slots >= u128::from(plan.num_vectors),
        "the group slots cover {slots} vectors, fewer than {}",
        plan.num_vectors
    );
}

/// Whether `DeviceError::TooLarge` is justified for a `per_group` plan with `groups` groups,
/// given the per-cube shape from a probe plan under the same properties and overrides.
///
/// `vector_size`, `cube_dim`, `units_per_group` and `items_per_unit` do not depend on `groups`
/// or `max_cube_count`, so checking ADR-0010's x-then-y-then-z spread and `Shape::plan`'s u32
/// launched-units bound (`x * y * z * cube_dim * items_per_unit <= u32::MAX`) against the
/// probe's shape is exact.
fn per_group_too_large_is_justified(
    probe: &LaunchPlan,
    groups: usize,
    count: (u32, u32, u32),
    shared_bytes: usize,
    shared_limit: usize,
) -> bool {
    if shared_bytes > shared_limit {
        return true;
    }
    let Some(padded) = groups.max(1).checked_next_multiple_of(VECTOR_PAD) else {
        return true;
    };
    let num_vectors = (padded / probe.vector_size) as u128;
    if num_vectors > u128::from(u32::MAX) {
        return true;
    }
    let cube_dim = u128::from(probe.cube_dim.num_elems());
    let upg = u128::from(probe.units_per_group);
    let ipu = u128::from(probe.items_per_unit);
    let per_cube = (cube_dim / upg) * ipu;
    let cubes = num_vectors.div_ceil(per_cube).max(1);
    let mx = u128::from(count.0.max(1));
    let my = u128::from(count.1.max(1));
    let mz = u128::from(count.2);
    let x = cubes.min(mx);
    let y = cubes.div_ceil(x).min(my);
    let z = cubes.div_ceil(x * y);
    if z > mz {
        return true;
    }
    let covered = x * y * z * cube_dim * ipu;
    covered > u128::from(u32::MAX)
}

/// Regression test for ADR-0010's `spread_cubes` 3D grid rounding pushing the launched-units
/// product (`x * y * z * cube_dim * items_per_unit`) past `u32::MAX` when `padded * cube` alone
/// still fits `u32` (M10 follow-up; reproduced on `vulkan-other` for the injected CPU limits).
#[test]
fn per_group_grid_rounding_overflow_is_too_large() {
    let client = test_client();
    // (label, cpu, injected max_cube_count): with a pinned 4 KiB CPU block (items_per_unit = 512),
    // at 402_647_040 groups both fit two xy planes (z = 2, covered = 3_221_176_320 <= u32::MAX);
    // at 402_647_041 groups both round to z = 3 (covered = 4_831_764_480 > u32::MAX).
    let cases = [
        ("cpu", true, (65_535, 6, 1196)),
        ("gpu", false, (65_535, 3072, 10)),
    ];
    for (label, cpu, count) in cases {
        let mut props = injected(client.properties(), cpu, 8, (8, 1, 1), count, 128);
        props.hardware.max_shared_memory_size = 18_874;
        let run = |groups| {
            with_properties(props.clone(), || {
                with_cpu_block_bytes(4 << 10, || {
                    with_cpu_units(CpuUnits::All, || {
                        LaunchPlan::per_group(
                            &client,
                            KernelKind::GeomFactors,
                            groups,
                            8,
                            8,
                            1,
                            2,
                            0,
                        )
                    })
                })
            })
        };
        let fits = run(402_647_040)
            .unwrap_or_else(|e| panic!("{label}: 402_647_040 groups should fit in z = 2: {e}"));
        check_group_plan(&fits, &props, 402_647_040, 8, 1);
        assert_eq!(cube_count(&fits).2, 2, "{label}: expected z = 2");
        assert!(
            !per_group_too_large_is_justified(&fits, 402_647_040, count, 0, 18_874),
            "{label}: TooLarge at 402_647_040 groups must not be considered justified"
        );

        match run(402_647_041) {
            Ok(plan) => check_group_plan(&plan, &props, 402_647_041, 8, 1),
            Err(DeviceError::TooLarge) => assert!(
                per_group_too_large_is_justified(&fits, 402_647_041, count, 0, 18_874),
                "{label}: TooLarge at 402_647_041 groups with max_cube_count {count:?} must be justified"
            ),
            Err(other) => panic!("{label}: unexpected error at 402_647_041 groups: {other}"),
        }
    }
}

proptest! {
    #![proptest_config(common::prop_config())]

    #[test]
    fn prop_per_group_respects_injected_limits(
        cpu in any::<bool>(),
        units_limit in prop_oneof![1u32..=16, 1u32..=1024],
        dim in (1u32..=1024, 1u32..=1024, 1u32..=64),
        count in (limit(), limit(), limit()),
        shared_limit in prop_oneof![Just(0usize), 1usize..=(64 << 10), Just(usize::MAX)],
        groups in prop_oneof![0usize..=10_000, 0usize..=100_000_000, 0usize..=(1usize << 34)],
        units in prop_oneof![1usize..=8, 1usize..=2048, Just(usize::MAX)],
        shared_bytes in prop_oneof![Just(0usize), 0usize..=(64 << 10)],
        elem_bytes in prop::sample::select(vec![4usize, 8]),
        max_vector in prop::sample::select(vec![1usize, 2, 4, 8, 16]),
        vector in override_value(),
        block in prop::option::of(prop::sample::select(vec![4usize << 10, 16 << 10, 64 << 10])),
    ) {
        let client = test_client();
        let mut props = injected(client.properties(), cpu, units_limit, dim, count, 128);
        props.hardware.max_shared_memory_size = shared_limit;
        let run_with = |run_props: DeviceProperties, run_groups: usize, run_shared: usize| {
            with_properties(run_props, || {
                let make = || {
                    LaunchPlan::per_group(
                        &client,
                        KernelKind::GeomFactors,
                        run_groups,
                        units,
                        elem_bytes,
                        max_vector,
                        2,
                        run_shared,
                    )
                };
                let make = || match vector {
                    Some(v) => with_vector_size(v, make),
                    None => make(),
                };
                let make = || match block {
                    Some(b) => with_cpu_block_bytes(b, make),
                    None => make(),
                };
                with_cpu_units(CpuUnits::All, make)
            })
        };
        let plan = run_with(props.clone(), groups, shared_bytes);
        match plan {
            Ok(plan) => {
                prop_assert!(shared_bytes <= shared_limit);
                check_group_plan(&plan, &props, groups, units, max_vector);
            }
            Err(DeviceError::TooLarge) => {
                if shared_bytes <= shared_limit {
                    let mut probe_props = props.clone();
                    probe_props.hardware.max_cube_count = (u32::MAX, 1, 1);
                    let probe = run_with(probe_props.clone(), 1, 0)
                        .expect("a 1-group probe with unconstrained cube count must fit");
                    check_group_plan(&probe, &probe_props, 1, units, max_vector);
                    if cpu {
                        let bytes = probe.items_per_unit as usize * probe.vector_size * elem_bytes;
                        let max_block = block.unwrap_or(64 << 10).max(128);
                        prop_assert!(
                            bytes.is_multiple_of(128) && bytes <= max_block,
                            "probe block {bytes} exceeds {max_block}"
                        );
                    }
                    prop_assert!(
                        per_group_too_large_is_justified(
                            &probe,
                            groups,
                            count,
                            shared_bytes,
                            shared_limit,
                        ),
                        "TooLarge for {groups} groups with max_cube_count {count:?}"
                    );
                }
            }
            Err(other) => prop_assert!(false, "unexpected error {other}"),
        }
    }
}

/// Leaves per group of the tree kernel (rule 4's comptime L).
const LEAVES: u32 = 64;

/// Rule 4's tree: out[v] sums, lane by lane, the `leaves` leaves of the vector of groups v,
/// leaf l at x[v · leaves + l]. Each unit of a group reduces its consecutive leaves
/// pairwise, then the units combine pairwise in shared memory (at strides t = 1, 2, …, unit
/// u with u mod 2t = 0 adds unit u + t's partial), so the sum is the pairwise tree over the
/// leaves for every `units` dividing `leaves`. Two buffers.
#[cube(launch_unchecked)]
fn group_tree_kernel<F: Float, N: Size>(
    x: &[Vector<F, N>],
    out: &mut [Vector<F, N>],
    num_vectors: u32,
    items_per_unit: u32,
    #[comptime] units: u32,
    #[comptime] cube_dim: u32,
    #[comptime] leaves: u32,
) {
    let per_unit = comptime!((leaves / units) as usize);
    let slots = comptime!((cube_dim / units) as usize);
    let slot = UNIT_POS / units;
    // Not UNIT_POS % units: a remainder by a comptime 1 folds wrongly on the CPU runtime (C3).
    let u = UNIT_POS - slot * units;
    let last = num_vectors as usize - 1;
    let first = (CUBE_POS * slots + slot as usize) * items_per_unit as usize;
    let mut shared = Shared::<[Vector<F, N>]>::new_slice(cube_dim as usize);
    let p = UNIT_POS as usize;
    for i in 0..items_per_unit as usize {
        let v = first + i;
        // Slots past the last vector read the last one and write nothing, so every unit
        // reaches every sync_cube.
        let base = min(v, last) * leaves as usize + u as usize * per_unit;
        let mut a = Array::<Vector<F, N>>::new(per_unit);
        for j in 0..per_unit {
            a[j] = x[base + j];
        }
        let mut s = 1usize;
        while s < per_unit {
            let mut j = 0usize;
            while j < per_unit {
                a[j] = a[j] + a[j + s];
                j += 2 * s;
            }
            s *= 2;
        }
        // The previous vector's reads of `shared` are done.
        sync_cube();
        shared[p] = a[0];
        sync_cube();
        let mut t = 1u32;
        while t < units {
            if (u & (2 * t - 1)) == 0 {
                shared[p] = shared[p] + shared[p + t as usize];
            }
            sync_cube();
            t *= 2;
        }
        if u == 0 && v < num_vectors as usize {
            out[v] = shared[p];
        }
    }
}

/// The pairwise tree over `leaves` on the host, adjacent pairs first, each sum rounded to `T`
/// (through f64, which rounds f32 sums correctly).
fn host_tree<T: TestScalar>(leaves: &[T]) -> T {
    let mut a = leaves.to_vec();
    let mut s = 1;
    while s < a.len() {
        for j in (0..a.len()).step_by(2 * s) {
            a[j] = T::of_f64(a[j].host_f64() + a[j + s].host_f64());
        }
        s *= 2;
    }
    a[0]
}

/// The tree kernel's sums of `groups` groups of `LEAVES` leaves (`leaf(g, l)`) under a
/// `per_group` plan for `units` units at the vector size of `vector`, and the plan.
fn run_group_tree<T: TestScalar + Float>(
    client: &Client,
    groups: usize,
    units: usize,
    vector: VectorSizeOverride,
    leaf: &impl Fn(usize, usize) -> T,
) -> (Vec<T>, LaunchPlan) {
    const MAX_VECTOR: usize = 4;
    let leaves = LEAVES as usize;
    // One vector per unit of a cube of at most 256 units.
    let shared_bytes = 256 * MAX_VECTOR * size_of::<T>();
    let plan = with_vector_size(vector, || {
        LaunchPlan::per_group(
            client,
            KernelKind::GeomFactors,
            groups,
            units,
            size_of::<T>(),
            MAX_VECTOR,
            2,
            shared_bytes,
        )
    })
    .unwrap();
    let n = plan.vector_size;
    let vectors = plan.num_vectors as usize;
    // Lane k of vector v is group v · n + k; padding groups get leaves too.
    let mut host = vec![T::of_f64(0.0); vectors * leaves * n];
    for v in 0..vectors {
        for l in 0..leaves {
            for k in 0..n {
                host[(v * leaves + l) * n + k] = leaf(v * n + k, l);
            }
        }
    }
    let x = DeviceBuffer::from_host(client, &host).unwrap();
    let out = DeviceBuffer::<T>::uninit(client, vectors * n).unwrap();
    tetraloma_device::debug_assert_bindings(client, 2);
    // SAFETY: `x` holds num_vectors · LEAVES vectors of n lanes, and the kernel reads vector
    // min(v, num_vectors − 1) · LEAVES + l with l < LEAVES; `out` holds num_vectors vectors,
    // and the kernel writes only v < num_vectors. The shared array has cube_dim entries,
    // indexed by UNIT_POS and by UNIT_POS + t within the unit's group.
    unsafe {
        group_tree_kernel::launch_unchecked::<T>(
            client,
            plan.cube_count.clone(),
            plan.cube_dim,
            n,
            x.vector_arg(n),
            out.vector_arg(n),
            plan.num_vectors,
            plan.items_per_unit,
            plan.units_per_group,
            plan.cube_dim.num_elems(),
            LEAVES,
        );
    }
    let mut sums = out.to_host().unwrap();
    sums.truncate(groups);
    (sums, plan)
}

/// Rule 4's tree over L = 64 leaves gives the same sums at units_per_group 1, 2 and
/// min(64, limit) and at vector sizes 1 and the policy's (at most 4): bitwise, and bitwise
/// equal to the host's tree, on the CPU runtime; within tolerance elsewhere. Cubes hold
/// several groups where the units allow (on CPUs too, ADR-0045), and CPU group slots visit
/// blocks of groups (4 KiB blocks, so 3000 groups take several cubes).
fn group_tree_case<T: TestScalar + Float>(client: &Client) {
    let groups = 3000;
    let leaves = LEAVES as usize;
    // Leaves of random sign and magnitudes up to 2^24, so the order of the sums matters.
    let mut rng = SplitMix::new(0x5eed_0042);
    let values: Vec<T> = (0..groups * leaves)
        .map(|_| {
            let scale = (1u64 << rng.below(25)) as f64;
            T::of_f64((rng.unit() - 0.5) * scale)
        })
        .collect();
    // Padding groups repeat the first ones.
    let leaf = |g: usize, l: usize| values[(g % groups) * leaves + l];
    let want: Vec<T> = (0..groups)
        .map(|g| host_tree(&values[g * leaves..(g + 1) * leaves]))
        .collect();
    let scale: Vec<f64> = (0..groups)
        .map(|g| (0..leaves).map(|l| leaf(g, l).host_f64().abs()).sum())
        .collect();
    // A sequential sum differs from the tree somewhere, so the test tells orders apart.
    let differs = (0..groups).any(|g| {
        let s = (0..leaves).fold(T::of_f64(0.0), |acc, l| {
            T::of_f64(acc.host_f64() + leaf(g, l).host_f64())
        });
        s.bits() != want[g].bits()
    });
    assert!(
        differs,
        "{}: the leaves do not tell summation orders apart",
        T::NAME
    );

    let limit = run_group_tree(client, 1, 64, VectorSizeOverride::Scalar, &leaf)
        .1
        .units_per_group;
    assert!(limit.is_power_of_two() && limit <= 64, "{limit}");
    let bitwise = tetraloma_testing::bitwise_device();
    for vector in [VectorSizeOverride::Scalar, VectorSizeOverride::Policy] {
        for units in [1usize, 2, 64] {
            let (have, plan) = with_cpu_block_bytes(4 << 10, || {
                run_group_tree(client, groups, units, vector, &leaf)
            });
            let what = format!(
                "{}, {units} units ({} per group), vector size {}, cube count {:?}",
                T::NAME,
                plan.units_per_group,
                plan.vector_size,
                cube_count(&plan)
            );
            assert_eq!(plan.units_per_group, (units as u32).min(limit), "{what}");
            for g in 0..groups {
                if bitwise {
                    assert_eq!(have[g].bits(), want[g].bits(), "{what}: group {g}");
                } else {
                    let err = (have[g].host_f64() - want[g].host_f64()).abs();
                    assert!(err <= T::TOL * scale[g], "{what}: group {g}: error {err:e}");
                }
            }
        }
    }
}

#[test]
fn group_tree_is_independent_of_units_per_group() {
    let client = test_client();
    group_tree_case::<f32>(&client);
    if tetraloma_testing::supports::<f64>(&client) {
        group_tree_case::<f64>(&client);
    } else {
        println!("{}: no f64", client.name());
    }
}

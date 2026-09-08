// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Criterion entry point. Registration and validation are outside timed closures.

mod support;

use criterion::{BatchSize, Criterion};
use std::{hint::black_box, path::Path};
use vihaco_benchmark::{Case, Workload, discover, validate};

use support::MeasurementGroup;
use vihaco_benchmark_api::BenchmarkMachine;
use vihaco_benchmark_machine::Machine;

fn main() -> eyre::Result<()> {
    let workloads = discover(&Path::new(env!("CARGO_MANIFEST_DIR")).join("workloads"))?;
    validate(&workloads)?;
    let mut criterion = support::criterion();
    // Cargo test invokes harness=false benches without --bench. Validate only.
    if !std::env::args().any(|arg| arg == "--bench") {
        return Ok(());
    }
    let group = MeasurementGroup::from_env()?;
    for workload in &workloads {
        for case in &workload.contract.cases {
            let prefix = format!("{}/{}", workload.contract.id, case.id);
            if group.includes_native() {
                register_native(&mut criterion, &prefix, workload, case);
            }
            if group.includes_vihaco() {
                register_program::<true>(&mut criterion, &prefix, workload, case);
                register_program::<false>(&mut criterion, &prefix, workload, case);
            }
        }
    }
    if group.includes_vihaco() {
        support::register_instructions(&mut criterion);
    }
    criterion.final_summary();
    Ok(())
}

fn register_native(criterion: &mut Criterion, prefix: &str, workload: &Workload, case: &Case) {
    criterion.bench_function(&format!("{prefix}/rust/program"), |b| {
        b.iter(|| {
            black_box((workload.native)(
                black_box(case.iterations),
                black_box(case.seed),
            ))
        })
    });
}

fn register_program<const COMPOSITE: bool>(
    criterion: &mut Criterion,
    prefix: &str,
    workload: &Workload,
    case: &Case,
) {
    let route = if COMPOSITE { "program" } else { "cpu-program" };
    criterion.bench_function(&format!("{prefix}/sst/{route}"), |b| {
        // Preparation and destruction stay outside timing. The generic route
        // remains statically selected, and inputs/results retain black-box barriers.
        b.iter_batched_ref(
            || {
                Machine::prepare(&workload.program, case.iterations, case.seed)
                    .expect("prepare program")
            },
            |machine| {
                black_box(
                    Machine::execute::<COMPOSITE>(machine, black_box(&workload.program))
                        .expect("execute program"),
                )
            },
            BatchSize::SmallInput,
        );
    });
}

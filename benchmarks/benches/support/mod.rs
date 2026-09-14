// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Configuration and isolated instruction measurements for the Criterion driver.

use criterion::{BatchSize, Criterion};
use eyre::{Result, eyre};
use std::{hint::black_box, path::Path, time::Duration};
use vihaco_benchmark_api::{COMPOSITE, CPU_ENUM, CPU_OPERATION, ConstantBenchmark};
use vihaco_benchmark_machine::Machine;

pub enum MeasurementGroup {
    All,
    References,
    Vihaco,
}

impl MeasurementGroup {
    pub fn from_env() -> Result<Self> {
        match std::env::var("VIHACO_BENCH_GROUP").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("all") => Ok(Self::All),
            Ok("references") => Ok(Self::References),
            Ok("vihaco") => Ok(Self::Vihaco),
            _ => Err(eyre!(
                "VIHACO_BENCH_GROUP must be all, references, or vihaco"
            )),
        }
    }

    pub fn includes_native(&self) -> bool {
        matches!(self, Self::All | Self::References)
    }

    pub fn includes_vihaco(&self) -> bool {
        matches!(self, Self::All | Self::Vihaco)
    }
}

pub fn criterion() -> Criterion {
    let mut criterion = Criterion::default().without_plots();
    if std::env::var("VIHACO_BENCH_PROFILE").as_deref() == Ok("smoke") {
        criterion = criterion
            .sample_size(10)
            .warm_up_time(Duration::from_millis(50))
            .measurement_time(Duration::from_millis(100))
            .nresamples(1000);
    }
    if let Some(output) = std::env::var_os("VIHACO_CRITERION_OUTPUT") {
        criterion = criterion.output_directory(Path::new(&output));
    }
    criterion
}

pub fn register_instructions(criterion: &mut Criterion) {
    micro::<CPU_OPERATION>(criterion, "instruction/const/sst/cpu-operation");
    micro::<CPU_ENUM>(criterion, "instruction/const/sst/cpu");
    micro::<COMPOSITE>(criterion, "instruction/const/sst/composite");
}

// Isolate one CPU operation from the program fetch/PC loop. Preparation
// and machine destruction are excluded by iter_batched_ref.
fn micro<const ROUTE: u8>(criterion: &mut Criterion, name: &str)
where
    Machine: ConstantBenchmark<ROUTE>,
{
    criterion.bench_function(name, |b| {
        b.iter_batched_ref(
            || <Machine as ConstantBenchmark<ROUTE>>::prepare().expect("prepare const"),
            |machine| {
                let instruction = <Machine as ConstantBenchmark<ROUTE>>::instruction(black_box(42));
                black_box(
                    <Machine as ConstantBenchmark<ROUTE>>::execute(
                        black_box(machine),
                        black_box(&instruction),
                    )
                    .expect("execute const"),
                );
            },
            BatchSize::SmallInput,
        )
    });
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Design prototype split by responsibility:
//! - `framework`: proposed vihaco routing API and its implementation.
//! - `contexts`: reusable example scheduler policy and annotation types.
//! - `machine`: the code a machine author writes.
//!
//! This file runs and checks the three end-to-end examples.
//! Run with `cargo run -p vihaco-runtime --example annotated_route --features typed-declarations`.

#[path = "annotated_route/contexts.rs"]
mod contexts;
#[path = "annotated_route/framework.rs"]
mod framework;
#[path = "annotated_route/machine.rs"]
mod machine;

use contexts::Scheduler;
use eyre::{Result, eyre};
use framework::Immediate;
use machine::{Machine, commands};
use std::time::Duration;
use vihaco_runtime::Step;

// All three timing policies must produce the same calibrated pulse.
fn assert_deliveries(machine: &Machine, dac_count: usize, monitor_count: usize) {
    assert_eq!(machine.dac.outputs.len(), dac_count);
    assert_eq!(machine.monitor.records.len(), monitor_count);
    for waveform in &machine.dac.outputs {
        assert_eq!(waveform.channel, 0);
        // sample * (instruction amplitude 2 * calibration gain 0.5) + offset 0.125
        assert_eq!(waveform.samples, [0.375, -0.375, 1.125]);
    }
    for record in &machine.monitor.records {
        assert_eq!(record.channel, 0);
        assert_eq!(record.sample_count, 3);
        assert_eq!(record.peak, 1.125);
    }
}

fn main() -> Result<()> {
    // Command-line inputs demonstrate that the dynamic values are not constants.
    // Defaults: 5 ns execution delay, 11 ns delivery delay.
    let mut args = std::env::args().skip(1);
    let execution_ns: u64 = args.next().map_or(Ok(5), |value| value.parse())?;
    let delivery_ns: u64 = args.next().map_or(Ok(11), |value| value.parse())?;
    // Require positive delivery delay so monitor precedes receiver in these checks.
    eyre::ensure!(delivery_ns > 0, "example expects a positive delivery delay");
    let runtime_due = Duration::from_nanos(execution_ns)
        .checked_add(Duration::from_nanos(delivery_ns))
        .ok_or_else(|| eyre!("time overflow"))?;
    let mixed_due = Duration::from_nanos(3)
        .checked_add(Duration::from_nanos(delivery_ns))
        .ok_or_else(|| eyre!("time overflow"))?;

    println!("compile-time latency: execution=3 ns, delivery=7 ns");
    let mut immediate = Machine::default();
    immediate.step(&commands::Fire(0, 2), &Immediate)?;
    assert_deliveries(&immediate, 1, 1);

    let mut machine = Machine::default();
    let scheduler = Scheduler::default();
    {
        let instruction = commands::Fire(0, 2);
        machine.step(&instruction, &scheduler)?;
    } // Neither the instruction nor the Step borrows survive submission.
    assert_eq!(machine.preparations, 1);
    assert_eq!(machine.source.executions, 0);
    assert_deliveries(&machine, 0, 0);

    assert!(scheduler.run_next(&mut machine)?); // execution at 3 ns
    assert_eq!(scheduler.now.get(), Duration::from_nanos(3));
    assert_eq!(machine.source.executions, 1);
    assert_deliveries(&machine, 0, 0);
    assert!(scheduler.run_next(&mut machine)?); // monitor at 3 ns
    assert_deliveries(&machine, 0, 1);
    assert!(scheduler.run_next(&mut machine)?); // receiver at 3 + 7 ns
    assert_eq!(scheduler.now.get(), Duration::from_nanos(10));
    assert_deliveries(&machine, 1, 1);
    assert!(!scheduler.run_next(&mut machine)?);
    assert_eq!(
        *scheduler.trace.borrow(),
        [(Duration::from_nanos(3), "monitor delivery")]
    );

    println!("runtime latency: execution={execution_ns} ns, delivery={delivery_ns} ns");
    let mut machine = Machine::default();
    let scheduler = Scheduler::default();
    {
        let instruction = commands::FireRuntime(0, 2, execution_ns, delivery_ns);
        machine.step(&instruction, &scheduler)?;
    }
    assert_eq!(machine.preparations, 1);
    assert_eq!(machine.source.executions, 0);
    assert!(scheduler.run_next(&mut machine)?);
    assert_eq!(scheduler.now.get(), Duration::from_nanos(execution_ns));
    assert_eq!(machine.source.executions, 1);
    assert_deliveries(&machine, 0, 0);
    assert!(scheduler.run_next(&mut machine)?);
    assert_deliveries(&machine, 0, 1);
    assert!(scheduler.run_next(&mut machine)?);
    assert_eq!(scheduler.now.get(), runtime_due);
    assert_deliveries(&machine, 1, 1);
    assert!(!scheduler.run_next(&mut machine)?);

    println!("mixed latency: execution=3 ns, delivery={delivery_ns} ns");
    let mut machine = Machine::default();
    let scheduler = Scheduler::default();
    {
        let instruction = commands::FireMixed(0, 2, delivery_ns);
        machine.step(&instruction, &scheduler)?;
    }
    assert_eq!(machine.preparations, 1);
    assert_eq!(machine.source.executions, 0);
    assert!(scheduler.run_next(&mut machine)?);
    assert_eq!(scheduler.now.get(), Duration::from_nanos(3));
    assert_eq!(machine.source.executions, 1);
    assert_deliveries(&machine, 0, 0);
    assert!(scheduler.run_next(&mut machine)?);
    assert_deliveries(&machine, 0, 1);
    assert!(scheduler.run_next(&mut machine)?);
    assert_eq!(scheduler.now.get(), mixed_due);
    assert_deliveries(&machine, 1, 1);
    assert!(!scheduler.run_next(&mut machine)?);
    // Message preparation errors occur before execution or submission.
    let mut invalid = Machine::default();
    let scheduler = Scheduler::default();
    let error = invalid
        .step(&commands::Fire(99, 2), &scheduler)
        .expect_err("unknown channel");
    assert!(error.to_string().contains("unknown channel"));
    assert_eq!(invalid.preparations, 0);
    assert_eq!(invalid.source.executions, 0);
    assert!(!scheduler.run_next(&mut invalid)?);

    // Delivery errors occur later, when the owned continuation runs.
    let mut out_of_range = Machine::default();
    let scheduler = Scheduler::default();
    out_of_range.step(&commands::Fire(0, 10), &scheduler)?;
    assert!(scheduler.run_next(&mut out_of_range)?); // pulse execution
    assert!(scheduler.run_next(&mut out_of_range)?); // monitoring
    let error = scheduler
        .run_next(&mut out_of_range)
        .expect_err("DAC rejects excessive amplitude");
    assert!(error.to_string().contains("DAC output range"));
    assert_eq!(out_of_range.source.executions, 1);
    assert_eq!(out_of_range.monitor.records.len(), 1);
    assert!(out_of_range.dac.outputs.is_empty());
    Ok(())
}

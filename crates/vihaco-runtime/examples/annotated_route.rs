// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Annotated topology graph: `framework` supplies typed nodes/edges and the
//! builder, `contexts` interprets annotations, and `machine` supplies behavior.
//! Run with `cargo run -p vihaco-runtime --example annotated_route --features typed-declarations -- 2 4`.

#[path = "annotated_route/contexts.rs"]
mod contexts;
#[path = "annotated_route/framework.rs"]
mod framework;
#[path = "annotated_route/machine.rs"]
mod machine;

use contexts::{ComputeTimeNs, DeliveryLatencyNs, Scheduler, TraceLabel};
use eyre::{Result, eyre};
use framework::{Annotation, Element, Immediate, NodeKind, deliver_to, route_through};
use machine::{__vihaco_fields_Machine, DacField, Machine, MonitorField, commands};
use std::time::Duration;
use vihaco_runtime::{Step, field};

fn assert_deliveries(machine: &Machine, dac_count: usize, monitor_count: usize) {
    assert_eq!(machine.dac.outputs.len(), dac_count);
    assert_eq!(machine.monitor.records.len(), monitor_count);
    for waveform in &machine.dac.outputs {
        assert_eq!(waveform.channel, 0);
        assert_eq!(waveform.samples, [0.375, -0.375, 1.125]);
    }
    for record in &machine.monitor.records {
        assert_eq!(record.channel, 0);
        assert_eq!(record.sample_count, 3);
        assert_eq!(record.peak, 1.125);
    }
}

fn inspect_graph() {
    // Inspection does not require a machine instance, context, or instruction value.
    let graph = route_through::<field!(Machine::source)>()
        .build_message::<commands::Fire>()
        .annotate::<ComputeTimeNs<2>>()
        .forward_message()
        .annotate::<DeliveryLatencyNs<1>>()
        .execute()
        .annotate::<ComputeTimeNs<3>>()
        .fan_out((
            deliver_to::<DacField>()
                .annotate::<DeliveryLatencyNs<7>>()
                .handle_effect()
                .annotate::<ComputeTimeNs<4>>(),
            deliver_to::<MonitorField>()
                .annotate_with(TraceLabel("monitor delivery"))
                .handle_effect()
                .annotate::<ComputeTimeNs<1>>(),
        ))
        .topology::<Machine>();
    assert_eq!(graph.len(), 7);
    let nodes: Vec<_> = graph
        .iter()
        .filter_map(|part| match &part.element {
            Element::Node { site, .. } => Some(*site),
            _ => None,
        })
        .collect();
    assert_eq!(nodes.len(), 4);
    assert_eq!(
        nodes
            .iter()
            .filter(|site| site.kind == NodeKind::ExecuteInstruction)
            .count(),
        1
    );
    assert_eq!(
        graph
            .iter()
            .filter(|part| matches!(part.annotation, Annotation::Static(_)))
            .count(),
        6
    );
    assert_eq!(
        graph
            .iter()
            .filter(|part| matches!(part.annotation, Annotation::Dynamic(_)))
            .count(),
        1
    );
    for part in &graph {
        if let Element::Edge {
            from, to, payload, ..
        } = &part.element
        {
            assert!(nodes.contains(from));
            assert!(nodes.contains(to));
            assert!(!payload.is_empty());
        }
        println!("{part:?}");
    }
    // One destination contains distinct build and execution nodes.
    assert_eq!(nodes[0].field_type, nodes[1].field_type);
    assert_ne!(nodes[0].kind, nodes[1].kind);
}

fn check_schedule<I>(instruction: I, execution_ns: u64, delivery_ns: u64) -> Result<()>
where
    Machine: Step<field!(Machine::source), I, Scheduler<Machine>>,
    machine::Source: vihaco_runtime::Execute<I>,
{
    let execution_end = Duration::from_nanos(3)
        .checked_add(Duration::from_nanos(execution_ns))
        .ok_or_else(|| eyre!("time overflow"))?;
    let arrival = execution_end
        .checked_add(Duration::from_nanos(delivery_ns))
        .ok_or_else(|| eyre!("time overflow"))?;
    let dac_end = arrival
        .checked_add(Duration::from_nanos(4))
        .ok_or_else(|| eyre!("time overflow"))?;
    let monitor_end = execution_end
        .checked_add(Duration::from_nanos(1))
        .ok_or_else(|| eyre!("time overflow"))?;
    let mut machine = Machine::default();
    let scheduler = Scheduler::default();
    machine.step(&instruction, &scheduler)?;
    drop(instruction); // All deferred inputs are owned.
    assert_eq!(machine.preparations, 0);
    assert_eq!(machine.source.executions, 0);
    assert!(scheduler.run_next(&mut machine)?); // Build message completes at 2 ns.
    assert_eq!(scheduler.now.get(), Duration::from_nanos(2));
    assert_eq!(machine.preparations, 1);
    assert_eq!(machine.source.executions, 0);
    assert!(scheduler.run_next(&mut machine)?); // Message arrives at 3 ns.
    assert_eq!(scheduler.now.get(), Duration::from_nanos(3));
    assert_eq!(machine.source.executions, 0);
    assert!(scheduler.run_next(&mut machine)?); // Execute completes.
    assert_eq!(scheduler.now.get(), execution_end);
    assert_eq!(machine.source.executions, 1);
    assert_deliveries(&machine, 0, 0);
    while scheduler.run_next(&mut machine)? {
        // Arrival schedules receiver computation; it does not apply the effect.
        let completed = scheduler.completed.borrow();
        if matches!(completed.last(), Some((_, Element::Edge { to, .. })) if to.name == "dac") {
            assert!(machine.dac.outputs.is_empty());
        }
    }
    assert_deliveries(&machine, 1, 1);

    let completed = scheduler.completed.borrow();
    assert_eq!(completed.len(), 7);
    for (time, element) in completed.iter() {
        match element {
            Element::Node { site, .. } if site.name == "dac" => assert_eq!(*time, dac_end),
            Element::Node { site, .. } if site.name == "monitor" => assert_eq!(*time, monitor_end),
            Element::Edge { to, .. } if to.name == "dac" => assert_eq!(*time, arrival),
            _ => {}
        }
    }
    let scheduled = scheduler.scheduled.borrow();
    assert_eq!(scheduled.len(), 7);
    for part in scheduled.iter() {
        assert!(part.due >= part.ready);
        if matches!(&part.element, Element::Node { site, .. } if site.name == "dac") {
            assert_eq!(part.ready, arrival);
            assert_eq!(part.due - part.ready, Duration::from_nanos(4));
        }
    }
    assert_eq!(
        *scheduler.trace.borrow(),
        [(execution_end, "monitor delivery")]
    );
    println!(
        "execution completes at {} ns, monitor at {} ns, DAC arrival at {} ns, DAC handling completes at {} ns",
        execution_end.as_nanos(),
        monitor_end.as_nanos(),
        arrival.as_nanos(),
        dac_end.as_nanos()
    );
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let execution_ns: u64 = args.next().map_or(Ok(5), |value| value.parse())?;
    let delivery_ns: u64 = args.next().map_or(Ok(11), |value| value.parse())?;
    inspect_graph();
    // Placement is a scheduler policy, not a universal meaning of annotations.
    let mut wrong_placement = Machine::default();
    let scheduler = Scheduler::default();
    let error = route_through::<field!(Machine::source)>()
        .build_message::<commands::Fire>()
        .annotate::<DeliveryLatencyNs<1>>() // A transfer annotation on a computation.
        .forward_message()
        .execute()
        .fan_out((
            deliver_to::<DacField>().handle_effect(),
            deliver_to::<MonitorField>().handle_effect(),
        ))
        .start(&mut wrong_placement, &scheduler, &commands::Fire(0, 2))
        .expect_err("delivery latency requires an edge");
    assert!(
        error
            .to_string()
            .contains("delivery latency requires an edge")
    );
    assert!(!scheduler.run_next(&mut wrong_placement)?);
    let mut immediate = Machine::default();
    immediate.step(&commands::Fire(0, 2), &Immediate)?;
    assert_deliveries(&immediate, 1, 1);

    println!("Static timings:");
    check_schedule(commands::Fire(0, 2), 3, 7)?;
    println!("Runtime execution duration and DAC delivery latency:");
    check_schedule(
        commands::FireRuntime(0, 2, execution_ns, delivery_ns),
        execution_ns,
        delivery_ns,
    )?;
    println!("Mixed timings:");
    check_schedule(commands::FireMixed(0, 2, delivery_ns), 3, delivery_ns)?;

    // Preparation is now a scheduled computation; its failure is deferred too.
    let mut invalid = Machine::default();
    let scheduler = Scheduler::default();
    invalid.step(&commands::Fire(99, 2), &scheduler)?;
    let error = scheduler
        .run_next(&mut invalid)
        .expect_err("unknown channel");
    assert!(error.to_string().contains("unknown channel"));
    assert_eq!(scheduler.now.get(), Duration::from_nanos(2));
    assert_eq!(invalid.preparations, 0);
    assert_eq!(invalid.source.executions, 0);
    assert!(!scheduler.run_next(&mut invalid)?);

    let mut excessive = Machine::default();
    let scheduler = Scheduler::default();
    excessive.step(&commands::Fire(0, 10), &scheduler)?;
    loop {
        match scheduler.run_next(&mut excessive) {
            Ok(true) => {}
            Ok(false) => panic!("expected DAC range failure"),
            Err(error) => {
                assert!(error.to_string().contains("DAC output range"));
                break;
            }
        }
    }
    assert_eq!(scheduler.now.get(), Duration::from_nanos(17));
    assert_eq!(excessive.source.executions, 1);
    assert_eq!(excessive.monitor.records.len(), 1);
    assert!(excessive.dac.outputs.is_empty());

    // Samples/calibration are read at message-node completion and owned thereafter.
    let mut snapshot = Machine::default();
    let scheduler = Scheduler::default();
    snapshot.step(&commands::Fire(0, 2), &scheduler)?;
    snapshot.set_calibration_gain(0.25);
    assert!(scheduler.run_next(&mut snapshot)?);
    snapshot.set_calibration_gain(100.0);
    while scheduler.run_next(&mut snapshot)? {}
    assert_eq!(snapshot.dac.outputs[0].samples, [0.25, -0.125, 0.625]);
    assert_eq!(snapshot.monitor.records[0].peak, 0.625);
    Ok(())
}

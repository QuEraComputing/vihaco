// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Annotated topology graph: `framework` supplies typed nodes/edges and the
//! builder, `contexts` interprets annotations, and `machine` supplies behavior.
//! Run with `cargo run -p vihaco-runtime --example annotated_route --features typed-declarations -- 2 4`.

#[path = "annotated_route/annotations.rs"]
mod annotations;
#[path = "annotated_route/contexts.rs"]
mod contexts;
#[path = "annotated_route/framework.rs"]
mod framework;
#[path = "annotated_route/machine.rs"]
mod machine;

use annotations::{ComputeTime, ConstNs, DeliveryLatency, RuntimeTime, TraceLabel};
use contexts::RecordingScheduler as Scheduler;
use eyre::{Result, eyre};
use framework::{
    AnnotationInfo, Computation, EffectWire, ExecuteAt, HandleAt, Immediate, Route, Stage, Visitor,
    Wire, deliver_to, route_through,
};
use machine::{__vihaco_fields_Machine, DacField, Machine, MonitorField, commands};
use std::{any::type_name, time::Duration};
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

// Inspection consumes concrete stage and annotation types through generic
// callbacks. These output lists are owned by this demonstration, not by Route.
#[derive(Default)]
struct GraphInspection {
    nodes: Vec<&'static str>,
    edges: Vec<(&'static str, &'static str)>,
    static_annotations: usize,
    runtime_annotations: usize,
}
impl GraphInspection {
    fn annotation<A: AnnotationInfo>(&mut self) {
        self.static_annotations += usize::from(A::STORAGE == "static");
        self.runtime_annotations += usize::from(A::STORAGE == "runtime");
    }
}
impl Visitor<Machine> for GraphInspection {
    fn node<N, Input, A>(&mut self, _: &A)
    where
        N: Stage<Machine, Input, Kind = Computation>,
        A: AnnotationInfo,
    {
        self.nodes.push(type_name::<N>());
        assert_eq!(type_name::<N::Source>(), type_name::<N>());
        assert_eq!(type_name::<N::Target>(), type_name::<N>());
        assert_eq!(N::source(), N::target());
        self.annotation::<A>();
        println!(
            "node {} at {}: instruction={}, input={}, output={}, payload={} [{} {}]",
            type_name::<N>(),
            N::source(),
            type_name::<N::Instruction>(),
            type_name::<Input>(),
            type_name::<N::Output>(),
            type_name::<N::Payload>(),
            A::STORAGE,
            type_name::<A::Value>()
        );
    }
    fn edge<N, Input, A>(&mut self, _: &A)
    where
        N: Stage<Machine, Input, Kind = Wire>,
        A: AnnotationInfo,
    {
        self.edges
            .push((type_name::<N::Source>(), type_name::<N::Target>()));
        self.annotation::<A>();
        println!(
            "edge {}: {} -> {}, payload={} [{} {}]",
            type_name::<N>(),
            N::source(),
            N::target(),
            type_name::<N::Payload>(),
            A::STORAGE,
            type_name::<A::Value>()
        );
    }
}

fn inspect_graph() {
    // Inspection does not require a machine instance, context, or instruction value.
    let static_route = route_through::<field!(Machine::source)>()
        .build_message::<commands::Fire>()
        .annotate::<ComputeTime<ConstNs<2>>>()
        .forward_message()
        .execute()
        .fan_out((
            deliver_to::<DacField>().handle_effect(),
            deliver_to::<MonitorField>().handle_effect(),
        ));
    assert_eq!(std::mem::size_of_val(&static_route), 0);
    assert_eq!(std::mem::size_of_val(static_route.graph()), 0);

    let runtime_route = route_through::<field!(Machine::source)>()
        .build_message::<commands::Fire>()
        .annotate::<ComputeTime<ConstNs<2>>>()
        .forward_message()
        .annotate::<DeliveryLatency<ConstNs<1>>>()
        .execute()
        .annotate::<ComputeTime<ConstNs<3>>>()
        .fan_out((
            deliver_to::<DacField>()
                .annotate::<DeliveryLatency<ConstNs<7>>>()
                .handle_effect()
                .annotate::<ComputeTime<ConstNs<4>>>(),
            deliver_to::<MonitorField>()
                .annotate_with(TraceLabel("monitor delivery"))
                .handle_effect()
                .annotate::<ComputeTime<ConstNs<1>>>(),
        ));
    // Only the runtime TraceLabel contributes data; the topology stays typed.
    assert!(std::mem::size_of_val(runtime_route.graph()) > 0);
    let mut inspection = GraphInspection::default();
    runtime_route.inspect::<Machine, _>(&mut inspection);
    assert_eq!(inspection.nodes.len(), 4);
    assert_eq!(inspection.edges.len(), 3);
    assert_eq!(inspection.static_annotations, 6);
    assert_eq!(inspection.runtime_annotations, 1);
    let execution = type_name::<ExecuteAt<field!(Machine::source)>>();
    assert_eq!(
        inspection
            .nodes
            .iter()
            .filter(|node| **node == execution)
            .count(),
        1
    );
    for (from, to) in &inspection.edges {
        assert!(inspection.nodes.contains(from));
        assert!(inspection.nodes.contains(to));
    }
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
    let dac_edge = type_name::<EffectWire<field!(Machine::source), DacField>>();
    while scheduler.run_next(&mut machine)? {
        // Arrival schedules receiver computation; it does not apply the effect.
        let completed = scheduler.observer.completed.borrow();
        if completed
            .last()
            .is_some_and(|(_, operation)| *operation == dac_edge)
        {
            assert!(machine.dac.outputs.is_empty());
        }
    }
    assert_deliveries(&machine, 1, 1);

    let completed = scheduler.observer.completed.borrow();
    assert_eq!(completed.len(), 7);
    for (time, operation) in completed.iter() {
        if *operation == type_name::<HandleAt<DacField>>() {
            assert_eq!(*time, dac_end);
        }
        if *operation == type_name::<HandleAt<MonitorField>>() {
            assert_eq!(*time, monitor_end);
        }
        if *operation == dac_edge {
            assert_eq!(*time, arrival);
        }
    }
    let scheduled = scheduler.observer.scheduled.borrow();
    assert_eq!(scheduled.len(), 7);
    for part in scheduled.iter() {
        assert!(part.due >= part.ready);
        if part.operation == type_name::<HandleAt<DacField>>() {
            assert_eq!(part.ready, arrival);
            assert_eq!(part.due - part.ready, Duration::from_nanos(4));
        }
    }
    assert_eq!(
        *scheduler.observer.trace.borrow(),
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

// New const amounts need no new bounds or implementations on C.
fn submit_new_static_amounts<C: machine::MachineContext>(
    machine: &mut Machine,
    context: &C,
) -> Result<()> {
    let route = route_through::<field!(Machine::source)>()
        .build_message::<commands::Fire>()
        .annotate::<ComputeTime<ConstNs<13>>>()
        .forward_message()
        .annotate::<DeliveryLatency<ConstNs<11>>>()
        .execute()
        .annotate::<ComputeTime<ConstNs<29>>>()
        .fan_out((
            deliver_to::<DacField>()
                .annotate::<DeliveryLatency<ConstNs<17>>>()
                .handle_effect()
                .annotate::<ComputeTime<ConstNs<23>>>(),
            deliver_to::<MonitorField>()
                .annotate::<DeliveryLatency<ConstNs<0>>>()
                .handle_effect()
                .annotate::<ComputeTime<ConstNs<5>>>(),
        ));
    assert_eq!(std::mem::size_of_val(&route), 0);
    route.start(machine, context, &commands::Fire(0, 2))
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let execution_ns: u64 = args.next().map_or(Ok(5), |value| value.parse())?;
    let delivery_ns: u64 = args.next().map_or(Ok(11), |value| value.parse())?;
    // Static and runtime forms use the same annotation definition, with storage
    // determined solely by their amount representation.
    assert_eq!(std::mem::size_of::<ComputeTime<ConstNs<3>>>(), 0);
    assert_eq!(std::mem::size_of::<DeliveryLatency<ConstNs<7>>>(), 0);
    assert_eq!(
        std::mem::size_of::<ComputeTime<RuntimeTime>>(),
        std::mem::size_of::<Duration>()
    );
    assert_eq!(
        std::mem::size_of::<DeliveryLatency<RuntimeTime>>(),
        std::mem::size_of::<Duration>()
    );
    inspect_graph();
    let mut immediate = Machine::default();
    immediate.step(&commands::Fire(0, 2), &Immediate)?;
    assert_deliveries(&immediate, 1, 1);

    // Recording is a backend choice, not required by Route or ordinary scheduling.
    let mut unrecorded = Machine::default();
    let scheduler = contexts::Scheduler::<Machine>::default();
    assert_eq!(std::mem::size_of_val(&scheduler.observer), 0);
    unrecorded.step(&commands::Fire(0, 2), &scheduler)?;
    while scheduler.run_next(&mut unrecorded)? {}
    assert_deliveries(&unrecorded, 1, 1);

    let mut new_amounts = Machine::default();
    let scheduler = Scheduler::default();
    submit_new_static_amounts(&mut new_amounts, &scheduler)?;
    while scheduler.run_next(&mut new_amounts)? {}
    assert_eq!(scheduler.now.get(), Duration::from_nanos(93));
    assert_deliveries(&new_amounts, 1, 1);

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

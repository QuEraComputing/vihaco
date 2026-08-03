// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Heterogeneous two-CPU demo. This is the end-to-end integration reference from
//! `demos/examples/demo.md`.
//!
//! The implementation is split into three layers under `demo/`: framework contracts in
//! `vihaco/`, reusable components in `stdlib/`, and this demo's user-written machine in `src/`.

#![allow(dead_code)]

use std::collections::HashMap;
use vihaco::Effects;

include!("demo/vihaco/execute.rs");
include!("demo/vihaco/resume.rs");
include!("demo/vihaco/supply.rs");
include!("demo/vihaco/handle.rs");
include!("demo/vihaco/machine_macro.rs");
include!("demo/vihaco/route.rs");
include!("demo/stdlib/debug_trace.rs");
include!("demo/stdlib/clock.rs");
include!("demo/stdlib/stack.rs");
include!("demo/stdlib/arithmetic.rs");
include!("demo/stdlib/channel.rs");
include!("demo/src/cpu.rs");
include!("demo/src/surface.rs");
include!("demo/src/machine.rs");

fn main() -> Result<(), CpuFault> {
    // The two CPU programs, authored with symbolic channel names, then resolved to runtime form.
    let cpu_a_program =
        resolve_program(&[SurfaceInstruction::Recv("from_b"), SurfaceInstruction::Mul]);
    let cpu_b_program = resolve_program(&[
        SurfaceInstruction::Sub,
        SurfaceInstruction::Mul,
        SurfaceInstruction::Send("to_a"),
    ]);

    // Two instances of the same reusable `Cpu`. Their local-to-global ratios are owned by the
    // root machine and selected by CpuId when each child is stepped.
    let fabric = std::rc::Rc::new(std::cell::RefCell::new(
        ChannelFabric::<i64>::with_channels(2),
    ));
    let transport_a = SharedTransport::new(fabric.clone());
    let transport_b = SharedTransport::new(fabric.clone());

    let cpu_a = Cpu {
        // CpuA starts with a receive and therefore parks at global tick 0. The value sent by
        // CpuB becomes the second operand for its multiplication.
        operand_stack: Stack::seeded(&[3]),
        alu: ArithmeticUnit::new(),
        channel: ChannelEndpoint::new(EndpointId(0), transport_a),
        debug: DebugTrace::default(),
        program: cpu_a_program,
        pc: 0,
    };
    let cpu_b = Cpu {
        // CpuB performs subtraction and multiplication before it reaches the send.
        operand_stack: Stack::seeded(&[10, 4, 2]),
        alu: ArithmeticUnit::new(),
        channel: ChannelEndpoint::new(EndpointId(1), transport_b),
        debug: DebugTrace::default(),
        program: cpu_b_program,
        pc: 0,
    };

    let mut machine = HeterogeneousMachine {
        clock: GlobalClock::new(),
        transport: SharedTransport::new(fabric),
        ticks_per_local_cycle: HashMap::from([
            (CpuId::A, GlobalTicksPerLocalCycle::new(3)?),
            (CpuId::B, GlobalTicksPerLocalCycle::new(1)?),
        ]),
        // CpuA has three global ticks per local tick. CpuB has one global tick per local tick.
        cpu_a,
        cpu_b,
        execution_trace: Vec::new(),
    };

    let outcome = machine.run()?;

    println!("global trace:");
    for line in &machine.execution_trace {
        println!("  {line}");
    }
    println!("outcome        = {outcome:?}");
    println!("CpuA stack     = {:?}", machine.cpu_a.operand_stack.items);
    println!("CpuB stack     = {:?}", machine.cpu_b.operand_stack.items);
    println!("CpuA debug      = {:?}", machine.cpu_a.debug.records);
    println!("CpuB debug      = {:?}", machine.cpu_b.debug.records);

    // Acceptance: CpuA's receive is woken at global tick 3, the next local boundary after
    // CpuB's send at global tick 2. CpuA then executes its multiply at tick 6.
    assert_eq!(outcome, RunOutcome::Completed);
    assert_eq!(
        machine.execution_trace,
        vec![
            "global  0: CpuA recv parks on ChannelId(1)",
            "global  0: CpuB Sub",
            "global  1: CpuB Mul",
            "global  2: CpuB send on ChannelId(1)",
            "global  3: CpuA wakes, recv 20",
            "global  6: CpuA Mul",
        ]
    );
    assert_eq!(machine.cpu_a.operand_stack.top(), Some(60));
    assert!(!machine.cpu_a.channel.is_parked());
    assert!(!machine.cpu_b.channel.is_parked());
    assert!(machine.cpu_a.finished());
    assert!(machine.cpu_b.finished());

    println!("OK: heterogeneous exchange completed with 60 on CpuA, no stale continuation");
    Ok(())
}

include!("demo/src/driver.rs");

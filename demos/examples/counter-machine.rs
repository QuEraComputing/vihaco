// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Clocked counter-group machine.
//!
//! Two counters are queued and played together. Their reports are recorded with the global tick
//! at which each channel was sampled.

#![allow(dead_code)]

// The included demo components refer to the facade's `Effects` type through this module root.
use vihaco::Effects;

#[path = "demo/stdlib/clock.rs"]
mod clock;
#[path = "demo/stdlib/counter.rs"]
mod counter;
#[path = "demo/stdlib/debug_trace.rs"]
mod debug_trace;
#[path = "demo/vihaco/handle.rs"]
mod handle;
#[path = "counter-machine/src/machine.rs"]
mod machine;

fn main() {
    use counter::counter_group::instruction;
    use machine::{CounterMachine, CounterMachineInstruction};

    let program = vec![
        CounterMachineInstruction::Queue(instruction::Queue {
            start: 10,
            duration: 2,
        }),
        CounterMachineInstruction::Queue(instruction::Queue {
            start: 100,
            duration: 4,
        }),
        CounterMachineInstruction::Play(instruction::Play),
        CounterMachineInstruction::Queue(instruction::Queue {
            start: 50,
            duration: 5,
        }),
        CounterMachineInstruction::Play(instruction::Play),
        CounterMachineInstruction::Queue(instruction::Queue {
            start: 50,
            duration: 5,
        }),
        CounterMachineInstruction::Play(instruction::Play),
    ];

    let mut machine = CounterMachine::new(program);
    machine.run().expect("counter machine should complete");

    for record in &machine.debug.records {
        println!("{record:?}");
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

use vihaco_runtime::{Dispatch, Execute, Step, component, composite, dialect, field};

#[dialect]
mod arithmetic {
    #[instruction]
    pub struct Add(pub u64);
}

#[component]
#[instructions { arithmetic }]
struct Counter(u64);

impl Execute<arithmetic::Add> for Counter {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;

    fn execute(&mut self, instruction: &arithmetic::Add, _: ()) -> Result<(), Self::Error> {
        self.0 += instruction.0;
        Ok(())
    }
}

#[composite]
struct Controller {
    #[device]
    counter: Counter,
}

#[composite]
struct Machine {
    #[device]
    direct: Counter,
    #[device]
    controller: Controller,
}

impl Step<field!(Machine::direct), arithmetic::Add> for Machine {
    fn step(&mut self, instruction: arithmetic::Add) {
        self.direct.execute(&instruction, ()).unwrap();
    }
}

impl Step<field!(Machine::controller::counter), arithmetic::Add> for Machine {
    fn step(&mut self, instruction: arithmetic::Add) {
        self.controller.counter.execute(&instruction, ()).unwrap();
    }
}

enum MachineInstruction {
    DirectAdd(arithmetic::Add),
    NestedAdd(arithmetic::Add),
}

impl Dispatch for Machine {
    type Instruction = MachineInstruction;

    fn dispatch(&mut self, instruction: Self::Instruction) {
        match instruction {
            MachineInstruction::DirectAdd(add) => {
                <Self as Step<field!(Machine::direct), arithmetic::Add>>::step(self, add);
            }
            MachineInstruction::NestedAdd(add) => {
                <Self as Step<field!(Machine::controller::counter), arithmetic::Add>>::step(
                    self, add,
                );
            }
        }
    }
}

#[test]
fn dispatch_selects_distinct_direct_and_nested_destinations() {
    fn dispatch<C: Dispatch>(composite: &mut C, instruction: C::Instruction) {
        composite.dispatch(instruction);
    }

    let mut machine = Machine {
        direct: Counter(0),
        controller: Controller {
            counter: Counter(0),
        },
    };
    dispatch(
        &mut machine,
        MachineInstruction::DirectAdd(arithmetic::Add(3)),
    );
    assert_eq!((machine.direct.0, machine.controller.counter.0), (3, 0));
    dispatch(
        &mut machine,
        MachineInstruction::NestedAdd(arithmetic::Add(7)),
    );
    assert_eq!((machine.direct.0, machine.controller.counter.0), (3, 7));
}

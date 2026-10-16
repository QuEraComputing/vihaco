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
    #[device(1)]
    counter: Counter,
}

#[composite]
struct Machine {
    #[device(2)]
    direct: Counter,
    #[device(3)]
    controller: Controller,
}

impl Step<field!(Controller::counter), arithmetic::Add> for Controller {
    fn step(&mut self, instruction: arithmetic::Add) {
        self.counter.execute(&instruction, ()).unwrap();
    }
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
        __vihaco_MachineInstruction::V_direct__arithmetic__Add(arithmetic::Add(3)),
    );
    assert_eq!((machine.direct.0, machine.controller.counter.0), (3, 0));
    dispatch(
        &mut machine,
        __vihaco_MachineInstruction::V_controller__counter__arithmetic__Add(arithmetic::Add(7)),
    );
    assert_eq!((machine.direct.0, machine.controller.counter.0), (3, 7));
}

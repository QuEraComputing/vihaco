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
    fn step(&mut self, instruction: &arithmetic::Add, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.counter.execute(instruction, ())?;
        Ok(())
    }
}

impl Step<field!(Machine::direct), arithmetic::Add> for Machine {
    fn step(&mut self, instruction: &arithmetic::Add, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.direct.execute(instruction, ())?;
        Ok(())
    }
}

impl Step<field!(Machine::controller::counter), arithmetic::Add> for Machine {
    fn step(&mut self, instruction: &arithmetic::Add, _: &()) -> vihaco_runtime::eyre::Result<()> {
        self.controller.counter.execute(instruction, ())?;
        Ok(())
    }
}

#[test]
fn dispatch_selects_distinct_direct_and_nested_destinations() {
    fn dispatch<C: Dispatch>(composite: &mut C, instruction: C::Instruction) {
        composite.dispatch(&instruction, &()).unwrap();
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

mod fallible {
    use super::*;
    use vihaco_runtime::eyre;

    #[dialect]
    mod control {
        #[instruction]
        pub struct Run(pub u32);

        #[instruction]
        pub struct Report;
    }

    #[derive(Debug, PartialEq)]
    enum Failure {
        Prepare,
        Execute,
        Handle,
    }

    impl std::fmt::Display for Failure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    impl std::error::Error for Failure {}

    #[component]
    #[instructions { control }]
    struct Device {
        executions: usize,
        handled: usize,
    }

    impl Execute<control::Run> for Device {
        type Message = ();
        type Effect = u32;
        type Error = Failure;

        fn execute(&mut self, instruction: &control::Run, _: ()) -> Result<u32, Failure> {
            self.executions += 1;
            if instruction.0 == 1 {
                return Err(Failure::Execute);
            }
            Ok(instruction.0)
        }
    }

    impl Execute<control::Report> for Device {
        type Message = ();
        type Effect = ();
        type Error = eyre::Report;

        fn execute(&mut self, _: &control::Report, _: ()) -> eyre::Result<()> {
            Err(eyre::Report::new(Failure::Execute).wrap_err("device context"))
        }
    }

    impl Device {
        fn prepare(&self, instruction: &control::Run) -> Result<(), Failure> {
            if instruction.0 == 0 {
                return Err(Failure::Prepare);
            }
            Ok(())
        }

        fn handle(&mut self, effect: u32) -> Result<(), Failure> {
            if effect == 2 {
                return Err(Failure::Handle);
            }
            self.handled += 1;
            Ok(())
        }
    }

    #[composite]
    struct Child {
        #[device(1)]
        device: Device,
    }

    #[composite]
    struct Machine {
        #[device(1)]
        direct: Device,
        #[device(2)]
        child: Child,
    }

    macro_rules! steps {
        ($machine:ty, $route:ty, $($device:ident).+) => {
            impl Step<$route, control::Run> for $machine {
                fn step(&mut self, instruction: &control::Run, _: &()) -> eyre::Result<()> {
                    self.$($device).+.prepare(&instruction)?;
                    let effect = self.$($device).+.execute(instruction, ())?;
                    self.$($device).+.handle(effect)?;
                    Ok(())
                }
            }

            impl Step<$route, control::Report> for $machine {
                fn step(&mut self, instruction: &control::Report, _: &()) -> eyre::Result<()> {
                    self.$($device).+.execute(instruction, ())?;
                    Ok(())
                }
            }
        };
    }

    steps!(Child, field!(Child::device), device);
    steps!(Machine, field!(Machine::direct), direct);
    steps!(Machine, field!(Machine::child::device), child.device);

    fn machine() -> Machine {
        Machine {
            direct: Device {
                executions: 0,
                handled: 0,
            },
            child: Child {
                device: Device {
                    executions: 0,
                    handled: 0,
                },
            },
        }
    }

    #[test]
    fn dispatch_propagates_each_failure_stage_for_direct_and_nested_destinations() {
        for nested in [false, true] {
            let mut machine = machine();
            for (value, failure, executions) in [
                (0, Failure::Prepare, 0),
                (1, Failure::Execute, 1),
                (2, Failure::Handle, 2),
            ] {
                let instruction = if nested {
                    __vihaco_MachineInstruction::V_child__device__control__Run(control::Run(value))
                } else {
                    __vihaco_MachineInstruction::V_direct__control__Run(control::Run(value))
                };
                let error = machine.dispatch(&instruction, &()).unwrap_err();
                assert_eq!(error.downcast_ref::<Failure>(), Some(&failure));
                let device = if nested {
                    &machine.child.device
                } else {
                    &machine.direct
                };
                assert_eq!(device.executions, executions);
                assert_eq!(device.handled, 0);
            }
            let instruction = if nested {
                __vihaco_MachineInstruction::V_child__device__control__Run(control::Run(3))
            } else {
                __vihaco_MachineInstruction::V_direct__control__Run(control::Run(3))
            };
            machine.dispatch(&instruction, &()).unwrap();
            let (selected, other) = if nested {
                (&machine.child.device, &machine.direct)
            } else {
                (&machine.direct, &machine.child.device)
            };
            assert_eq!((selected.executions, selected.handled), (3, 1));
            assert_eq!((other.executions, other.handled), (0, 0));
        }
    }

    #[test]
    fn dispatch_preserves_existing_report_context_and_concrete_error() {
        let mut machine = machine();
        for instruction in [
            __vihaco_MachineInstruction::V_direct__control__Report(control::Report),
            __vihaco_MachineInstruction::V_child__device__control__Report(control::Report),
        ] {
            let error = machine.dispatch(&instruction, &()).unwrap_err();
            assert_eq!(error.to_string(), "device context");
            assert_eq!(error.downcast_ref::<Failure>(), Some(&Failure::Execute));
        }
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]
#![allow(dead_code)]

use vihaco_runtime::__private::{LeafAt, RouteAt};
use vihaco_runtime::chumsky::Parser;
use vihaco_runtime::{
    Dispatch, Execute, HasInstructionSet, Parse, Step, component, composite, dialect, field,
};

#[dialect]
mod arithmetic {
    #[instruction]
    pub struct Add(pub i64);

    #[instruction]
    pub struct Halt;

    pub struct Helper;
}

#[component]
#[instructions { arithmetic }]
struct Cpu<T>(T);

impl<T, I> Execute<I> for Cpu<T> {
    type Message = ();

    type Effect = ();

    type Error = std::convert::Infallible;

    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}
// A narrower executable association does not limit aggregate availability.
impl<T> HasInstructionSet for Cpu<T> {
    type Runtime = arithmetic::Halt;

    type Syntax = arithmetic::syntax::Halt;
}

type Alias<T> = Cpu<T>;
pub use __vihaco_fields_Cpu as __vihaco_fields_Alias;
pub use __vihaco_inventory_Cpu as __vihaco_inventory_Alias;

#[composite]
struct Inner<T> {
    #[device(7, alias = "core", alias = "core-v2")]
    cpu: Alias<T>,
    #[device(8)]
    #[loadable]
    image: Image,
}

impl<T, I> Step<field!(Inner<T>::cpu), I> for Inner<T> {
    fn step(&mut self, instruction: I) {
        self.cpu.execute(&instruction, ()).unwrap();
    }
}

struct Image;

#[composite]
struct Machine<T> {
    #[device(42)]
    left: Inner<T>,
    #[device(43)]
    right: Inner<T>,
    #[device(44)]
    #[loadable]
    excluded: Inner<T>,
    total: i64,
}

impl<T> Step<field!(Machine<T>::left::cpu), arithmetic::Add> for Machine<T> {
    fn step(&mut self, instruction: arithmetic::Add) {
        self.total += instruction.0;
    }
}

impl<T> Step<field!(Machine<T>::right::cpu), arithmetic::Add> for Machine<T> {
    fn step(&mut self, instruction: arithmetic::Add) {
        self.total += 10 * instruction.0;
    }
}

impl<T> Step<field!(Machine<T>::left::cpu), arithmetic::Halt> for Machine<T> {
    fn step(&mut self, _: arithmetic::Halt) {
        self.total = -1;
    }
}

impl<T> Step<field!(Machine<T>::right::cpu), arithmetic::Halt> for Machine<T> {
    fn step(&mut self, _: arithmetic::Halt) {
        self.total = -2;
    }
}

#[test]
fn flat_payloads_and_generated_dispatch_keep_distinct_destinations() {
    let mut machine = Machine {
        left: Inner {
            cpu: Cpu(0u8),
            image: Image,
        },
        right: Inner {
            cpu: Cpu(0u8),
            image: Image,
        },
        excluded: Inner {
            cpu: Cpu(0u8),
            image: Image,
        },
        total: 0,
    };
    machine.dispatch(__vihaco_MachineInstruction::V_left__cpu__arithmetic__Add(
        arithmetic::Add(2),
    ));
    machine.dispatch(__vihaco_MachineInstruction::V_right__cpu__arithmetic__Add(
        arithmetic::Add(3),
    ));
    assert_eq!(machine.total, 32);
    machine.dispatch(__vihaco_MachineInstruction::V_left__cpu__arithmetic__Halt(
        arithmetic::Halt,
    ));
    assert_eq!(machine.total, -1);
    machine.dispatch(__vihaco_MachineInstruction::V_right__cpu__arithmetic__Halt(
        arithmetic::Halt,
    ));
    assert_eq!(machine.total, -2);
    assert_eq!(
        <__vihaco_inventory_Machine::Provider as LeafAt<0>>::DEVICE_CODES,
        [42, 7]
    );
    assert_eq!(
        <__vihaco_inventory_Machine::Provider as LeafAt<2>>::DEVICE_CODES,
        [43, 7]
    );
    assert_eq!(
        std::any::TypeId::of::<<__vihaco_inventory_Machine::Provider as RouteAt<0>>::Field>(),
        std::any::TypeId::of::<field!(Machine<u8>::left::cpu)>()
    );
    let _: <Machine<u16> as HasInstructionSet>::Runtime =
        __vihaco_MachineInstruction::V_left__cpu__arithmetic__Add(arithmetic::Add(1));
}

#[test]
fn complete_surface_parser_supports_aliases_and_preserves_leaf_values() {
    type Syntax = <Machine<u8> as HasInstructionSet>::Syntax;
    let source = Syntax::parser()
        .parse("left::core-v2::arithmetic::add -3")
        .into_result()
        .unwrap();
    assert_eq!(
        source,
        __vihaco_MachineSyntaxInstruction::V_left__cpu__arithmetic__Add(arithmetic::syntax::Add(
            -3
        ))
    );
    assert!(
        Syntax::parser()
            .parse("excluded::cpu::arithmetic::add 1")
            .has_errors()
    );
    assert!(
        Syntax::parser()
            .parse("left::image::arithmetic::add 1")
            .has_errors()
    );
}

#[composite]
struct Empty {}

#[test]
fn empty_inventory_has_an_uninhabited_aggregate_and_rejects_source() {
    assert_eq!(
        std::mem::size_of::<<Empty as HasInstructionSet>::Runtime>(),
        0
    );
    assert!(
        <Empty as HasInstructionSet>::Syntax::parser()
            .parse("")
            .has_errors()
    );
}
macro_rules! declarations {
    ($module:ident) => {
        mod $module {
            #[vihaco_runtime::dialect]
            mod control {
                #[instruction]
                pub struct Stop;
            }

            #[vihaco_runtime::component]
            #[instructions { control }]
            struct Cpu;

            impl vihaco_runtime::Execute<control::Stop> for Cpu {
                type Message = ();

                type Effect = ();

                type Error = std::convert::Infallible;

                fn execute(&mut self, _: &control::Stop, _: ()) -> Result<(), Self::Error> {
                    Ok(())
                }
            }

            #[vihaco_runtime::composite]
            struct Machine {
                #[device(255)]
                cpu: Cpu,
            }

            impl vihaco_runtime::Step<vihaco_runtime::field!(Machine::cpu), control::Stop> for Machine {
                fn step(&mut self, instruction: control::Stop) {
                    vihaco_runtime::Execute::execute(&mut self.cpu, &instruction, ()).unwrap();
                }
            }
        }
    };
}
declarations!(one);
declarations!(two);

#[test]
fn inventory_and_step_contract_diagnostics() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/inventory/*.rs");
}

mod readable_names {
    use super::*;

    #[dialect]
    mod first {
        #[instruction]
        pub struct Add;
    }

    #[dialect]
    mod second {
        #[instruction]
        pub struct Add;
    }

    #[component]
    #[instructions { first, second }]
    struct Device;

    impl<I> Execute<I> for Device {
        type Message = ();

        type Effect = ();

        type Error = std::convert::Infallible;

        fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[composite]
    struct Machine {
        #[device(1)]
        cpu_2: Device,
        #[device(2)]
        r#type: Device,
    }

    impl<I> Step<field!(Machine::cpu_2), I> for Machine {
        fn step(&mut self, instruction: I) {
            self.cpu_2.execute(&instruction, ()).unwrap();
        }
    }

    impl<I> Step<field!(Machine::r#type), I> for Machine {
        fn step(&mut self, instruction: I) {
            self.r#type.execute(&instruction, ()).unwrap();
        }
    }

    #[test]
    fn readable_names_distinguish_dialects_and_escape_raw_and_underscore_fields() {
        let _: <Machine as HasInstructionSet>::Runtime =
            __vihaco_MachineInstruction::V_cpu_u2__first__Add(first::Add);
        let _: <Machine as HasInstructionSet>::Runtime =
            __vihaco_MachineInstruction::V_cpu_u2__second__Add(second::Add);
        assert_eq!(
            <Machine as HasInstructionSet>::Syntax::parser()
                .parse("type::second::add")
                .into_result()
                .unwrap(),
            __vihaco_MachineSyntaxInstruction::V_type__second__Add(second::syntax::Add)
        );
    }
}

mod helper_capture {
    use vihaco_runtime as __VihacoMachine;

    #[__VihacoMachine::dialect]
    mod control {
        #[instruction]
        pub struct Halt;
    }

    #[__VihacoMachine::component]
    #[vihaco(crate = self::__VihacoMachine)]
    #[instructions { control }]
    struct Cpu;

    impl<I> __VihacoMachine::Execute<I> for Cpu {
        type Message = ();

        type Effect = ();

        type Error = std::convert::Infallible;

        fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[__VihacoMachine::composite]
    #[vihaco(crate = self::__VihacoMachine)]
    struct Machine {
        #[device(1)]
        cpu: self::Cpu,
    }

    impl<I> __VihacoMachine::Step<__VihacoMachine::field!(Machine::cpu), I> for Machine {
        fn step(&mut self, instruction: I) {
            __VihacoMachine::Execute::execute(&mut self.cpu, &instruction, ()).unwrap();
        }
    }

    #[test]
    fn direct_dispatch_preserves_authored_framework_root() {
        let _: <Machine as __VihacoMachine::HasInstructionSet>::Runtime =
            __vihaco_MachineInstruction::V_cpu__control__Halt(control::Halt);
    }
}

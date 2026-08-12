// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use eyre::Result;
use vihaco::{
    ContextHandle, Effects, Execute, Execution, NoEffect, NoMessage, Parse, ProgramImage,
    StepResult, Type, Value, component, composite,
    syntax::{ParsedFunction, ParsedModule},
};
use vihaco_parser::Ident;

component! {
    component Arithmetic {}

    runtime {
        instruction {
            #[derive(Clone)]
            Add(super::syntax::ArithmeticType),
        }
    }

    syntax {
        type ArithmeticType {
            Integer = "`integer`";
            Address = "`address`";
            Invalid = "`invalid`";
        }

        value ArithmeticValue {
            Zero = "`zero`";
        }

        instruction {
            Add(ArithmeticType) = "'add $0";
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for arithmetic::Arithmetic {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Default)]
struct IntegerStack {
    calls: usize,
}

#[derive(Default)]
struct AddressStack {
    calls: usize,
}

impl Execute<arithmetic::runtime::instruction::Add> for IntegerStack {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &arithmetic::runtime::instruction::Add,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.calls += 1;
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}

impl Execute<arithmetic::runtime::instruction::Add> for AddressStack {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &arithmetic::runtime::instruction::Add,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.calls += 1;
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}

composite! {
    #[derive(Default)]
    #[allow(dead_code)]
    composite MultiRouteMachine {
        error = eyre::Report;

        #[syntax("arithmetic")]
        arithmetic: arithmetic::Arithmetic,

        integer_stack: IntegerStack,
        address_stack: AddressStack,

        #[program]
        program: ProgramImage<MultiRouteMachineInstruction, TestContext, Value, Type>,
    }

    runtime {
        IntegerAdd(arithmetic::runtime::instruction::Add) => integer_stack {
            message none;
        }

        AddressAdd(arithmetic::runtime::instruction::Add) => address_stack {
            message none;
        }
    }
}

#[derive(Debug)]
struct TestContext;

impl vihaco::SstGlobalContext for TestContext {
    fn from_text(_text: &str) -> Result<Self> {
        Ok(Self)
    }
}

impl From<multi_route_machine::syntax::Type> for Type {
    fn from(value: multi_route_machine::syntax::Type) -> Self {
        match value {
            multi_route_machine::syntax::Type::Arithmetic(
                arithmetic::syntax::ArithmeticType::Integer,
            ) => Type::I64,
            multi_route_machine::syntax::Type::Arithmetic(
                arithmetic::syntax::ArithmeticType::Address,
            ) => Type::U64,
            multi_route_machine::syntax::Type::Arithmetic(
                arithmetic::syntax::ArithmeticType::Invalid,
            ) => Type::Undefined,
        }
    }
}

impl multi_route_machine::syntax::Resolver for MultiRouteMachine {
    fn lower_arithmetic(
        &mut self,
        instruction: arithmetic::syntax::Instruction,
    ) -> Result<Vec<multi_route_machine::runtime::Instruction>, eyre::Report> {
        let runtime_instruction = match instruction {
            arithmetic::syntax::Instruction::Add(kind) => {
                arithmetic::runtime::instruction::Add(kind)
            }
        };

        match runtime_instruction.0 {
            arithmetic::syntax::ArithmeticType::Integer => {
                Ok(vec![multi_route_machine::runtime::Instruction::IntegerAdd(
                    runtime_instruction,
                )])
            }
            arithmetic::syntax::ArithmeticType::Address => {
                Ok(vec![multi_route_machine::runtime::Instruction::AddressAdd(
                    runtime_instruction,
                )])
            }
            arithmetic::syntax::ArithmeticType::Invalid => Err(eyre::eyre!(
                "arithmetic::add requires a supported source type"
            )),
        }
    }
}

#[test]
fn component_surface_instruction_selects_runtime_route() {
    let integer = multi_route_machine::syntax::Instruction::parser()
        .parse("arithmetic::add integer")
        .into_result()
        .unwrap();
    let address = multi_route_machine::syntax::Instruction::parser()
        .parse("arithmetic::add address")
        .into_result()
        .unwrap();

    assert!(matches!(
        integer,
        multi_route_machine::syntax::Instruction::Arithmetic(arithmetic::syntax::Instruction::Add(
            arithmetic::syntax::ArithmeticType::Integer
        ))
    ));
    assert!(matches!(
        address,
        multi_route_machine::syntax::Instruction::Arithmetic(arithmetic::syntax::Instruction::Add(
            arithmetic::syntax::ArithmeticType::Address
        ))
    ));

    let mut machine = MultiRouteMachine::default();
    machine
        .load_parsed(
            ParsedModule {
                header: multi_route_machine::syntax::Header,
                functions: vec![ParsedFunction {
                    name: Ident("main".to_owned()),
                    params: Vec::<vihaco::syntax::Param<multi_route_machine::syntax::Module>>::new(
                    ),
                    return_ty: None,
                    body: vec![integer, address],
                }],
                labels: Vec::new(),
                constants: Vec::new(),
                strings: Vec::new(),
                source_symbols: Vec::new(),
            },
            ContextHandle::new(TestContext),
        )
        .unwrap();

    assert!(matches!(
        &machine.program.module.code[..],
        [
            MultiRouteMachineInstruction::IntegerAdd(_),
            MultiRouteMachineInstruction::AddressAdd(_),
        ]
    ));

    let integer_instruction = machine.program.module.code[0].clone();
    let address_instruction = machine.program.module.code[1].clone();
    machine.execute_generated(&integer_instruction).unwrap();
    machine.execute_generated(&address_instruction).unwrap();

    assert_eq!(machine.integer_stack.calls, 1);
    assert_eq!(machine.address_stack.calls, 1);
}

#[test]
fn semantic_type_mismatch_does_not_install_a_program() {
    let instruction = multi_route_machine::syntax::Instruction::parser()
        .parse("arithmetic::add invalid")
        .into_result()
        .unwrap();
    let mut machine = MultiRouteMachine::default();

    let error = machine
        .load_parsed(
            ParsedModule {
                header: multi_route_machine::syntax::Header,
                functions: vec![ParsedFunction {
                    name: Ident("main".to_owned()),
                    params: Vec::<vihaco::syntax::Param<multi_route_machine::syntax::Module>>::new(
                    ),
                    return_ty: None,
                    body: vec![instruction],
                }],
                labels: Vec::new(),
                constants: Vec::new(),
                strings: Vec::new(),
                source_symbols: Vec::new(),
            },
            ContextHandle::new(TestContext),
        )
        .unwrap_err();

    assert!(error.to_string().contains("supported source type"));
    assert!(machine.program.module.code.is_empty());
    assert!(machine.program.context.is_none());
}

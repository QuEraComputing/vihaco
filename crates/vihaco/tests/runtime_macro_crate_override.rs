// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use eyre::Result;
use vihaco::{Effects, Execute, Execution, Observe, StepResult, composite};
use vihaco_parser::{Ident, Parse};

mod test_root {
    pub use ::vihaco::*;
}

#[derive(Debug, Clone, Copy)]
pub struct TestInstruction;

struct TestMessage;
struct TestEffect;
struct TestComponent;
struct TestContext;

impl Execute<TestInstruction> for TestComponent {
    type Message = TestMessage;
    type Effect = TestEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &TestInstruction,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(TestEffect),
            execution: Execution::Complete,
        })
    }
}

#[derive(Default)]
struct TestObserver {
    observed: bool,
}

impl<R> Observe<TestEffect, R> for TestObserver {
    type Effect = ();
    type Error = eyre::Report;

    fn observe(&mut self, _effect: &TestEffect) -> Result<Effects<()>> {
        self.observed = true;
        Ok(Effects::none())
    }
}

composite! {
    #[vihaco(crate = crate::test_root)]
    pub composite TestMachine {
        error = eyre::Report;

        #[device(0x01)]
        component: TestComponent,
        observer: TestObserver,
        #[program]
        program: vihaco::ProgramImage<TestMachineInstruction, TestContext, vihaco::Value, ()>,
    }

    syntax {
        #[pattern = "'test::run"]
        Run => runtime Run;
        #[pattern = "'test::count $0"]
        Count(u32) => lower_count;
    }

    runtime {
        Run(TestInstruction) => component {
            message with resolve_message;
            effects {
                observe observer;
                handle with handle_effect;
            }
        }
    }
}

impl TestMachine {
    fn resolve_message(&mut self, _instruction: &TestInstruction) -> Result<TestMessage> {
        Ok(TestMessage)
    }

    fn handle_effect(&mut self, _effect: TestEffect) -> Result<()> {
        Ok(())
    }
}

impl test_machine::syntax::Resolver for TestMachine {
    fn lower_count(
        &mut self,
        _instruction: u32,
    ) -> Result<Vec<test_machine::runtime::Instruction>, eyre::Report> {
        Ok(Vec::new())
    }
}

#[test]
fn runtime_macros_honor_explicit_crate_override() {
    let parsed = test_machine::syntax::Instruction::parser()
        .parse("test::run")
        .into_result()
        .unwrap();
    assert!(matches!(parsed, test_machine::syntax::Instruction::Run));

    let mut machine = TestMachine {
        component: TestComponent,
        observer: TestObserver::default(),
        program: vihaco::ProgramImage::new(),
    };
    let outcome = machine
        .execute_generated(&TestMachineInstruction::Run(TestInstruction))
        .unwrap();
    assert_eq!(outcome, Execution::Complete);
    assert!(machine.observer.observed);

    let metadata = test_root::__private::GeneratedMachine::metadata(&machine);
    assert_eq!(metadata.devices[0].code, 0x01);
    assert_eq!(metadata.devices[0].name, "component");
}

#[test]
fn generated_program_loader_builds_and_installs_module() {
    let parsed = vihaco::syntax::ParsedModule {
        header: (),
        functions: vec![vihaco::syntax::ParsedFunction {
            name: Ident("main".to_owned()),
            params: Vec::<vihaco::syntax::Param<()>>::new(),
            return_ty: None,
            body: vec![test_machine::syntax::Instruction::Run],
        }],
    };
    let mut machine = TestMachine {
        component: TestComponent,
        observer: TestObserver::default(),
        program: vihaco::ProgramImage::new(),
    };

    machine
        .load_parsed(parsed, vihaco::ContextHandle::new(TestContext))
        .unwrap();

    assert_eq!(machine.program.module.code.len(), 1);
    assert_eq!(machine.program.module.functions.len(), 1);
    assert_eq!(machine.program.module.main_function, Some(0));
    assert_eq!(machine.program.pc, 0);
}

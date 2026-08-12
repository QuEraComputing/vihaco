// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use eyre::Result;
use vihaco::{
    Effects, Execute, Execution, NoEffect, Observe, SstFile, SstGlobalContext, StepResult, VERSION,
    composite,
};
use vihaco_parser::{Ident, Parse};

mod test_root {
    pub use ::vihaco::*;
}

#[derive(Debug, Clone, Copy)]
pub struct TestInstruction;

pub struct TestMessage(u32);
struct TestEffect;
struct IntermediateEffect;
struct TestComponent {
    received_message: Option<u32>,
}
struct TestContext;

impl SstGlobalContext for TestContext {
    fn from_text(_text: &str) -> Result<Self> {
        Ok(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(type)]
enum TestType {
    #[pattern = "`unit`"]
    Unit,
}

impl From<test_machine::syntax::Type> for TestType {
    fn from(value: test_machine::syntax::Type) -> Self {
        match value {}
    }
}

impl Execute<TestInstruction> for TestComponent {
    type Message = TestMessage;
    type Effect = TestEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &TestInstruction,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.received_message = Some(message.0);
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
    type Effect = NoEffect;
    type Error = eyre::Report;

    fn observe(&mut self, _effect: &TestEffect) -> Result<Effects<NoEffect>> {
        self.observed = true;
        Ok(Effects::none())
    }
}

#[derive(Default)]
struct TransformObserver {
    observed: bool,
}

#[derive(Default)]
struct SinkObserver {
    observed: bool,
}

impl<R> Observe<TestEffect, R> for TransformObserver {
    type Effect = IntermediateEffect;
    type Error = eyre::Report;

    fn observe(&mut self, _effect: &TestEffect) -> Result<Effects<Self::Effect>> {
        self.observed = true;
        Ok(Effects::one(IntermediateEffect))
    }
}

impl<R> Observe<IntermediateEffect, R> for SinkObserver {
    type Effect = NoEffect;
    type Error = eyre::Report;

    fn observe(&mut self, _effect: &IntermediateEffect) -> Result<Effects<Self::Effect>> {
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
        transform: TransformObserver,
        sink: SinkObserver,
        #[program]
        program: vihaco::ProgramImage<TestMachineInstruction, TestContext, vihaco::Value, TestType>,
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

        Nested(TestInstruction) => component {
            message with resolve_nested_message;
            effects {
                observe transform {
                    observe sink;
                }
            }
        }
    }
}

impl test_machine::runtime::MessageResolver for TestMachine {
    fn resolve_message(&mut self, _instruction: &TestInstruction) -> Result<TestMessage> {
        Ok(TestMessage(42))
    }

    fn resolve_nested_message(&mut self, _instruction: &TestInstruction) -> Result<TestMessage> {
        Ok(TestMessage(42))
    }
}

impl TestMachine {
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
        component: TestComponent {
            received_message: None,
        },
        observer: TestObserver::default(),
        transform: TransformObserver::default(),
        sink: SinkObserver::default(),
        program: vihaco::ProgramImage::new(),
    };
    let outcome = machine
        .execute_generated(&TestMachineInstruction::Run(TestInstruction))
        .unwrap();
    assert_eq!(outcome, Execution::Complete);
    assert_eq!(machine.component.received_message, Some(42));
    assert!(machine.observer.observed);

    let metadata = test_root::__private::GeneratedMachine::metadata(&machine);
    assert_eq!(metadata.devices[0].code, 0x01);
    assert_eq!(metadata.devices[0].name, "component");
}

#[test]
fn nested_observers_receive_concrete_follow_up_effects() {
    let mut machine = TestMachine {
        component: TestComponent {
            received_message: None,
        },
        observer: TestObserver::default(),
        transform: TransformObserver::default(),
        sink: SinkObserver::default(),
        program: vihaco::ProgramImage::new(),
    };

    machine
        .execute_generated(&TestMachineInstruction::Nested(TestInstruction))
        .unwrap();

    assert!(machine.transform.observed);
    assert!(machine.sink.observed);
}

#[test]
fn generated_program_loader_builds_and_installs_module() {
    let parsed = vihaco::syntax::ParsedModule {
        header: test_machine::syntax::Header,
        functions: vec![vihaco::syntax::ParsedFunction {
            name: Ident("main".to_owned()),
            params: Vec::<vihaco::syntax::Param<test_machine::syntax::Module>>::new(),
            return_ty: None,
            body: vec![test_machine::syntax::Instruction::Run],
        }],
        labels: Vec::new(),
        constants: Vec::new(),
        strings: Vec::new(),
        source_symbols: Vec::new(),
    };
    let mut machine = TestMachine {
        component: TestComponent {
            received_message: None,
        },
        observer: TestObserver::default(),
        transform: TransformObserver::default(),
        sink: SinkObserver::default(),
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

#[test]
fn generated_source_loader_parses_and_installs_module() {
    let file = SstFile::<TestContext>::from_text(&format!(
        "sst v{VERSION}\n\n.section(root):\n\t.text(root):\n\t\tfn @main() {{\n\t\t\ttest::run\n\t\t}}\n\t.text(root).\n.section(root).\n"
    ))
    .unwrap();
    let mut machine = TestMachine {
        component: TestComponent {
            received_message: None,
        },
        observer: TestObserver::default(),
        transform: TransformObserver::default(),
        sink: SinkObserver::default(),
        program: vihaco::ProgramImage::new(),
    };

    machine.load_source(file.root()).unwrap();

    assert_eq!(machine.program.module.code.len(), 1);
    assert_eq!(machine.program.module.functions.len(), 1);
    assert_eq!(machine.program.module.main_function, Some(0));
    assert_eq!(machine.program.pc, 0);
}

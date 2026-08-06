// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{Effects, Execute, Execution, Instruction, Observe, StepResult, composite};

mod test_root {
    pub use ::vihaco::*;
}

#[derive(Debug, Clone, Instruction)]
pub enum TestInstruction {
    Run,
}

struct TestMessage;
struct TestEffect;
struct TestComponent;

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
    }

    runtime_instructions {
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

#[test]
fn runtime_macros_honor_explicit_crate_override() {
    let mut machine = TestMachine {
        component: TestComponent,
        observer: TestObserver::default(),
    };
    let outcome = machine
        .execute_generated(&TestMachineInstruction::Run(TestInstruction::Run))
        .unwrap();
    assert_eq!(outcome, Execution::Complete);
    assert!(machine.observer.observed);

    let metadata = test_root::__private::GeneratedMachine::metadata(&machine);
    assert_eq!(metadata.devices[0].code, 0x01);
    assert_eq!(metadata.devices[0].name, "component");
}

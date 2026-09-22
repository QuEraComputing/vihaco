// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use eyre::Result;
use vihaco::{
    Component, Effects, GeneratedComponent, Instruction, Observe, Parse as _, dialect, dispatch,
    observe,
};

mod test_root {
    pub use ::vihaco::*;
}

dialect! {
    #[vihaco(crate = crate::test_root)]
    overridden {
        Run(u32 => u64),
    }
}

#[derive(Clone, Debug, PartialEq, crate::test_root::Parse)]
#[syntax_class(instruction, head = "overridden")]
pub struct Run(pub u32);

#[derive(Debug, Clone, Instruction)]
enum TestInstruction {
    Run,
}

struct TestMessage;

struct TestEffect;

#[derive(Debug, Clone, PartialEq, vihaco::Parse)]
#[syntax_class(instruction, head = "legacy")]
enum LegacySyntax {
    #[pattern = "'noop"]
    Noop,
}

struct TestComponent;

impl vihaco::HasInstructionSet for TestComponent {
    type Runtime = TestInstruction;
    type Syntax = LegacySyntax;
}

impl Component for TestComponent {}

#[dispatch(instruction = TestInstruction, message = TestMessage, effect = TestEffect)]
#[vihaco(crate = crate::test_root)]
impl TestComponent {
    fn execute(
        &mut self,
        _instruction: &TestInstruction,
        _message: TestMessage,
    ) -> Result<Effects<TestEffect>> {
        Ok(Effects::one(TestEffect))
    }
}

#[derive(Default)]
struct TestObserver {
    observed: bool,
}

#[observe(TestEffect, effect = ())]
#[vihaco(crate = crate::test_root)]
impl TestObserver {
    fn observe_test_effect(&mut self, _effect: &TestEffect) -> Result<Effects<()>> {
        self.observed = true;
        Ok(Effects::none())
    }
}

#[test]
fn runtime_macros_honor_explicit_crate_override() {
    let parsed = overridden::syntax::Run::parser()
        .parse("overridden.run 5")
        .into_result()
        .unwrap();
    assert_eq!(parsed, overridden::syntax::Run(5));
    assert_eq!(overridden::Run(5_u64), overridden::Run(5));

    let mut component = TestComponent;
    let effects = component
        .execute_generated(&TestInstruction::Run, TestMessage)
        .unwrap();
    assert_eq!(effects.into_iter().count(), 1);

    let mut observer = TestObserver::default();
    Observe::<TestEffect>::observe(&mut observer, &TestEffect)
        .unwrap()
        .into_iter()
        .for_each(drop);
    assert!(observer.observed);
}

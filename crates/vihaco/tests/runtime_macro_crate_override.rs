// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{
    Effects, GeneratedComponent, Instruction, Observe, component_attr as component, composite,
    observe,
};

mod test_root {
    pub use ::vihaco::*;
}

#[derive(Debug, Clone, Instruction)]
enum TestInstruction {
    Run,
}

struct TestMessage;

struct TestEffect;

struct TestComponent;

#[component(instruction = TestInstruction, message = TestMessage, effect = TestEffect)]
#[vihaco(crate = crate::test_root)]
impl TestComponent {
    fn execute(
        &mut self,
        _instruction: TestInstruction,
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

#[composite]
#[vihaco(crate = crate::test_root)]
struct TestMachine {
    #[device(0x01)]
    component: TestComponent,
}

#[test]
fn runtime_macros_honor_explicit_crate_override() {
    let mut component = TestComponent;
    let effects = component
        .execute_generated(TestInstruction::Run, TestMessage)
        .unwrap();
    assert_eq!(effects.into_iter().count(), 1);

    let mut observer = TestObserver::default();
    Observe::<TestEffect>::observe(&mut observer, &TestEffect)
        .unwrap()
        .into_iter()
        .for_each(drop);
    assert!(observer.observed);

    let machine = TestMachine { component };
    let _ = &machine.component;
    let metadata = test_root::__private::GeneratedMachine::metadata(&machine);
    assert_eq!(metadata.devices[0].code, 0x01);
    assert_eq!(metadata.devices[0].name, "component");
}

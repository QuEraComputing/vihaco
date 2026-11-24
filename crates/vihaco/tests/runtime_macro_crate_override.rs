// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{Effects, Message, MessageMarker, Observe, observe};

mod test_root {
    pub use ::vihaco::*;
}

#[derive(Message)]
#[vihaco(crate = crate::test_root)]
struct TestMessage;

struct TestEffect;

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
    fn assert_message<T: MessageMarker>() {}
    assert_message::<TestMessage>();

    let mut observer = TestObserver::default();
    Observe::<TestEffect>::observe(&mut observer, &TestEffect)
        .unwrap()
        .into_iter()
        .for_each(drop);
    assert!(observer.observed);
}

#[test]
fn runtime_macros_reject_invalid_crate_overrides() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/runtime_root/*.rs");
}

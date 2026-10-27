// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

#[test]
fn contextual_generated_dispatch() {
    let t = trybuild::TestCases::new();
    t.pass("tests/context-ui/context-ui-pass.rs");
    t.compile_fail("tests/context-ui/context-ui-fail.rs");
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Compile errors for component authors: verifying a module whose
//! instructions lack a rule, or whose rules need a capability the context
//! does not provide, through the public entry points.

#[test]
fn missing_rules_and_capabilities_are_compile_errors() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}

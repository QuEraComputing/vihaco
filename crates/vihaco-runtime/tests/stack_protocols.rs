// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

#[test]
fn missing_stack_capabilities_are_rejected() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/stack/*.rs");
}

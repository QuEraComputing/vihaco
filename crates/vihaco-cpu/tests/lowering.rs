// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

#[test]
fn cpu_lowering_requires_loader_capabilities_at_compile_time() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/lowering/*.rs");
}

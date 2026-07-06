// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn composite_machine_rejects_ambiguous_wiring() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-device-code.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-loadable-name.rs");
    t.compile_fail("tests/compile_fail/composite_machine/invalid-loadable-name.rs");
    t.compile_fail("tests/compile_fail/composite_machine/loadable-without-device.rs");
}

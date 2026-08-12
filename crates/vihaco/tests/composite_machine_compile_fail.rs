// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn composite_machine_rejects_ambiguous_wiring() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-device-code.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-loadable-name.rs");
    t.compile_fail("tests/compile_fail/composite_machine/invalid-loadable-name.rs");
    t.compile_fail("tests/compile_fail/composite_machine/loadable-without-device.rs");
    t.compile_fail("tests/compile_fail/composite_machine/missing-effects-handler.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-route-variant.rs");
    t.compile_fail("tests/compile_fail/composite_machine/missing-message-clause.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-message-clause.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-effect-handler.rs");
    t.compile_fail("tests/compile_fail/composite_machine/duplicate-observer.rs");
    t.compile_fail("tests/compile_fail/composite_machine/unknown-route-fields.rs");
    t.compile_fail("tests/compile_fail/composite_machine/unsupported-to-clause.rs");
}

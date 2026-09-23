// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[cfg(feature = "component-attribute")]
#[test]
fn component_attribute_rejects_invalid_selection_and_missing_coverage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/component_attribute/*.rs");
}

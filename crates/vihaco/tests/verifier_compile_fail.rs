// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "verifier")]

/// A `#[composite]` whose leaf has no verification rule: the headline names
/// the leaf, not the generated instruction aggregate.
#[test]
fn a_composite_leaf_without_a_rule_is_named() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/verifier/*.rs");
}

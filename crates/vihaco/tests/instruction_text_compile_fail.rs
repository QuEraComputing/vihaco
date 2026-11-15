// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

/// An operand whose syntax type has no `OperandText` and whose runtime type
/// has no `Display` has no source text: the error points at the operand.
#[test]
fn an_operand_without_source_text_is_a_compile_error() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/instruction_text/*.rs");
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn dialect_macro_rejects_invalid_declarations() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/dialect/duplicate-explicit-mnemonic.rs");
    t.compile_fail("tests/compile_fail/dialect/duplicate-instruction.rs");
    t.compile_fail("tests/compile_fail/dialect/duplicate-mnemonic.rs");
    t.compile_fail("tests/compile_fail/dialect/empty-dialect.rs");
    t.compile_fail("tests/compile_fail/dialect/malformed-pattern-before-duplicate.rs");
    t.compile_fail("tests/compile_fail/dialect/malformed-payload-mapping.rs");
    t.compile_fail("tests/compile_fail/dialect/missing-instruction-comma.rs");
    t.compile_fail("tests/compile_fail/dialect/named-fields.rs");
    t.compile_fail("tests/compile_fail/dialect/trailing-tokens.rs");
    t.compile_fail("tests/compile_fail/dialect/unsupported-attribute.rs");
    t.compile_fail("tests/compile_fail/dialect/unsupported-dialect-attribute.rs");
    t.compile_fail("tests/compile_fail/dialect/unsupported-vihaco-argument.rs");
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

#[test]
fn program_image_api_rejects_legacy_loading_and_instruction_mismatches() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/program_image/*.rs");
}

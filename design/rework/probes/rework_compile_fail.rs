// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn characterizes_reference_rejections() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/rework/*.rs");
}

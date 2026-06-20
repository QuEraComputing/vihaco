// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Continue,
    Breakpoint,
    Return,
    Halt,
}

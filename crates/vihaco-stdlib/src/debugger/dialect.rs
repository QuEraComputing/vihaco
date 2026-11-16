// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::dialect;

#[dialect]
pub mod debug {
    #[instruction]
    pub struct Breakpoint;
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[rt::dialect]
pub mod arithmetic {
    /// Add an integer operand.
    #[instruction]
    pub struct Add(pub i64);
    #[instruction]
    pub struct Halt;
}
pub use arithmetic as exported;

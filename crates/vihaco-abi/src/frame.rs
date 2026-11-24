// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Index of the first local slot in the stack.
    pub base: usize,

    /// Number of reserved local slots, including parameters.
    pub local_count: usize,

    /// Index of the function associated with this frame, if present.
    pub function: Option<usize>,

    /// Program counter to restore after this frame completes.
    pub ret_pc: u32,
}

impl Frame {
    /// Absolute index of the first operand, immediately after the locals.
    pub fn operands_index(&self) -> usize {
        self.base + self.local_count
    }
}

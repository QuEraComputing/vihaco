// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// base pointer to the bottom of the frame in the stack
    pub base: usize,

    /// Number of reserved local slots, including parameters.
    pub local_count: usize,

    /// source information (file, start, end)
    pub span: (u32, u32, u32),

    /// function index in the function table
    /// None if this frame is not associated with a function
    /// (e.g., the top-level frame, or a frame created by a direct call)
    pub function: Option<usize>,

    /// The PC to return to after this frame completes.
    pub ret_pc: u32,
}

impl Frame {
    /// Absolute index of the first operand, immediately after the locals.
    pub fn operands_index(&self) -> usize {
        self.base + self.local_count
    }
}

/// Local slot requirements by device code within a composite.
///
/// Counts include parameters. An absent entry means the function's arity is
/// its exact local slot requirement for that device.
pub type LocalCountsByDevice = std::collections::BTreeMap<u8, u32>;

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod context;
mod file;
mod format;
mod parser;
mod section;

#[cfg(test)]
mod tests;

pub use context::{BytecodeContext, BytecodeContextHandle, ProgramContext, ProgramGlobals};
pub use file::BytecodeFile;
pub use format::{CompositeHeader, ConstantId, FLAGS, MAGIC, VERSION, decode_instruction_stream};
pub use section::{SectionPath, SectionPathDisplay, SectionView, SectionWalk};

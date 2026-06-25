// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod common;
mod context;
mod file;
mod format;
mod parser;
mod section;
mod text;

#[cfg(test)]
mod tests;

pub use context::{BytecodeContext, ContextHandle, ProgramContext, ProgramGlobals};
pub use file::{BytecodeFile, FileContents};
pub use format::{decode_instruction_stream, CompositeHeader, ConstantId, FLAGS, MAGIC, VERSION};
pub use section::{SectionPath, SectionView};
pub use text::parse_instruction_stream;

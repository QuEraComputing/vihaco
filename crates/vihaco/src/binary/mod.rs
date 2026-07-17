// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod context;
mod file;
mod format;
mod section;

#[cfg(test)]
mod tests;

pub use context::{
    BytecodeGlobalContext, ContextHandle, GlobalContext, NoContext, SectionNameResolver,
    SstGlobalContext,
};
pub use file::{BytecodeFile, SstFile};
pub use format::{
    BytecodeHeader, ConstantId, FLAGS, MAGIC, SstHeader, VERSION, WriteBytecodeHeader,
    decode_instruction_stream,
};
pub use section::{BytecodeSectionView, SectionPath, SstSectionView};

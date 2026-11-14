// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco_bytecode;

mod container;
mod context;
mod file;
mod format;
mod section;

#[cfg(test)]
mod tests;

pub use container::sst::{
    SSTBlockLayout, SSTFramingError, SSTLayout, SSTSectionLayout, scan as scan_sst,
};
pub use context::{
    BytecodeGlobalContext, ContextHandle, GlobalContext, NoContext, SSTGlobalContext,
    SectionNameResolver,
};
pub use file::{BytecodeFile, SSTFile};
pub use format::{
    BytecodeHeader, ConstantId, FLAGS, MAGIC, SST_VERSION, SSTHeader, VERSION, WriteBytecodeHeader,
    decode_instruction_stream,
};
pub use section::{BytecodeSectionView, SSTSectionView};

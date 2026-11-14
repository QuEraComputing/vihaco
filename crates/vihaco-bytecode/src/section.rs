// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::Cursor;

use vihaco_abi::traits::Instruction;

use super::{
    context::ContextHandle,
    format::{BytecodeHeader, SSTHeader},
};

use crate::container::SectionEntry;

/// A lightweight view into one section of a bytecode file.
pub struct BytecodeSectionView<'bc, C> {
    contents: &'bc [u8],
    context: ContextHandle<C>,
    entry: &'bc SectionEntry,
}

impl<C> Clone for BytecodeSectionView<'_, C> {
    fn clone(&self) -> Self {
        Self {
            contents: self.contents,
            context: self.context.clone(),
            entry: self.entry,
        }
    }
}

impl<'bc, C> BytecodeSectionView<'bc, C> {
    pub(crate) fn from_parts(
        contents: &'bc [u8],
        context: ContextHandle<C>,
        entry: &'bc SectionEntry,
    ) -> Self {
        Self {
            contents,
            context,
            entry,
        }
    }

    /// The section's name. Names only select sections; execution uses addresses.
    pub fn name(&self) -> &'bc str {
        &self.entry.name
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    pub fn header_bytes(&self) -> &'bc [u8] {
        &self.contents[self.entry.header.clone()]
    }

    pub fn bytecode(&self) -> &'bc [u8] {
        &self.contents[self.entry.code.clone()]
    }

    /// Decode the specified header from raw header bytes.
    pub fn decode_header<H: BytecodeHeader>(&self) -> eyre::Result<H> {
        let bytes = self.header_bytes();
        let mut cursor = Cursor::new(bytes);
        let header = H::from_bytes(&mut cursor)?;
        if cursor.position() as usize != bytes.len() {
            return Err(eyre::eyre!(
                "section `{}` header has {} trailing bytes",
                self.name(),
                bytes.len() - cursor.position() as usize
            ));
        }
        Ok(header)
    }

    /// Decode this section's bytecode body as an instruction stream.
    pub fn decode_instructions<I: Instruction>(&self) -> eyre::Result<Vec<I>> {
        super::format::decode_instruction_stream(self.bytecode())
    }
}

impl<C> std::fmt::Debug for BytecodeSectionView<'_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BytecodeSectionView")
            .field("name", &self.name())
            .field("header_len", &self.header_bytes().len())
            .field("bytecode_len", &self.bytecode().len())
            .finish()
    }
}

/// A lightweight view into one section of an SST file.
pub struct SSTSectionView<'bc, C> {
    contents: &'bc str,
    context: ContextHandle<C>,
    entry: &'bc SectionEntry,
}

impl<C> Clone for SSTSectionView<'_, C> {
    fn clone(&self) -> Self {
        Self {
            contents: self.contents,
            context: self.context.clone(),
            entry: self.entry,
        }
    }
}

impl<'bc, C> SSTSectionView<'bc, C> {
    pub(crate) fn from_parts(
        contents: &'bc str,
        context: ContextHandle<C>,
        entry: &'bc SectionEntry,
    ) -> Self {
        Self {
            contents,
            context,
            entry,
        }
    }

    /// The section's name (from `.section <name>`). Names only select
    /// sections; execution uses addresses.
    pub fn name(&self) -> &'bc str {
        &self.entry.name
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// The raw text of the section's `.header` block (empty when omitted).
    pub fn header_text(&self) -> &'bc str {
        &self.contents[self.entry.header.clone()]
    }

    /// The raw text of the section's `.text` block (empty when omitted).
    pub fn text(&self) -> &'bc str {
        &self.contents[self.entry.code.clone()]
    }

    /// Parse the specified header from the section's header text.
    pub fn parse_header<H: SSTHeader>(&self) -> eyre::Result<H> {
        H::from_text(self.header_text())
    }
}

impl<C> std::fmt::Debug for SSTSectionView<'_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSTSectionView")
            .field("name", &self.name())
            .field("header_len", &self.header_text().len())
            .field("text_len", &self.text().len())
            .finish()
    }
}

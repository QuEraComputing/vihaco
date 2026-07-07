// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::Cursor;

use crate::traits::Instruction;

use super::{
    context::ContextHandle,
    format::{BytecodeHeader, SstHeader},
};

use vihaco_parser_core::container::SectionNode;
pub use vihaco_parser_core::container::SectionPath;

/// A lightweight view into a parsed bytecode section.
pub struct BytecodeSectionView<'bc, C> {
    contents: &'bc [u8],
    context: ContextHandle<C>,
    node: &'bc SectionNode,
}

impl<'bc, C> Clone for BytecodeSectionView<'bc, C> {
    fn clone(&self) -> Self {
        Self {
            contents: self.contents,
            context: self.context.clone(),
            node: self.node,
        }
    }
}

impl<'bc, C> BytecodeSectionView<'bc, C> {
    pub(crate) fn from_parts(
        contents: &'bc [u8],
        context: ContextHandle<C>,
        node: &'bc SectionNode,
    ) -> Self {
        Self {
            contents,
            context,
            node,
        }
    }

    pub fn path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn display_path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    pub fn children(&self) -> impl Iterator<Item = BytecodeSectionView<'bc, C>> + '_ {
        self.node.children.iter().map(|node| BytecodeSectionView {
            contents: self.contents,
            context: self.context.clone(),
            node,
        })
    }

    pub fn child(&self, local_name: &str) -> Option<BytecodeSectionView<'bc, C>> {
        self.node
            .children
            .iter()
            .find(|child| {
                child
                    .path
                    .local_name()
                    .is_some_and(|name| name == local_name)
            })
            .map(|node| BytecodeSectionView {
                contents: self.contents,
                context: self.context.clone(),
                node,
            })
    }

    pub fn local_name(&self) -> Option<&str> {
        self.node.path.local_name()
    }

    pub fn header_bytes(&self) -> &'bc [u8] {
        &self.contents[self.node.header.clone()]
    }

    pub fn bytecode(&self) -> &'bc [u8] {
        &self.contents[self.node.bytecode.clone()]
    }

    /// Decode the specified composite header from raw header bytes.
    pub fn decode_header<H: BytecodeHeader>(&self) -> eyre::Result<H> {
        let bytes = self.header_bytes();
        let mut cursor = Cursor::new(bytes);
        let header = H::from_bytes(&mut cursor)?;
        if cursor.position() as usize != bytes.len() {
            return Err(eyre::eyre!(
                "section `{}` header has {} trailing bytes",
                self.display_path(),
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
            .field("path", &self.display_path().to_string())
            .field("header_len", &self.header_bytes().len())
            .field("bytecode_len", &self.bytecode().len())
            .field("child_count", &self.node.children.len())
            .finish()
    }
}

/// A lightweight view into a parsed SST section.
pub struct SstSectionView<'bc, C> {
    contents: &'bc str,
    context: ContextHandle<C>,
    node: &'bc SectionNode,
}

impl<'bc, C> Clone for SstSectionView<'bc, C> {
    fn clone(&self) -> Self {
        Self {
            contents: self.contents,
            context: self.context.clone(),
            node: self.node,
        }
    }
}

impl<'bc, C> SstSectionView<'bc, C> {
    pub(crate) fn from_parts(
        contents: &'bc str,
        context: ContextHandle<C>,
        node: &'bc SectionNode,
    ) -> Self {
        Self {
            contents,
            context,
            node,
        }
    }

    pub fn path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn display_path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    pub fn children(&self) -> impl Iterator<Item = SstSectionView<'bc, C>> + '_ {
        self.node.children.iter().map(|node| SstSectionView {
            contents: self.contents,
            context: self.context.clone(),
            node,
        })
    }

    pub fn child(&self, local_name: &str) -> Option<SstSectionView<'bc, C>> {
        self.node
            .children
            .iter()
            .find(|child| {
                child
                    .path
                    .local_name()
                    .is_some_and(|name| name == local_name)
            })
            .map(|node| SstSectionView {
                contents: self.contents,
                context: self.context.clone(),
                node,
            })
    }

    pub fn local_name(&self) -> Option<&str> {
        self.node.path.local_name()
    }

    pub fn header_text(&self) -> &'bc str {
        &self.contents[self.node.header.clone()]
    }

    pub fn sst(&self) -> &'bc str {
        &self.contents[self.node.bytecode.clone()]
    }

    /// Parse the specified composite header from SST header text.
    pub fn parse_header<H: SstHeader>(&self) -> eyre::Result<H> {
        let text = self.header_text();
        let header = H::from_text(text)?;
        Ok(header)
    }

    /// Parse this section's SST body as an instruction stream.
    pub fn parse_instructions<I>(&self) -> eyre::Result<Vec<I>>
    where
        I: Instruction + vihaco_parser_core::Parse<'bc>,
    {
        super::format::parse_instruction_stream(self.sst())
    }
}

impl<C> std::fmt::Debug for SstSectionView<'_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SstSectionView")
            .field("path", &self.display_path().to_string())
            .field("header_len", &self.header_text().len())
            .field("sst_len", &self.sst().len())
            .field("child_count", &self.node.children.len())
            .finish()
    }
}

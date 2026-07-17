// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{
    context::{BytecodeGlobalContext, ContextHandle, SstGlobalContext},
    section::{BytecodeSectionView, SstSectionView},
};
use vihaco_parser_core::container::SectionNode;

/// A parsed bytecode file.
#[derive(Debug, Clone)]
pub struct BytecodeFile<C> {
    contents: Vec<u8>,
    context: ContextHandle<C>,
    root: SectionNode,
}

impl<C> BytecodeFile<C> {
    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// The view of the root of the bytecode section tree.
    pub fn root(&self) -> BytecodeSectionView<'_, C> {
        BytecodeSectionView::from_parts(self.contents.as_slice(), self.context.clone(), &self.root)
    }
}

impl<C> BytecodeFile<C>
where
    C: BytecodeGlobalContext,
{
    /// Parse a bytecode file from bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> eyre::Result<Self> {
        let context_range = vihaco_parser_core::container::bytecode::context_range(&bytes)?;
        let context = C::from_bytes(
            bytes
                .get(context_range)
                .ok_or_else(|| eyre::eyre!("global context is out of bounds"))?,
        )?;
        let parsed = vihaco_parser_core::container::bytecode::parse_file(&bytes, |index| {
            context.section_name(index).map(ToOwned::to_owned)
        })?;

        Ok(BytecodeFile {
            contents: bytes,
            context: ContextHandle::new(context),
            root: parsed.root,
        })
    }
}

/// A parsed SST file.
#[derive(Debug, Clone)]
pub struct SstFile<C> {
    contents: String,
    context: ContextHandle<C>,
    root: SectionNode,
}

impl<C> SstFile<C> {
    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// The view of the root of the SST section tree.
    pub fn root(&self) -> SstSectionView<'_, C> {
        SstSectionView::from_parts(self.contents.as_str(), self.context.clone(), &self.root)
    }
}

impl<C> SstFile<C>
where
    C: SstGlobalContext,
{
    /// Parse an SST file from text.
    pub fn from_text(text: &str) -> eyre::Result<Self> {
        let parsed = vihaco_parser_core::container::sst::parse_file(text)?;
        let context = C::from_text(
            text.get(parsed.context)
                .ok_or_else(|| eyre::eyre!("global context is out of bounds"))?,
        )?;

        Ok(SstFile {
            contents: text.to_string(),
            context: ContextHandle::new(context),
            root: parsed.root,
        })
    }
}

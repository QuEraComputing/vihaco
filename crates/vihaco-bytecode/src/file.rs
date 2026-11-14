// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{
    context::{BytecodeGlobalContext, ContextHandle, SSTGlobalContext},
    section::{BytecodeSectionView, SSTSectionView},
};
use crate::container::SectionEntry;

/// A parsed bytecode file: a global context plus a flat table of named sections.
#[derive(Debug, Clone)]
pub struct BytecodeFile<C> {
    contents: Vec<u8>,
    context: ContextHandle<C>,
    sections: Vec<SectionEntry>,
}

impl<C> BytecodeFile<C> {
    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// All sections, in file order.
    pub fn sections(&self) -> impl ExactSizeIterator<Item = BytecodeSectionView<'_, C>> + '_ {
        self.sections.iter().map(|entry| self.view(entry))
    }

    /// The section named `name`, if present.
    pub fn section(&self, name: &str) -> Option<BytecodeSectionView<'_, C>> {
        self.sections
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| self.view(entry))
    }

    fn view<'bc>(&'bc self, entry: &'bc SectionEntry) -> BytecodeSectionView<'bc, C> {
        BytecodeSectionView::from_parts(self.contents.as_slice(), self.context.clone(), entry)
    }
}

impl<C> BytecodeFile<C>
where
    C: BytecodeGlobalContext,
{
    /// Parse a bytecode file from bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> eyre::Result<Self> {
        let context_range = crate::container::bytecode::context_range(&bytes)?;
        let context = C::from_bytes(
            bytes
                .get(context_range)
                .ok_or_else(|| eyre::eyre!("global context is out of bounds"))?,
        )?;
        let parsed = crate::container::bytecode::parse_file(&bytes, |index| {
            context.section_name(index).map(ToOwned::to_owned)
        })?;

        Ok(BytecodeFile {
            contents: bytes,
            context: ContextHandle::new(context),
            sections: parsed.sections,
        })
    }
}

/// A parsed SST file: a global context plus a flat list of named sections.
#[derive(Debug, Clone)]
pub struct SSTFile<C> {
    contents: String,
    context: ContextHandle<C>,
    sections: Vec<SectionEntry>,
}

impl<C> SSTFile<C> {
    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// All sections, in file order.
    pub fn sections(&self) -> impl ExactSizeIterator<Item = SSTSectionView<'_, C>> + '_ {
        self.sections.iter().map(|entry| self.view(entry))
    }

    /// The section named `name`, if present.
    pub fn section(&self, name: &str) -> Option<SSTSectionView<'_, C>> {
        self.sections
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| self.view(entry))
    }

    fn view<'bc>(&'bc self, entry: &'bc SectionEntry) -> SSTSectionView<'bc, C> {
        SSTSectionView::from_parts(self.contents.as_str(), self.context.clone(), entry)
    }
}

impl<C> SSTFile<C>
where
    C: SSTGlobalContext,
{
    /// Parse an SST file from text.
    pub fn from_text(text: &str) -> eyre::Result<Self> {
        let parsed = crate::container::sst::parse_file(text)?;
        let context = C::from_text(
            text.get(parsed.context)
                .ok_or_else(|| eyre::eyre!("global context is out of bounds"))?,
        )?;

        Ok(SSTFile {
            contents: text.to_string(),
            context: ContextHandle::new(context),
            sections: parsed.sections,
        })
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{io::Cursor, ops::Range};

use crate::binary::file::FileContents;
use crate::program::ProgramContext;

use super::{
    context::{BytecodeContext, ContextHandle},
    format::CompositeHeader,
};

/// The fully resolved path for a given section.
///
/// The root section will be empty.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionPath {
    components: Vec<String>,
}

impl SectionPath {
    pub fn root() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    pub fn local_name(&self) -> Option<&str> {
        self.components.last().map(String::as_str)
    }

    pub fn child(&self, local_name: impl Into<String>) -> Self {
        let mut components = self.components.clone();
        components.push(local_name.into());
        Self { components }
    }
}

impl Default for SectionPath {
    fn default() -> Self {
        Self::root()
    }
}

impl std::fmt::Display for SectionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return f.write_str("<root>");
        }

        for (index, component) in self.components.iter().enumerate() {
            if index != 0 {
                f.write_str("/")?;
            }
            f.write_str(component)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for SectionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// The internal parser representation of a section.
///
/// For the public handle of a section, see [`SectionView`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SectionNode {
    pub path: SectionPath,
    pub section: Range<usize>,
    pub header: Range<usize>,
    pub bytecode: Range<usize>,
    pub children: Vec<SectionNode>,
}

/// The public handle of a bytecode section.
///
/// This is a lightweight view into information owned by [`BytecodeFile`].
pub struct SectionView<'bc, F = Vec<u8>, C = ProgramContext>
where
    F: FileContents,
    C: BytecodeContext,
{
    pub(super) contents: &'bc F,
    pub(super) context: ContextHandle<C>,
    pub(super) node: &'bc SectionNode,
}

impl<'bc, F, C> Clone for SectionView<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    fn clone(&self) -> SectionView<'bc, F, C> {
        SectionView {
            contents: self.contents,
            context: self.context.clone(),
            node: self.node,
        }
    }
}

impl<'bc, F, C> SectionView<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    pub fn path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn display_path(&self) -> &'bc SectionPath {
        &self.node.path
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    pub fn children(&self) -> impl Iterator<Item = SectionView<'bc, F, C>> + '_ {
        self.node.children.iter().map(|node| SectionView {
            contents: self.contents,
            context: self.context.clone(),
            node,
        })
    }

    pub fn child(&self, local_name: &str) -> Option<SectionView<'bc, F, C>> {
        self.node
            .children
            .iter()
            .find(|child| {
                child
                    .path
                    .local_name()
                    .is_some_and(|name| name == local_name)
            })
            .map(|node| SectionView {
                contents: self.contents,
                context: self.context.clone(),
                node,
            })
    }

    pub fn local_name(&self) -> Option<&str> {
        self.node.path.local_name()
    }
}

pub type BinarySectionView<'bc, C> = SectionView<'bc, Vec<u8>, C>;

impl<'bc, C> BinarySectionView<'bc, C>
where
    C: BytecodeContext,
{
    pub fn header_bytes(&self) -> &'bc [u8] {
        &self.contents[self.node.header.clone()]
    }

    pub fn bytecode(&self) -> &'bc [u8] {
        &self.contents[self.node.bytecode.clone()]
    }

    /// Parse the specified composite header from the raw header bytes.
    pub fn parse_header<H: CompositeHeader>(&self) -> eyre::Result<H> {
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
}

impl<C> std::fmt::Debug for BinarySectionView<'_, C>
where
    C: BytecodeContext,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectionView")
            .field("path", &self.display_path().to_string())
            .field("header_len", &self.header_bytes().len())
            .field("bytecode_len", &self.bytecode().len())
            .field("child_count", &self.node.children.len())
            .finish()
    }
}

pub type TextSectionView<'bc, C> = SectionView<'bc, String, C>;

impl<'bc, C> TextSectionView<'bc, C>
where
    C: BytecodeContext,
{
    pub fn header_text(&self) -> &'bc str {
        &self.contents[self.node.header.clone()]
    }

    pub fn text(&self) -> &'bc str {
        &self.contents[self.node.bytecode.clone()]
    }

    /// Parse the specified composite header from the text format.
    pub fn parse_header<H: CompositeHeader>(&self) -> eyre::Result<H> {
        let text = self.header_text();
        let mut cursor = Cursor::new(text);
        let header = H::from_text(&mut cursor)?;
        if cursor.position() as usize != text.len() {
            return Err(eyre::eyre!(
                "section `{}` header has {} trailing bytes",
                self.display_path(),
                text.len() - cursor.position() as usize
            ));
        }
        Ok(header)
    }
}

impl<C> std::fmt::Debug for TextSectionView<'_, C>
where
    C: BytecodeContext,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectionView")
            .field("path", &self.display_path().to_string())
            .field("header_len", &self.header_text().len())
            .field("text_len", &self.text().len())
            .field("child_count", &self.node.children.len())
            .finish()
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::Cursor;

use super::{
    context::{BytecodeContext, BytecodeContextHandle, ProgramContext},
    format::BytecodeHeader,
    parser::{SectionParseInfo, checked_add, parse_section},
    section::{SectionNode, SectionPath, SectionView, SectionWalk},
};

/// A parsed bytecode file.
///
/// This connects the raw bytes of the file with the parsed global context
/// and the tree of section nodes.
#[derive(Debug, Clone)]
pub struct BytecodeFile<C = ProgramContext> {
    bytes: Vec<u8>,
    context: BytecodeContextHandle<C>,
    root: SectionNode,
}

impl<C> BytecodeFile<C> {
    /// The public entry point for the parsing of a bytecode file.
    ///
    /// This will automatically split a multi-section file into
    /// each individual section node and send it to its corresponding
    /// composite loader marked with `#[program]`.
    pub fn from_bytes(bytes: Vec<u8>) -> eyre::Result<Self>
    where
        C: BytecodeContext,
    {
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = BytecodeHeader::read_from(&mut cursor)?;

        let context_start = BytecodeHeader::ENCODED_LEN;
        let context_end = checked_add(context_start, header.context_len, "program context end")?;
        let context = C::from_bytes(
            bytes
                .get(context_start..context_end)
                .ok_or_else(|| eyre::eyre!("program context is out of bounds"))?,
        )?;
        let root = parse_section(
            &bytes,
            &context,
            SectionParseInfo {
                start: context_end,
                path: SectionPath::root(),
            },
        )?;
        if root.section.end != bytes.len() {
            return Err(eyre::eyre!(
                "bytecode length mismatch: root section describes {} bytes, file has {} bytes",
                root.section.end,
                bytes.len()
            ));
        }

        Ok(Self {
            bytes,
            context: BytecodeContextHandle::new(context),
            root,
        })
    }

    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> BytecodeContextHandle<C> {
        self.context.clone()
    }

    /// The view of the root of the section tree.
    pub fn root(&self) -> SectionView<'_, C> {
        SectionView {
            bytes: &self.bytes,
            context: self.context.clone(),
            node: &self.root,
        }
    }

    /// Walk every section in this file in depth-first order.
    ///
    /// The first yielded section is always the root section.
    pub fn sections(&self) -> SectionWalk<'_, C>
    where
        C: BytecodeContext,
    {
        self.root().walk()
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::Cursor;

use crate::binary::text::{
    consume_context, lex_lines, line_error as text_line_error, parse_section as parse_text_section,
    verify_version, LineCursor, LineKind, TextSectionParseInfo,
};

use super::{
    context::{BytecodeContext, ContextHandle, ProgramContext},
    format::BytecodeHeader,
    parser::{checked_add, parse_section, SectionParseInfo},
    section::{SectionNode, SectionPath, SectionView},
};

/// We're sealing [`FileContents`] for two reasons:
///
/// 1. we don't want downstream authors to implement their own file content
///    support (yet? maybe in the future we can expose an API), and
/// 2. we don't want to use an enum for [`BytecodeFile`] when we know the
///    type of our file's contents statically; when dealing with a
///    [`BytecodeFile`] we'd have to constantly destruct and unreachable!()
///
/// https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
mod private {
    pub trait Sealed {}

    impl Sealed for Vec<u8> {}
    impl Sealed for String {}
}

pub trait FileContents: private::Sealed {}

impl FileContents for Vec<u8> {}
impl FileContents for String {}

/// A parsed bytecode file.
///
/// This connects the raw bytes of the file with the parsed global context
/// and the tree of section nodes.
#[derive(Debug, Clone)]
pub struct BytecodeFile<F = Vec<u8>, C = ProgramContext>
where
    F: FileContents,
    C: BytecodeContext,
{
    contents: F,
    context: ContextHandle<C>,
    root: SectionNode,
}

impl<F, C> BytecodeFile<F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    pub fn context(&self) -> &C {
        self.context.get()
    }

    pub fn context_handle(&self) -> ContextHandle<C> {
        self.context.clone()
    }

    /// The view of the root of the section tree.
    pub fn root(&self) -> SectionView<'_, F, C> {
        SectionView {
            contents: &self.contents,
            context: self.context.clone(),
            node: &self.root,
        }
    }
}

pub type BinaryBytecodeFile<C = ProgramContext> = BytecodeFile<Vec<u8>, C>;

impl<C> BinaryBytecodeFile<C>
where
    C: BytecodeContext,
{
    /// The public entry point for the parsing of a bytecode file.
    ///
    /// This will automatically split a multi-section file into
    /// each individual section node and send it to its corresponding
    /// composite loader marked with `#[program]`.
    pub fn from_bytes(bytes: Vec<u8>) -> eyre::Result<BinaryBytecodeFile<C>> {
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

        Ok(BytecodeFile {
            contents: bytes,
            context: ContextHandle::new(context),
            root,
        })
    }
}

pub type TextBytecodeFile<C = ProgramContext> = BytecodeFile<String, C>;

impl<C> TextBytecodeFile<C>
where
    C: BytecodeContext,
{
    pub fn from_text(text: &str) -> eyre::Result<TextBytecodeFile<C>> {
        let lines = lex_lines(text)?;
        let mut cursor = LineCursor::new(&lines);

        let Some(version) = cursor.next_significant() else {
            return Err(eyre::eyre!(
                "expected `vihaco version {}`",
                super::format::VERSION
            ));
        };
        match &version.kind {
            LineKind::Version(version) => verify_version(*version)?,
            _ => {
                return Err(text_line_error(
                    version,
                    format!("expected `vihaco version {}`", super::format::VERSION),
                ))
            }
        }

        let Some(context_begin) = cursor.next_significant() else {
            return Err(eyre::eyre!("expected `begin context:`"));
        };
        if context_begin.kind != LineKind::BeginContext {
            return Err(text_line_error(context_begin, "expected `begin context:`"));
        }

        let context_start = context_begin.full.end;
        let context_end = consume_context(&mut cursor)?;
        let context = C::from_bytes(text[context_start..context_end].as_bytes())?;

        let Some(section_begin) = cursor.peek_significant() else {
            return Err(eyre::eyre!("expected root section"));
        };
        let root = parse_text_section(
            &mut cursor,
            &context,
            TextSectionParseInfo {
                parent: None,
                begin: section_begin,
            },
        )?;

        if let Some(extra) = cursor.next_significant() {
            return Err(text_line_error(
                extra,
                "unexpected content after root section",
            ));
        }

        Ok(BytecodeFile {
            contents: text.to_string(),
            context: ContextHandle::new(context),
            root,
        })
    }
}

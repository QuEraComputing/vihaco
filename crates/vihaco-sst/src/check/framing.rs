// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-set-independent framing checks.

use vihaco_bytecode::{SSTBlockLayout, scan_sst};

use crate::diagnostic::{Diagnostics, Report};
use crate::source::{SourceFile, Span};

/// One located block (`.global`, `.header`, or `.text`) of an SST file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block<'a> {
    directive: Option<Span>,
    body: Span,
    text: &'a str,
}

impl<'a> Block<'a> {
    fn new(source: &'a SourceFile, layout: Option<&SSTBlockLayout>, anchor: usize) -> Self {
        match layout {
            Some(layout) => {
                let body = Span::from(layout.body.clone());
                Self {
                    directive: Some(layout.directive.clone().into()),
                    body,
                    text: source.slice(body),
                }
            }
            None => Self {
                directive: None,
                body: Span::point(anchor),
                text: "",
            },
        }
    }

    /// The block contents passed to the consumer grammar.
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// The file span of [`text`](Self::text). Add a block-relative offset to
    /// `span().start` to obtain a file offset.
    pub fn span(&self) -> Span {
        self.body
    }

    /// The directive that opened the block, or `None` if the block was omitted.
    pub fn directive(&self) -> Option<Span> {
        self.directive
    }

    /// Whether the file declares this block.
    pub fn is_declared(&self) -> bool {
        self.directive.is_some()
    }
}

/// One located `.section` of an SST file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section<'a> {
    name: &'a str,
    directive: Span,
    name_span: Span,
    header: Block<'a>,
    text: Block<'a>,
}

impl<'a> Section<'a> {
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// The `.section <name>` directive.
    pub fn directive(&self) -> Span {
        self.directive
    }

    /// The name within the directive.
    pub fn name_span(&self) -> Span {
        self.name_span
    }

    /// The `.header` block; empty, anchored after the directive, if omitted.
    pub fn header(&self) -> &Block<'a> {
        &self.header
    }

    /// The `.text` block; empty, anchored after the directive, if omitted.
    pub fn text(&self) -> &Block<'a> {
        &self.text
    }

    /// Where to report a problem with `block` as a whole: its directive, or
    /// the section directive if the block was omitted.
    pub fn block_anchor(&self, block: &Block<'_>) -> Span {
        block.directive().unwrap_or(self.directive)
    }
}

/// The located structure of an SST file, recovered even when malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framing<'a> {
    /// The `.global` block; empty, anchored after the version line, if omitted.
    pub global: Block<'a>,
    /// Sections in file order, including duplicates.
    pub sections: Vec<Section<'a>>,
    /// The version line, or an empty span at the start of the file.
    pub version: Span,
}

impl<'a> Framing<'a> {
    /// The first section named `name`.
    pub fn section(&self, name: &str) -> Option<&Section<'a>> {
        self.sections.iter().find(|section| section.name == name)
    }
}

/// The offset just past the line containing `offset`.
fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index + 1)
}

/// Locate the file's blocks and record every framing problem in `out`.
pub fn scan<'a>(source: &'a SourceFile, out: &mut Diagnostics<'_>) -> Framing<'a> {
    let text = source.text();
    let layout = scan_sst(text);
    for error in &layout.errors {
        let mut diagnostic = out.build_error(error.span.clone(), error.message.clone());
        if let Some((span, message)) = &error.related {
            diagnostic = diagnostic.with_secondary(span.clone(), message.clone());
        }
        out.push(diagnostic);
    }

    let version = layout.version.clone().map_or(Span::point(0), Span::from);
    let global_anchor = layout
        .version
        .as_ref()
        .map_or(0, |version| line_end(text, version.end));
    let sections = layout
        .sections
        .iter()
        .map(|section| {
            let anchor = line_end(text, section.directive.end);
            Section {
                name: source.slice(section.name_span.clone().into()),
                directive: section.directive.clone().into(),
                name_span: section.name_span.clone().into(),
                header: Block::new(source, section.header.as_ref(), anchor),
                text: Block::new(source, section.text.as_ref(), anchor),
            }
        })
        .collect();
    Framing {
        global: Block::new(source, layout.global.as_ref(), global_anchor),
        sections,
        version,
    }
}

/// Report every framing problem in `source`, without parsing block contents.
pub fn check_framing(source: &SourceFile) -> Report {
    let mut out = Diagnostics::new(source);
    scan(source, &mut out);
    out.finish()
}

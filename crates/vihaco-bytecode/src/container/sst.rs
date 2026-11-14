// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Line-based SST framing.
//!
//! An SST file is an assembly-style text file: a version line followed by flat
//! directives. Each directive sits on its own line and opens a block that runs
//! until the next directive (or the end of the file):
//!
//! ```text
//! sst v2
//! .global              // optional, consumer-defined context
//! .section controller  // start a named section
//! .header              // the current section's header
//! .text                // the current section's code
//! .section readout
//! .text
//! ```
//!
//! Directive lines may carry a trailing `//` comment. Every other line belongs
//! to the enclosing block and is passed through to the consumer verbatim.

use std::{collections::BTreeSet, ops::Range};

use eyre::Result;

use super::{ParsedFile, SectionEntry, section::claim_section_name};

/// The SST text format version accepted by this crate (`sst v2`).
pub const VERSION: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LineKind {
    Version(u16),
    Global,
    Section(String),
    Header,
    Text,
    Body,
    Blank,
}

#[derive(Debug, Clone)]
struct SourceLine {
    kind: LineKind,
    full: Range<usize>,
    number: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Global,
    Header,
    Text,
}

/// A section under construction.
struct PendingSection {
    name: String,
    /// End of the `.section` line; empty blocks are anchored here.
    anchor: usize,
    header: Option<Range<usize>>,
    text: Option<Range<usize>>,
}

impl PendingSection {
    fn finish(self) -> SectionEntry {
        let empty = self.anchor..self.anchor;
        SectionEntry {
            name: self.name,
            header: self.header.unwrap_or_else(|| empty.clone()),
            code: self.text.unwrap_or(empty),
        }
    }
}

#[derive(Default)]
struct Builder {
    context: Option<Range<usize>>,
    sections: Vec<PendingSection>,
    /// The block currently being filled and the byte offset where it started.
    open: Option<(BlockKind, usize)>,
}

impl Builder {
    /// Finish the open block (if any) at byte offset `end`.
    fn close_block(&mut self, end: usize) {
        let Some((kind, start)) = self.open.take() else {
            return;
        };
        let range = start..end;
        match kind {
            BlockKind::Global => self.context = Some(range),
            BlockKind::Header => self.current_mut().header = Some(range),
            BlockKind::Text => self.current_mut().text = Some(range),
        }
    }

    fn current_mut(&mut self) -> &mut PendingSection {
        self.sections
            .last_mut()
            .expect("header and text blocks open only inside a section")
    }
}

pub fn parse_file(text: &str) -> Result<ParsedFile> {
    let lines = lex_lines(text)?;
    let mut significant = lines.iter().filter(|line| line.kind != LineKind::Blank);

    let version_line = significant
        .next()
        .ok_or_else(|| eyre::eyre!("expected `sst v{VERSION}`"))?;
    let LineKind::Version(version) = version_line.kind else {
        return Err(line_error(
            version_line,
            format!("expected `sst v{VERSION}`"),
        ));
    };
    verify_version(version)?;
    let after_version = version_line.full.end;

    let mut builder = Builder::default();
    let mut names = BTreeSet::new();

    for line in significant {
        match &line.kind {
            // A later `sst ...` line is ordinary block content.
            LineKind::Version(_) | LineKind::Body => {
                if builder.open.is_none() {
                    let expected = if builder.sections.is_empty() {
                        "`.global` or `.section <name>`"
                    } else {
                        "`.header`, `.text`, or `.section <name>`"
                    };
                    return Err(line_error(
                        line,
                        format!("unexpected content; expected {expected}"),
                    ));
                }
            }
            LineKind::Blank => {}
            LineKind::Global => {
                let global_open = matches!(builder.open, Some((BlockKind::Global, _)));
                if builder.context.is_some() || global_open {
                    return Err(line_error(line, "duplicate `.global` block"));
                }
                if !builder.sections.is_empty() {
                    return Err(line_error(
                        line,
                        "`.global` must come before any `.section`",
                    ));
                }
                builder.open = Some((BlockKind::Global, line.full.end));
            }
            LineKind::Section(name) => {
                builder.close_block(line.full.start);
                claim_section_name(&mut names, name).map_err(|err| line_error(line, err))?;
                builder.sections.push(PendingSection {
                    name: name.clone(),
                    anchor: line.full.end,
                    header: None,
                    text: None,
                });
            }
            LineKind::Header | LineKind::Text => {
                let (kind, directive) = if line.kind == LineKind::Header {
                    (BlockKind::Header, ".header")
                } else {
                    (BlockKind::Text, ".text")
                };
                builder.close_block(line.full.start);
                let Some(section) = builder.sections.last() else {
                    return Err(line_error(
                        line,
                        format!("`{directive}` must follow a `.section <name>` directive"),
                    ));
                };
                let existing = match kind {
                    BlockKind::Header => &section.header,
                    _ => &section.text,
                };
                if existing.is_some() {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` declares duplicate `{directive}`",
                            section.name
                        ),
                    ));
                }
                builder.open = Some((kind, line.full.end));
            }
        }
    }
    builder.close_block(text.len());

    Ok(ParsedFile {
        context: builder.context.unwrap_or(after_version..after_version),
        sections: builder
            .sections
            .into_iter()
            .map(PendingSection::finish)
            .collect(),
    })
}

fn verify_version(version: u16) -> Result<()> {
    if version != VERSION {
        return Err(eyre::eyre!(
            "unsupported sst version {} (expected {})",
            version,
            VERSION
        ));
    }
    Ok(())
}

fn lex_lines(text: &str) -> Result<Vec<SourceLine>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let full = start..start + line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        lines.push(SourceLine {
            kind: classify_line(content).map_err(|err| eyre::eyre!("line {}: {err}", index + 1))?,
            full: full.clone(),
            number: index + 1,
        });
        start = full.end;
    }
    Ok(lines)
}

/// Classify one line (without its line terminator).
///
/// Only the structural directives and the version line are reserved; every
/// other non-blank line is block content owned by the consumer grammar.
fn classify_line(line: &str) -> Result<LineKind> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(LineKind::Blank);
    }

    // Directives may end with a `//` comment.
    let directive = trimmed
        .split_once("//")
        .map_or(trimmed, |(code, _)| code)
        .trim_end();
    let (keyword, rest) = directive
        .split_once(char::is_whitespace)
        .map_or((directive, ""), |(keyword, rest)| (keyword, rest.trim()));

    match keyword {
        "sst" => Ok(rest
            .strip_prefix('v')
            .and_then(|version| version.parse::<u16>().ok())
            .map_or(LineKind::Body, LineKind::Version)),
        ".global" => expect_no_argument(".global", rest, LineKind::Global),
        ".header" => expect_no_argument(".header", rest, LineKind::Header),
        ".text" => expect_no_argument(".text", rest, LineKind::Text),
        ".section" => {
            if rest.is_empty() {
                return Err(eyre::eyre!("`.section` expects a section name"));
            }
            if rest.contains(char::is_whitespace) {
                return Err(eyre::eyre!(
                    "`.section` expects a single name, found `{rest}`"
                ));
            }
            Ok(LineKind::Section(rest.to_owned()))
        }
        _ => Ok(LineKind::Body),
    }
}

fn expect_no_argument(keyword: &str, rest: &str, kind: LineKind) -> Result<LineKind> {
    if rest.is_empty() {
        Ok(kind)
    } else {
        Err(eyre::eyre!(
            "`{keyword}` takes no arguments, found `{rest}`"
        ))
    }
}

fn line_error(line: &SourceLine, message: impl std::fmt::Display) -> eyre::Report {
    eyre::eyre!("line {}: {}", line.number, message)
}

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
//!
//! [`scan`] locates every directive and block and collects all framing errors;
//! [`parse_file`] is the fail-fast form used by `SSTFile::from_text`.

use std::{
    collections::{BTreeSet, HashMap},
    ops::Range,
};

use eyre::Result;

use super::{ParsedFile, SectionEntry, section::claim_section_name};

/// The SST text format version accepted by this crate (`sst v2`).
pub const VERSION: u16 = 2;

/// A framing problem found by [`scan`], located in the source file.
///
/// Spans are byte ranges into the scanned text. [`Display`](std::fmt::Display)
/// renders the message as [`SSTFile::from_text`](crate::SSTFile::from_text)
/// reports it, for example ``line 3: duplicate section `cpu` ``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SSTFramingError {
    /// The offending text.
    pub span: Range<usize>,
    /// The 1-based line containing the start of [`span`](Self::span).
    pub line: usize,
    /// A human-readable description without a location prefix.
    pub message: String,
    /// An earlier location the problem relates to, such as the first
    /// definition of a duplicated section, with a short description.
    pub related: Option<(Range<usize>, String)>,
    /// Whether the location is part of the `SSTFile` error message.
    prefix_line: bool,
    /// Line-classification errors are reported before structural ones.
    lexical: bool,
}

impl std::fmt::Display for SSTFramingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.prefix_line {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

/// One directive-opened block: the directive itself and the block contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SSTBlockLayout {
    /// The directive text, such as `.header`, excluding any trailing comment.
    pub directive: Range<usize>,
    /// The block contents: from the line after the directive to the next
    /// directive or the end of the file.
    pub body: Range<usize>,
}

/// One `.section` and its blocks, located in the source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SSTSectionLayout {
    /// The section name as written (possibly invalid if the directive was
    /// malformed; see [`SSTLayout::errors`]).
    pub name: String,
    /// The `.section <name>` directive, excluding any trailing comment.
    pub directive: Range<usize>,
    /// The name within the directive (empty, at the directive end, if missing).
    pub name_span: Range<usize>,
    /// The section's `.header` block, if declared.
    pub header: Option<SSTBlockLayout>,
    /// The section's `.text` block, if declared.
    pub text: Option<SSTBlockLayout>,
}

/// The located framing of an SST file, produced by [`scan`] even when the file
/// is malformed.
///
/// Recovery keeps scanning after each problem so that all framing errors are
/// reported: a duplicate section is still listed, while a misplaced or
/// duplicate block is skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SSTLayout {
    /// The `sst v<N>` version directive, if the file starts with one.
    pub version: Option<Range<usize>>,
    /// The `.global` block, if declared.
    pub global: Option<SSTBlockLayout>,
    /// All sections, in file order, including duplicates.
    pub sections: Vec<SSTSectionLayout>,
    /// Every framing problem, in file order.
    pub errors: Vec<SSTFramingError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LineKind {
    Version(u16),
    Global,
    Section {
        name: String,
        name_span: Range<usize>,
        valid: bool,
    },
    Header,
    Text,
    Body,
    Blank,
}

#[derive(Debug, Clone)]
struct SourceLine {
    kind: LineKind,
    full: Range<usize>,
    /// The directive text without its trailing comment, trimmed.
    directive: Range<usize>,
    number: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Global,
    Header,
    Text,
    /// A misplaced or duplicate block whose contents are skipped.
    Discard,
}

struct Builder {
    layout: SSTLayout,
    /// The block currently being filled: its kind, directive, and body start.
    open: Option<(BlockKind, Range<usize>, usize)>,
}

impl Builder {
    /// Finish the open block (if any) at byte offset `end`.
    fn close_block(&mut self, end: usize) {
        let Some((kind, directive, start)) = self.open.take() else {
            return;
        };
        let block = SSTBlockLayout {
            directive,
            body: start..end,
        };
        match kind {
            BlockKind::Global => self.layout.global = Some(block),
            BlockKind::Header => self.current_mut().header = Some(block),
            BlockKind::Text => self.current_mut().text = Some(block),
            BlockKind::Discard => {}
        }
    }

    fn current_mut(&mut self) -> &mut SSTSectionLayout {
        self.layout
            .sections
            .last_mut()
            .expect("header and text blocks open only inside a section")
    }

    fn error(&mut self, line: &SourceLine, span: Range<usize>, message: impl Into<String>) {
        self.layout.errors.push(SSTFramingError {
            span,
            line: line.number,
            message: message.into(),
            related: None,
            prefix_line: true,
            lexical: false,
        });
    }

    /// Attach a related location to the most recent error.
    fn related(&mut self, span: Range<usize>, message: &str) {
        if let Some(error) = self.layout.errors.last_mut() {
            error.related = Some((span, message.to_owned()));
        }
    }
}

/// Scan the framing of an SST file, recovering after each problem.
///
/// This is the located, multi-error counterpart of
/// [`SSTFile::from_text`](crate::SSTFile::from_text): it does not stop at the
/// first problem and does not parse the global context or block contents. A
/// file is well framed exactly when [`SSTLayout::errors`] is empty.
pub fn scan(text: &str) -> SSTLayout {
    let mut builder = Builder {
        layout: SSTLayout {
            version: None,
            global: None,
            sections: Vec::new(),
            errors: Vec::new(),
        },
        open: None,
    };
    let lines = lex_lines(text, &mut builder.layout.errors);
    let mut significant = lines
        .iter()
        .filter(|line| line.kind != LineKind::Blank)
        .peekable();

    match significant.peek().copied() {
        None => builder.layout.errors.push(SSTFramingError {
            span: text.len()..text.len(),
            line: text.split_inclusive('\n').count().max(1),
            message: format!("expected `sst v{VERSION}`"),
            related: None,
            prefix_line: false,
            lexical: false,
        }),
        Some(line) => {
            if let LineKind::Version(version) = line.kind {
                builder.layout.version = Some(line.directive.clone());
                if version != VERSION {
                    builder.layout.errors.push(SSTFramingError {
                        span: line.directive.clone(),
                        line: line.number,
                        message: format!("unsupported sst version {version} (expected {VERSION})"),
                        related: None,
                        prefix_line: false,
                        lexical: false,
                    });
                }
                significant.next();
            } else {
                builder.error(
                    line,
                    line.directive.clone(),
                    format!("expected `sst v{VERSION}`"),
                );
            }
        }
    }

    let mut first_section = HashMap::<String, Range<usize>>::new();
    let mut names = BTreeSet::new();
    let mut first_global = None;
    // Report one error per run of stray lines rather than one per line.
    let mut in_stray_run = false;

    for line in significant {
        if !matches!(line.kind, LineKind::Version(_) | LineKind::Body) {
            in_stray_run = false;
        }
        match &line.kind {
            // A later `sst ...` line is ordinary block content.
            LineKind::Version(_) | LineKind::Body => {
                if builder.open.is_none() && !in_stray_run {
                    in_stray_run = true;
                    let expected = if builder.layout.sections.is_empty() {
                        "`.global` or `.section <name>`"
                    } else {
                        "`.header`, `.text`, or `.section <name>`"
                    };
                    builder.error(
                        line,
                        line.directive.clone(),
                        format!("unexpected content; expected {expected}"),
                    );
                }
            }
            LineKind::Blank => {}
            LineKind::Global => {
                builder.close_block(line.full.start);
                let kind = if let Some(first) = first_global.clone() {
                    builder.error(line, line.directive.clone(), "duplicate `.global` block");
                    builder.related(first, "first `.global` block here");
                    BlockKind::Discard
                } else if !builder.layout.sections.is_empty() {
                    builder.error(
                        line,
                        line.directive.clone(),
                        "`.global` must come before any `.section`",
                    );
                    BlockKind::Discard
                } else {
                    first_global = Some(line.directive.clone());
                    BlockKind::Global
                };
                builder.open = Some((kind, line.directive.clone(), line.full.end));
            }
            LineKind::Section {
                name,
                name_span,
                valid,
            } => {
                builder.close_block(line.full.start);
                if *valid {
                    if let Err(err) = claim_section_name(&mut names, name) {
                        builder.error(line, name_span.clone(), err.to_string());
                        if let Some(first) = first_section.get(name).cloned() {
                            builder.related(first, "first defined here");
                        }
                    } else {
                        first_section.insert(name.clone(), name_span.clone());
                    }
                }
                builder.layout.sections.push(SSTSectionLayout {
                    name: name.clone(),
                    directive: line.directive.clone(),
                    name_span: name_span.clone(),
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
                let open = match builder.layout.sections.last() {
                    None => {
                        builder.error(
                            line,
                            line.directive.clone(),
                            format!("`{directive}` must follow a `.section <name>` directive"),
                        );
                        BlockKind::Discard
                    }
                    Some(section) => {
                        let existing = match kind {
                            BlockKind::Header => &section.header,
                            _ => &section.text,
                        };
                        if let Some(existing) = existing {
                            let first = existing.directive.clone();
                            let message = format!(
                                "section `{}` declares duplicate `{directive}`",
                                section.name
                            );
                            builder.error(line, line.directive.clone(), message);
                            builder.related(first, &format!("first `{directive}` here"));
                            BlockKind::Discard
                        } else {
                            kind
                        }
                    }
                };
                builder.open = Some((open, line.directive.clone(), line.full.end));
            }
        }
    }
    builder.close_block(text.len());

    builder.layout.errors.sort_by_key(|error| error.span.start);
    builder.layout
}

/// Parse the framing of an SST file, failing on the first problem.
///
/// Line-classification problems (malformed directives) are reported before
/// structural ones, each in file order.
pub fn parse_file(text: &str) -> Result<ParsedFile> {
    let layout = scan(text);
    if let Some(error) = layout
        .errors
        .iter()
        .find(|error| error.lexical)
        .or_else(|| layout.errors.first())
    {
        return Err(eyre::eyre!("{error}"));
    }

    // The end of the line containing `offset`, where empty blocks are anchored.
    let line_end = |offset: usize| {
        text[offset..]
            .find('\n')
            .map_or(text.len(), |index| offset + index + 1)
    };
    let after_version = layout
        .version
        .map(|version| line_end(version.end))
        .unwrap_or_default();
    Ok(ParsedFile {
        context: layout
            .global
            .map_or(after_version..after_version, |global| global.body),
        sections: layout
            .sections
            .into_iter()
            .map(|section| {
                let anchor = line_end(section.directive.end);
                let empty = anchor..anchor;
                SectionEntry {
                    name: section.name,
                    header: section
                        .header
                        .map_or_else(|| empty.clone(), |block| block.body),
                    code: section.text.map_or(empty, |block| block.body),
                }
            })
            .collect(),
    })
}

fn lex_lines(text: &str, errors: &mut Vec<SSTFramingError>) -> Vec<SourceLine> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let full = start..start + line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        let classified = classify_line(content);
        let shift = |range: Range<usize>| full.start + range.start..full.start + range.end;
        if let Some((span, message)) = classified.error {
            errors.push(SSTFramingError {
                span: shift(span),
                line: index + 1,
                message,
                related: None,
                prefix_line: true,
                lexical: true,
            });
        }
        let kind = match classified.kind {
            LineKind::Section {
                name,
                name_span,
                valid,
            } => LineKind::Section {
                name,
                name_span: shift(name_span),
                valid,
            },
            kind => kind,
        };
        lines.push(SourceLine {
            kind,
            directive: shift(classified.directive),
            full: full.clone(),
            number: index + 1,
        });
        start = full.end;
    }
    lines
}

/// A classified line with line-relative spans and an optional problem.
struct Classified {
    kind: LineKind,
    directive: Range<usize>,
    error: Option<(Range<usize>, String)>,
}

/// The byte range of `part` within `whole`; `part` must be a subslice.
fn range_in(whole: &str, part: &str) -> Range<usize> {
    let start = part.as_ptr() as usize - whole.as_ptr() as usize;
    start..start + part.len()
}

/// Classify one line (without its line terminator).
///
/// Only the structural directives and the version line are reserved; every
/// other non-blank line is block content owned by the consumer grammar. A
/// malformed directive is still classified as that directive so scanning can
/// continue.
fn classify_line(line: &str) -> Classified {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Classified {
            kind: LineKind::Blank,
            directive: 0..0,
            error: None,
        };
    }

    // Directives may end with a `//` comment.
    let directive = trimmed
        .split_once("//")
        .map_or(trimmed, |(code, _)| code)
        .trim_end();
    let (keyword, rest) = directive
        .split_once(char::is_whitespace)
        .map_or((directive, ""), |(keyword, rest)| (keyword, rest.trim()));
    let directive_span = range_in(line, directive);
    let rest_span = if rest.is_empty() {
        directive_span.end..directive_span.end
    } else {
        range_in(line, rest)
    };
    let classified = |kind, error| Classified {
        kind,
        directive: directive_span.clone(),
        error,
    };
    let no_argument = |keyword: &str, kind| {
        let error = (!rest.is_empty()).then(|| {
            (
                rest_span.clone(),
                format!("`{keyword}` takes no arguments, found `{rest}`"),
            )
        });
        classified(kind, error)
    };

    match keyword {
        "sst" => classified(
            rest.strip_prefix('v')
                .and_then(|version| version.parse::<u16>().ok())
                .map_or(LineKind::Body, LineKind::Version),
            None,
        ),
        ".global" => no_argument(".global", LineKind::Global),
        ".header" => no_argument(".header", LineKind::Header),
        ".text" => no_argument(".text", LineKind::Text),
        ".section" => {
            let error = if rest.is_empty() {
                Some((
                    directive_span.clone(),
                    "`.section` expects a section name".to_owned(),
                ))
            } else if rest.contains(char::is_whitespace) {
                Some((
                    rest_span.clone(),
                    format!("`.section` expects a single name, found `{rest}`"),
                ))
            } else {
                None
            };
            let valid = error.is_none();
            classified(
                LineKind::Section {
                    name: rest.to_owned(),
                    name_span: rest_span.clone(),
                    valid,
                },
                error,
            )
        }
        _ => classified(LineKind::Body, None),
    }
}

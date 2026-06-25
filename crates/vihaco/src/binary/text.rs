// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, ops::Range};

use chumsky::{error::Simple, extra, prelude::*};
use eyre::Result;

use crate::binary::common::validate_local_section_name;

use super::{
    format::VERSION,
    section::{SectionNode, SectionPath},
};

type ParseExtra<'src> = extra::Err<Simple<'src, char>>;
const ROOT_SECTION_NAME: &str = "/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LineKind {
    Version(u16),
    BeginContext,
    EndContext,
    BeginSection(String),
    EndSection(String),
    BeginHeader,
    EndHeader,
    BeginBytecode,
    EndBytecode,
    Body,
    Blank,
}

#[derive(Debug, Clone)]
pub(super) struct SourceLine {
    pub(super) kind: LineKind,
    pub(super) full: Range<usize>,
    pub(super) indent: usize,
    pub(super) number: usize,
}

pub(super) fn verify_version(version: u16) -> Result<()> {
    if version != VERSION {
        return Err(eyre::eyre!(
            "unsupported bytecode version {} (expected {})",
            version,
            VERSION
        ));
    }
    Ok(())
}

pub fn parse_instruction_stream<'src, I>(text: &'src str) -> Result<Vec<I>>
where
    I: crate::traits::Instruction + vihaco_parser_core::Parse<'src>,
{
    use chumsky::IterParser as _;

    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    <I as vihaco_parser_core::Parse<'src>>::parser()
        .padded()
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .parse(text)
        .into_result()
        .map_err(|errors| eyre::eyre!("failed to parse text instruction stream: {:?}", errors))
}

fn parse_line(line: &str) -> Result<LineKind> {
    line_parser()
        .parse(line)
        .into_result()
        .map_err(format_parse_errors)
}

fn line_parser<'src>() -> impl Parser<'src, &'src str, LineKind, ParseExtra<'src>> {
    let space = one_of(" \t").repeated().at_least(1);
    let name = any()
        .filter(|c: &char| !c.is_whitespace() && !matches!(*c, ':' | '.'))
        .repeated()
        .at_least(1)
        .collect::<String>();

    let version = just("vhbc")
        .ignore_then(text::int(10).try_map(|version: &str, span| {
            version.parse::<u16>().map_err(|_| Simple::new(None, span))
        }))
        .map(LineKind::Version);

    let begin_context = just("@>").to(LineKind::BeginContext);
    let end_context = just("<@").to(LineKind::EndContext);

    let begin_section = just("~>")
        .ignore_then(space)
        .ignore_then(name)
        .then_ignore(just(':'))
        .map(LineKind::BeginSection);
    let end_section = just("<~")
        .ignore_then(space)
        .ignore_then(name)
        .then_ignore(just('.'))
        .map(LineKind::EndSection);

    let begin_header = just("!>").to(LineKind::BeginHeader);
    let end_header = just("<!").to(LineKind::EndHeader);

    let begin_bytecode = just("^>").to(LineKind::BeginBytecode);
    let end_bytecode = just("<^").to(LineKind::EndBytecode);

    let blank = one_of(" \t").repeated().to(LineKind::Blank);
    let body = any().repeated().at_least(1).to(LineKind::Body);

    just(' ')
        .repeated()
        .ignore_then(choice((
            version,
            begin_context,
            end_context,
            begin_section,
            end_section,
            begin_header,
            end_header,
            begin_bytecode,
            end_bytecode,
            body,
            blank,
        )))
        .then_ignore(end())
}

fn format_parse_errors(errors: Vec<Simple<'_, char>>) -> eyre::Report {
    let error = errors
        .into_iter()
        .next()
        .map(|error| format!("{error:?}"))
        .unwrap_or_else(|| "unknown parse error".to_string());
    eyre::eyre!("{error}")
}

pub(super) fn lex_lines(text: &str) -> Result<Vec<SourceLine>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let full_end = start + line.len();
        let mut content_end = full_end;
        if line.ends_with('\n') {
            content_end -= 1;
            if text.as_bytes().get(content_end.wrapping_sub(1)) == Some(&b'\r') {
                content_end -= 1;
            }
        }

        let indent = text[start..content_end]
            .bytes()
            .take_while(|byte| *byte == b'\t')
            .count();
        let line_start = start + indent;
        lines.push(SourceLine {
            kind: parse_line(&text[line_start..content_end])
                .map_err(|err| eyre::eyre!("line {}: {err}", index + 1))?,
            full: start..full_end,
            indent,
            number: index + 1,
        });
        start = full_end;
    }

    if start < text.len() {
        let number = lines.len() + 1;
        let indent = text[start..]
            .bytes()
            .take_while(|byte| *byte == b'\t')
            .count();
        let line_start = start + indent;
        lines.push(SourceLine {
            kind: parse_line(&text[line_start..])
                .map_err(|err| eyre::eyre!("line {}: {err}", number))?,
            full: start..text.len(),
            indent,
            number,
        });
    }

    Ok(lines)
}

pub(super) struct LineCursor<'a> {
    lines: &'a [SourceLine],
    next: usize,
}

impl<'a> LineCursor<'a> {
    pub(super) fn new(lines: &'a [SourceLine]) -> Self {
        Self { lines, next: 0 }
    }

    pub(super) fn peek_significant(&self) -> Option<&'a SourceLine> {
        self.lines[self.next..]
            .iter()
            .find(|line| line.kind != LineKind::Blank)
    }

    pub(super) fn next_significant(&mut self) -> Option<&'a SourceLine> {
        while let Some(line) = self.lines.get(self.next) {
            self.next += 1;
            if line.kind != LineKind::Blank {
                return Some(line);
            }
        }
        None
    }
}

pub(super) fn consume_context(cursor: &mut LineCursor<'_>) -> Result<usize> {
    while let Some(line) = cursor.next_significant() {
        if line.kind == LineKind::EndContext {
            ensure_indent(line, 0, "context end")?;
            return Ok(line.full.start);
        }
    }

    Err(eyre::eyre!("unterminated context; expected `<@`"))
}

pub(super) struct TextSectionParseInfo<'a> {
    pub(super) parent: Option<ParentSection<'a>>,
    pub(super) begin: &'a SourceLine,
}

#[derive(Clone, Copy)]
pub(super) struct ParentSection<'a> {
    pub(super) path: &'a SectionPath,
    pub(super) indent: usize,
}

pub(super) fn parse_section(
    cursor: &mut LineCursor<'_>,
    info: TextSectionParseInfo<'_>,
) -> Result<SectionNode> {
    let TextSectionParseInfo { parent, begin } = info;
    let section_name = match &begin.kind {
        LineKind::BeginSection(name) => name.clone(),
        _ => return Err(line_error(begin, "expected section")),
    };

    let consumed_begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected section"))?;
    debug_assert_eq!(consumed_begin.full, begin.full);

    let section_indent = begin.indent;
    match parent {
        Some(parent) => ensure_indent(begin, parent.indent + 1, "child section")?,
        None => {
            ensure_indent(begin, 0, "root section")?;
            if section_name != ROOT_SECTION_NAME {
                return Err(line_error(
                    begin,
                    format!("root section must be named `{ROOT_SECTION_NAME}`"),
                ));
            }
        }
    }

    let path = match parent {
        Some(parent) => {
            validate_local_section_name(parent.path, &section_name)?;
            parent.path.child(section_name.clone())
        }
        None => SectionPath::root(),
    };

    let mut header = None;
    let mut bytecode = None;
    let mut children = Vec::new();
    let mut child_names = BTreeSet::new();

    loop {
        let Some(line) = cursor.peek_significant() else {
            return Err(line_error(begin, "unterminated section"));
        };

        match &line.kind {
            LineKind::EndSection(name) => {
                ensure_indent(line, section_indent, "section end")?;
                if name != &section_name {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` ended with mismatched marker `{}`",
                            section_name, name
                        ),
                    ));
                }
                let end = cursor.next_significant().expect("peeked line must exist");
                let fallback = end.full.start..end.full.start;
                return Ok(SectionNode {
                    path,
                    section: begin.full.start..end.full.end,
                    header: header.unwrap_or_else(|| fallback.clone()),
                    bytecode: bytecode.unwrap_or(fallback),
                    children,
                });
            }
            LineKind::BeginHeader => {
                ensure_indent(line, section_indent + 1, "header")?;
                if header.is_some() {
                    return Err(line_error(
                        line,
                        format!("section `{}` declares duplicate header", path),
                    ));
                }
                header = Some(consume_named_block(
                    cursor,
                    &section_name,
                    "header",
                    section_indent + 1,
                    |kind| matches!(kind, LineKind::EndHeader),
                )?);
            }
            LineKind::BeginBytecode => {
                ensure_indent(line, section_indent + 1, "bytecode")?;
                if bytecode.is_some() {
                    return Err(line_error(
                        line,
                        format!("section `{}` declares duplicate bytecode", path),
                    ));
                }
                bytecode = Some(consume_named_block(
                    cursor,
                    &section_name,
                    "bytecode",
                    section_indent + 1,
                    |kind| matches!(kind, LineKind::EndBytecode),
                )?);
            }
            LineKind::BeginSection(child_name) => {
                ensure_indent(line, section_indent + 1, "child section")?;
                validate_local_section_name(&path, child_name)?;
                if !child_names.insert(child_name.clone()) {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` declares duplicate child `{}`",
                            path, child_name
                        ),
                    ));
                }
                let child = parse_section(
                    cursor,
                    TextSectionParseInfo {
                        parent: Some(ParentSection {
                            path: &path,
                            indent: section_indent,
                        }),
                        begin: line,
                    },
                )?;
                children.push(child);
            }
            LineKind::Blank => {
                cursor.next_significant();
            }
            _ => {
                return Err(line_error(
                    line,
                    format!("unexpected content in section `{}`", path),
                ));
            }
        }
    }
}

fn consume_named_block(
    cursor: &mut LineCursor<'_>,
    section_name: &str,
    label: &str,
    block_indent: usize,
    end_name: impl Fn(&LineKind) -> bool,
) -> Result<Range<usize>> {
    let begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected `{label}` block in section `{section_name}`"))?;
    let start = begin.full.end;

    while let Some(line) = cursor.next_significant() {
        if end_name(&line.kind) {
            ensure_indent(line, block_indent, label)?;
            return Ok(start..line.full.start);
        }
    }

    Err(line_error(
        begin,
        format!("unterminated {label}; expected `{}`", end_marker(label)),
    ))
}

fn ensure_indent(line: &SourceLine, expected: usize, label: &str) -> Result<()> {
    if line.indent != expected {
        return Err(line_error(
            line,
            format!(
                "{label} must be indented with {expected} tab(s), found {}",
                line.indent
            ),
        ));
    }
    Ok(())
}

fn end_marker(label: &str) -> &'static str {
    match label {
        "header" => "<!",
        "bytecode" => "<^",
        _ => "end marker",
    }
}

pub(super) fn line_error(line: &SourceLine, message: impl std::fmt::Display) -> eyre::Report {
    eyre::eyre!("line {}: {}", line.number, message)
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, ops::Range};

use chumsky::{error::Simple, extra, prelude::*};
use eyre::Result;

use crate::binary::common::validate_local_section_name;

use super::{
    context::BytecodeContext,
    format::VERSION,
    section::{SectionNode, SectionPath},
};

type ParseExtra<'src> = extra::Err<Simple<'src, char>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LineKind {
    Version(u16),
    BeginContext,
    EndContext,
    BeginSection(String),
    EndSection(String),
    BeginHeader(String),
    EndHeader(String),
    BeginBytecode(String),
    EndBytecode(String),
    Body,
    Blank,
}

#[derive(Debug, Clone)]
pub(super) struct SourceLine {
    pub(super) kind: LineKind,
    pub(super) full: Range<usize>,
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
    let hspace = one_of(" \t").repeated();
    let required_hspace = one_of(" \t").repeated().at_least(1);
    let name = any()
        .filter(|c: &char| !c.is_whitespace() && *c != ':')
        .repeated()
        .at_least(1)
        .collect::<String>();
    let colon = just(':');

    let version = just("vihaco")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("version"))
        .ignore_then(required_hspace.clone())
        .ignore_then(text::int(10).try_map(|version: &str, span| {
            version.parse::<u16>().map_err(|_| Simple::new(None, span))
        }))
        .map(LineKind::Version);

    let begin_context = just("begin")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("context"))
        .then_ignore(colon)
        .to(LineKind::BeginContext);
    let end_context = just("end")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("context"))
        .to(LineKind::EndContext);

    let begin_section = just("begin")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("section"))
        .ignore_then(required_hspace.clone())
        .ignore_then(name.clone())
        .then_ignore(colon)
        .map(LineKind::BeginSection);
    let end_section = just("end")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("section"))
        .ignore_then(required_hspace.clone())
        .ignore_then(name.clone())
        .map(LineKind::EndSection);

    let begin_header = just("begin")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("header"))
        .ignore_then(required_hspace.clone())
        .ignore_then(name.clone())
        .then_ignore(colon)
        .map(LineKind::BeginHeader);
    let end_header = just("end")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("header"))
        .ignore_then(required_hspace.clone())
        .ignore_then(name.clone())
        .map(LineKind::EndHeader);

    let begin_bytecode = just("begin")
        .ignore_then(required_hspace.clone())
        .ignore_then(just("bytecode"))
        .ignore_then(required_hspace.clone())
        .ignore_then(name.clone())
        .then_ignore(colon)
        .map(LineKind::BeginBytecode);
    let end_bytecode = just("end")
        .ignore_then(required_hspace)
        .ignore_then(just("bytecode"))
        .ignore_then(one_of(" \t").repeated().at_least(1))
        .ignore_then(name)
        .map(LineKind::EndBytecode);

    let blank = hspace.clone().to(LineKind::Blank);
    let body = any().repeated().at_least(1).to(LineKind::Body);

    hspace
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

        lines.push(SourceLine {
            kind: parse_line(&text[start..content_end])
                .map_err(|err| eyre::eyre!("line {}: {err}", index + 1))?,
            full: start..full_end,
            number: index + 1,
        });
        start = full_end;
    }

    if start < text.len() {
        let number = lines.len() + 1;
        lines.push(SourceLine {
            kind: parse_line(&text[start..])
                .map_err(|err| eyre::eyre!("line {}: {err}", number))?,
            full: start..text.len(),
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
            return Ok(line.full.start);
        }
    }

    Err(eyre::eyre!("unterminated context; expected `end context`"))
}

pub(super) struct TextSectionParseInfo<'a> {
    pub(super) parent: Option<ParentSection<'a>>,
    pub(super) begin: &'a SourceLine,
}

#[derive(Clone, Copy)]
pub(super) struct ParentSection<'a> {
    pub(super) path: &'a SectionPath,
}

pub(super) fn parse_section<C>(
    cursor: &mut LineCursor<'_>,
    context: &C,
    info: TextSectionParseInfo<'_>,
) -> Result<SectionNode>
where
    C: BytecodeContext,
{
    let TextSectionParseInfo { parent, begin } = info;
    let section_name = match &begin.kind {
        LineKind::BeginSection(name) => name.clone(),
        _ => return Err(line_error(begin, "expected section")),
    };

    let consumed_begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected section"))?;
    debug_assert_eq!(consumed_begin.full, begin.full);

    let path = match parent {
        Some(parent) => {
            validate_local_section_name(parent.path, context, &section_name)?;
            parent
                .path
                .child(find_section_name(context, parent.path, &section_name)?)
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
            LineKind::BeginHeader(name) => {
                if name != &section_name {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` has header marker for `{}`",
                            section_name, name
                        ),
                    ));
                }
                if header.is_some() {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` declares duplicate header",
                            path.display(context)
                        ),
                    ));
                }
                header = Some(consume_named_block(
                    cursor,
                    &section_name,
                    "header",
                    |kind| match kind {
                        LineKind::EndHeader(name) => Some(name.as_str()),
                        _ => None,
                    },
                )?);
            }
            LineKind::BeginBytecode(name) => {
                if name != &section_name {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` has bytecode marker for `{}`",
                            section_name, name
                        ),
                    ));
                }
                if bytecode.is_some() {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` declares duplicate bytecode",
                            path.display(context)
                        ),
                    ));
                }
                bytecode = Some(consume_named_block(
                    cursor,
                    &section_name,
                    "bytecode",
                    |kind| match kind {
                        LineKind::EndBytecode(name) => Some(name.as_str()),
                        _ => None,
                    },
                )?);
            }
            LineKind::BeginSection(child_name) => {
                validate_local_section_name(&path, context, child_name)?;
                if !child_names.insert(child_name.clone()) {
                    return Err(line_error(
                        line,
                        format!(
                            "section `{}` declares duplicate child `{}`",
                            path.display(context),
                            child_name
                        ),
                    ));
                }
                let child = parse_section(
                    cursor,
                    context,
                    TextSectionParseInfo {
                        parent: Some(ParentSection { path: &path }),
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
                    format!("unexpected content in section `{}`", path.display(context)),
                ));
            }
        }
    }
}

fn consume_named_block(
    cursor: &mut LineCursor<'_>,
    section_name: &str,
    label: &str,
    end_name: impl Fn(&LineKind) -> Option<&str>,
) -> Result<Range<usize>> {
    let begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected `begin {label} {section_name}:`"))?;
    let start = begin.full.end;

    while let Some(line) = cursor.next_significant() {
        if let Some(name) = end_name(&line.kind) {
            if name != section_name {
                return Err(line_error(
                    line,
                    format!(
                        "{label} `{}` ended with mismatched marker `{}`",
                        section_name, name
                    ),
                ));
            }
            return Ok(start..line.full.start);
        }
    }

    Err(line_error(
        begin,
        format!("unterminated {label}; expected `end {label} {section_name}`"),
    ))
}

fn find_section_name<C>(context: &C, parent: &SectionPath, name: &str) -> Result<u32>
where
    C: BytecodeContext,
{
    let mut index = 0;
    while let Some(candidate) = context.section_name(index) {
        if candidate == name {
            return Ok(index);
        }
        index = index.checked_add(1).ok_or_else(|| {
            eyre::eyre!(
                "section `{}` name lookup overflowed while resolving `{}`",
                parent.display(context),
                name
            )
        })?;
    }

    Err(eyre::eyre!(
        "section `{}` references missing section name `{}`",
        parent.display(context),
        name
    ))
}

pub(super) fn line_error(line: &SourceLine, message: impl std::fmt::Display) -> eyre::Report {
    eyre::eyre!("line {}: {}", line.number, message)
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, ops::Range};

use chumsky::{error::Simple, extra, prelude::*};
use eyre::Result;

use super::{
    section::{validate_local_section_name, SectionNode, SectionPath},
    ParsedFile,
};

type ParseExtra<'src> = extra::Err<Simple<'src, char>>;
const ROOT_SECTION_NAME: &str = "root";
pub const VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LineKind {
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
struct SourceLine {
    kind: LineKind,
    full: Range<usize>,
    number: usize,
}

pub fn parse_file(text: &str) -> Result<ParsedFile> {
    let lines = lex_lines(text)?;
    let mut cursor = LineCursor::new(&lines);

    let Some(version) = cursor.next_significant() else {
        return Err(eyre::eyre!("expected `sst v{}`", VERSION));
    };
    match &version.kind {
        LineKind::Version(version) => verify_version(*version)?,
        _ => return Err(line_error(version, format!("expected `sst v{}`", VERSION))),
    }

    let Some(context_or_section) = cursor.peek_significant() else {
        return Err(eyre::eyre!("expected `.global:` or root section"));
    };
    let context = match context_or_section.kind {
        LineKind::BeginContext => {
            let context_begin = cursor
                .next_significant()
                .expect("peeked context line must exist");
            let context_start = context_begin.full.end;
            context_start..consume_context(&mut cursor)?
        }
        LineKind::BeginSection(_) => context_or_section.full.start..context_or_section.full.start,
        _ => {
            return Err(line_error(
                context_or_section,
                "expected `.global:` or root section",
            ));
        }
    };

    let Some(section_begin) = cursor.peek_significant() else {
        return Err(eyre::eyre!("expected root section"));
    };
    let root = parse_section(
        &mut cursor,
        SstSectionParseInfo {
            parent: None,
            begin: section_begin,
        },
    )?;

    if let Some(extra) = cursor.next_significant() {
        return Err(line_error(extra, "unexpected content after root section"));
    }

    Ok(ParsedFile { context, root })
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

fn parse_line(line: &str) -> Result<LineKind> {
    line_parser()
        .parse(line)
        .into_result()
        .map_err(format_parse_errors)
}

fn line_parser<'src>() -> impl Parser<'src, &'src str, LineKind, ParseExtra<'src>> {
    let name = any()
        .filter(|c: &char| !c.is_whitespace() && !matches!(*c, ':' | '.' | '(' | ')'))
        .repeated()
        .at_least(1)
        .collect::<String>();

    let version = just("sst")
        .then_ignore(one_of(" \t").repeated().at_least(1))
        .then_ignore(just('v'))
        .ignore_then(text::int(10).try_map(|version: &str, span| {
            version.parse::<u16>().map_err(|_| Simple::new(None, span))
        }))
        .map(LineKind::Version);

    let begin_context = just(".global:").to(LineKind::BeginContext);
    let end_context = just(".global.").to(LineKind::EndContext);

    let begin_section = just(".section(")
        .ignore_then(name)
        .then_ignore(just("):"))
        .map(LineKind::BeginSection);
    let end_section = just(".section(")
        .ignore_then(name)
        .then_ignore(just(")."))
        .map(LineKind::EndSection);

    let begin_header = just(".header(")
        .ignore_then(name)
        .then_ignore(just("):"))
        .map(LineKind::BeginHeader);
    let end_header = just(".header(")
        .ignore_then(name)
        .then_ignore(just(")."))
        .map(LineKind::EndHeader);

    let begin_bytecode = just(".text(")
        .ignore_then(name)
        .then_ignore(just("):"))
        .map(LineKind::BeginBytecode);
    let end_bytecode = just(".text(")
        .ignore_then(name)
        .then_ignore(just(")."))
        .map(LineKind::EndBytecode);

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

fn lex_lines(text: &str) -> Result<Vec<SourceLine>> {
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

        let leading_tabs = text[start..content_end]
            .bytes()
            .take_while(|byte| *byte == b'\t')
            .count();
        let line_start = start + leading_tabs;
        lines.push(SourceLine {
            kind: parse_line(&text[line_start..content_end])
                .map_err(|err| eyre::eyre!("line {}: {err}", index + 1))?,
            full: start..full_end,
            number: index + 1,
        });
        start = full_end;
    }

    if start < text.len() {
        let number = lines.len() + 1;
        let leading_tabs = text[start..]
            .bytes()
            .take_while(|byte| *byte == b'\t')
            .count();
        let line_start = start + leading_tabs;
        lines.push(SourceLine {
            kind: parse_line(&text[line_start..])
                .map_err(|err| eyre::eyre!("line {}: {err}", number))?,
            full: start..text.len(),
            number,
        });
    }

    Ok(lines)
}

struct LineCursor<'a> {
    lines: &'a [SourceLine],
    next: usize,
}

impl<'a> LineCursor<'a> {
    fn new(lines: &'a [SourceLine]) -> Self {
        Self { lines, next: 0 }
    }

    fn peek_significant(&self) -> Option<&'a SourceLine> {
        self.lines[self.next..]
            .iter()
            .find(|line| line.kind != LineKind::Blank)
    }

    fn next_significant(&mut self) -> Option<&'a SourceLine> {
        while let Some(line) = self.lines.get(self.next) {
            self.next += 1;
            if line.kind != LineKind::Blank {
                return Some(line);
            }
        }
        None
    }
}

fn consume_context(cursor: &mut LineCursor<'_>) -> Result<usize> {
    while let Some(line) = cursor.next_significant() {
        if line.kind == LineKind::EndContext {
            return Ok(line.full.start);
        }
    }

    Err(eyre::eyre!("unterminated context; expected `.global.`"))
}

struct SstSectionParseInfo<'a> {
    parent: Option<ParentSection<'a>>,
    begin: &'a SourceLine,
}

#[derive(Clone, Copy)]
struct ParentSection<'a> {
    path: &'a SectionPath,
}

fn parse_section(
    cursor: &mut LineCursor<'_>,
    info: SstSectionParseInfo<'_>,
) -> Result<SectionNode> {
    let SstSectionParseInfo { parent, begin } = info;
    let section_name = match &begin.kind {
        LineKind::BeginSection(name) => name.clone(),
        _ => return Err(line_error(begin, "expected section")),
    };

    let consumed_begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected section"))?;
    debug_assert_eq!(consumed_begin.full, begin.full);

    if parent.is_none() && section_name != ROOT_SECTION_NAME {
        return Err(line_error(
            begin,
            format!("root section must be named `{ROOT_SECTION_NAME}`"),
        ));
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
                ensure_block_marker_name(line, "header", name, &section_name)?;
                if header.is_some() {
                    return Err(line_error(
                        line,
                        format!("section `{}` declares duplicate header", path),
                    ));
                }
                header = Some(consume_named_block(cursor, &section_name, "header")?);
            }
            LineKind::BeginBytecode(name) => {
                ensure_block_marker_name(line, "bytecode", name, &section_name)?;
                if bytecode.is_some() {
                    return Err(line_error(
                        line,
                        format!("section `{}` declares duplicate bytecode", path),
                    ));
                }
                bytecode = Some(consume_named_block(cursor, &section_name, "bytecode")?);
            }
            LineKind::BeginSection(child_name) => {
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
                    SstSectionParseInfo {
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
) -> Result<Range<usize>> {
    let begin = cursor
        .next_significant()
        .ok_or_else(|| eyre::eyre!("expected `{label}` block in section `{section_name}`"))?;
    let start = begin.full.end;

    while let Some(line) = cursor.next_significant() {
        if let Some(marker_name) = block_end_marker_name(&line.kind, label) {
            ensure_block_marker_name(line, label, marker_name, section_name)?;
            return Ok(start..line.full.start);
        }
    }

    Err(line_error(
        begin,
        format!(
            "unterminated {label}; expected `{}`",
            end_marker(label, section_name)
        ),
    ))
}

fn block_end_marker_name<'a>(kind: &'a LineKind, label: &str) -> Option<&'a str> {
    match (label, kind) {
        ("header", LineKind::EndHeader(name)) | ("bytecode", LineKind::EndBytecode(name)) => {
            Some(name)
        }
        _ => None,
    }
}

fn ensure_block_marker_name(
    line: &SourceLine,
    label: &str,
    marker_name: &str,
    section_name: &str,
) -> Result<()> {
    if marker_name != section_name {
        return Err(line_error(
            line,
            format!(
                "{label} marker for section `{section_name}` uses mismatched name `{marker_name}`"
            ),
        ));
    }
    Ok(())
}

fn end_marker(label: &str, section_name: &str) -> String {
    match label {
        "header" => format!(".header({section_name})."),
        "bytecode" => format!(".text({section_name})."),
        _ => "end marker".to_string(),
    }
}

fn line_error(line: &SourceLine, message: impl std::fmt::Display) -> eyre::Report {
    eyre::eyre!("line {}: {}", line.number, message)
}

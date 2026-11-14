// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Located, error-recovering parsers for `.text` streams and header
//! declarations.
//!
//! [`parse_text`] and [`parse_declarations`] accept exactly the inputs that
//! [`ParsedModule::parse_section`](crate::ParsedModule::parse_section) and
//! `HeaderDeclarations` accept, but keep the byte span of every instruction,
//! symbol definition, and declaration, and keep going after a malformed
//! statement so that every problem is reported as a human-readable
//! [`SyntaxError`].
//!
//! Both split their block into statements first (see
//! [`statements`](crate::statements)): one statement per line, continued onto
//! following lines only inside an open bracket. Each statement is parsed on
//! its own, so recovery is per line and every bad line is reported once.
//! Spans are byte ranges relative to the parsed block.

use std::ops::Range;

use chumsky::error::Simple;
use chumsky::prelude::*;
use vihaco_parser::Parse;

use crate::Symbol;
use crate::statements::statements;

/// A parsed value and the byte range of its source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Range<usize>,
}

/// A located, human-readable syntax error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// The offending text, relative to the parsed block. May be empty to mark
    /// a position, such as a missing operand at the end of a line.
    pub span: Range<usize>,
    /// What went wrong, for example ``expected an instruction, found `foo` ``.
    pub message: String,
    /// A short annotation for the span itself, such as `unknown instruction`.
    pub label: String,
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SyntaxError {}

/// A located `.text` stream: instructions in address order, symbol
/// definitions in source order, and every syntax error.
///
/// When [`errors`](Self::errors) is non-empty the other fields hold what could
/// be recovered: malformed statements are skipped and occupy no address.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedText<I> {
    /// Each instruction's span covers the instruction, without a symbol
    /// definition before it on its line or a trailing comment.
    pub code: Vec<Spanned<I>>,
    /// Each symbol's span covers its whole `@name:` definition.
    pub symbols: Vec<Spanned<Symbol>>,
    pub errors: Vec<SyntaxError>,
}

/// Located header declarations and every syntax error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDeclarations<D> {
    /// Each declaration's span covers its statement, without a trailing
    /// comment.
    pub declarations: Vec<Spanned<D>>,
    pub errors: Vec<SyntaxError>,
}

/// The 1-based line and column (counted in characters) of `offset` in `text`.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

/// Characters allowed in a symbol name: see [`crate::symbol_definition`].
pub(crate) fn symbol_name_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            ':' | '@' | ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
        )
}

/// The token starting at `offset`, ending at `limit` at the latest: a run of
/// non-whitespace, non-delimiter characters, or a single delimiter.
fn token_at(text: &str, offset: usize, limit: usize) -> Range<usize> {
    let rest = &text[offset..limit.max(offset)];
    let delimiter = |c: char| matches!(c, ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']');
    let mut chars = rest.char_indices();
    match chars.next() {
        None => offset..offset,
        Some((_, c)) if delimiter(c) => offset..offset + c.len_utf8(),
        Some(_) => {
            let token = &rest[..rest
                .find(|c: char| c.is_whitespace() || delimiter(c))
                .unwrap_or(rest.len())];
            // A record key is reported without its `:`.
            let token = match token.strip_suffix(':') {
                Some(key) if !key.is_empty() => key,
                _ => token,
            };
            offset..offset + token.len()
        }
    }
}

/// The first whitespace-delimited word of `text[span]`.
fn word_at(text: &str, span: &Range<usize>) -> Range<usize> {
    let rest = &text[span.clone()];
    span.start..span.start + rest.find(char::is_whitespace).unwrap_or(rest.len())
}

/// Whether `token` is shaped like a number literal.
fn numeric(token: &str) -> bool {
    let digits = token.strip_prefix('-').unwrap_or(token);
    digits.starts_with(|c: char| c.is_ascii_digit())
        && digits
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'))
}

/// What kind of statement failed to parse, for wording its error.
#[derive(Clone, Copy)]
enum Kind {
    Instruction,
    Declaration,
}

/// Explain why the statement at `statement` failed to parse.
///
/// `parses` reports whether a prefix of the statement is a complete statement
/// by itself, which tells a second statement on the line apart from a
/// malformed operand.
fn statement_error<'s>(
    text: &'s str,
    statement: Range<usize>,
    error: &Simple<'_, char>,
    kind: Kind,
    parses: impl Fn(&'s str) -> bool,
) -> SyntaxError {
    let head = match kind {
        Kind::Instruction => word_at(text, &statement),
        Kind::Declaration => token_at(text, statement.start, statement.end),
    };
    let head_text = &text[head.clone()];
    if let Some((prefix, after)) = complete_prefix(text, &statement, &parses) {
        let (what, label) = match kind {
            Kind::Instruction => ("instruction", "one instruction per line"),
            Kind::Declaration => ("declaration", "one declaration per line"),
        };
        let span = token_at(text, after, statement.end);
        return SyntaxError {
            message: format!(
                "expected end of line after the {what} `{}`, found `{}`",
                &text[prefix],
                &text[span.clone()]
            ),
            span,
            label: label.to_owned(),
        };
    }
    let relative = error.span().into_range();
    let at = (statement.start + relative.start).min(statement.end);
    // A value rejected after lexing (such as an overflowing integer) reports
    // no token but spans the value.
    let rejected = error.found().is_none() && !relative.is_empty();
    let rest = &text[at..statement.end];
    let next = at + rest.len() - rest.trim_start().len();

    if !rejected && next >= statement.end {
        let end = statement.end;
        return match kind {
            Kind::Instruction => SyntaxError {
                span: end..end,
                message: format!("expected more operands for `{head_text}`, found end of line"),
                label: "operand expected here".to_owned(),
            },
            Kind::Declaration => SyntaxError {
                span: end..end,
                message: format!(
                    "expected the rest of the `{head_text}` declaration, found end of line"
                ),
                label: "declaration is incomplete".to_owned(),
            },
        };
    }
    if next < head.end {
        return match kind {
            Kind::Instruction => SyntaxError {
                message: format!("expected an instruction, found `{head_text}`"),
                span: head,
                label: "unknown instruction".to_owned(),
            },
            Kind::Declaration if head_text.starts_with('.') => SyntaxError {
                message: format!("unknown directive `{head_text}`"),
                span: head,
                label: "not a header directive".to_owned(),
            },
            Kind::Declaration => SyntaxError {
                message: format!("expected a header declaration, found `{head_text}`"),
                span: head,
                label: "unknown declaration".to_owned(),
            },
        };
    }
    let token = token_at(text, next, statement.end);
    let span = if rejected {
        next..token
            .end
            .max((statement.start + relative.end).min(statement.end))
    } else {
        token
    };
    let found = &text[span.clone()];
    match (kind, numeric(found)) {
        (Kind::Instruction, true) => SyntaxError {
            message: format!("invalid operand `{found}` for `{head_text}`"),
            span,
            label: "value out of range or malformed".to_owned(),
        },
        (Kind::Instruction, false) => SyntaxError {
            message: format!("unexpected `{found}` in operands of `{head_text}`"),
            span,
            label: "unexpected operand".to_owned(),
        },
        (Kind::Declaration, true) => SyntaxError {
            message: format!("invalid value `{found}` in `{head_text}` declaration"),
            span,
            label: "value out of range or malformed".to_owned(),
        },
        (Kind::Declaration, false) => SyntaxError {
            message: format!("unexpected `{found}` in `{head_text}` declaration"),
            span,
            label: "unexpected token".to_owned(),
        },
    }
}

/// The longest proper prefix of `statement` that ends before whitespace
/// (outside string literals) and parses as a complete statement, and the
/// offset of the text after it.
fn complete_prefix<'s>(
    text: &'s str,
    statement: &Range<usize>,
    parses: impl Fn(&'s str) -> bool,
) -> Option<(Range<usize>, usize)> {
    let line = &text[statement.clone()];
    let mut boundaries = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut previous_space = true;
    for (index, c) in line.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            previous_space = false;
            continue;
        }
        let space = c.is_whitespace();
        if space && !previous_space {
            boundaries.push(index);
        }
        in_string = c == '"';
        previous_space = space;
    }
    boundaries.into_iter().rev().find_map(|end| {
        let prefix = statement.start..statement.start + end;
        let rest = &text[prefix.end..statement.end];
        let after = prefix.end + rest.len() - rest.trim_start().len();
        (after < statement.end && parses(&text[prefix.clone()])).then_some((prefix, after))
    })
}

/// The error chumsky reports furthest into the statement.
fn furthest<'a, 'src>(errors: &'a [Simple<'src, char>]) -> Option<&'a Simple<'src, char>> {
    errors.iter().max_by_key(|error| error.span().start)
}

/// Parse a `.text` stream, keeping spans and recovering after malformed
/// statements.
///
/// Accepts exactly the streams accepted by
/// [`ParsedModule::parse_section`](crate::ParsedModule::parse_section): one
/// statement per line, each an `@name:` symbol definition, an instruction, or
/// a symbol definition followed by an instruction. A symbol written without
/// its trailing `:` is reported as an error but still recorded, so later
/// checks do not report it as missing.
///
/// Line numbers in messages count from 1 at the start of `text`; use
/// [`parse_text_at`] when `text` is part of a larger file.
pub fn parse_text<'src, I>(text: &'src str) -> ParsedText<I>
where
    I: Parse<'src> + 'src,
{
    parse_text_at(text, 1)
}

/// [`parse_text`] for a block that starts on file line `first_line`.
pub fn parse_text_at<'src, I>(text: &'src str, first_line: usize) -> ParsedText<I>
where
    I: Parse<'src> + 'src,
{
    let split = statements(text, first_line);
    let mut parsed = ParsedText {
        code: Vec::new(),
        symbols: Vec::new(),
        errors: split.errors,
    };
    let parser = I::parser().then_ignore(end());
    let parses = |prefix: &'src str| !prefix.is_empty() && !parser.parse(prefix).has_errors();
    for statement in split.statements {
        if !statement.balanced {
            continue;
        }
        let mut start = statement.span.start;
        let end = statement.span.end;
        if let Some(rest) = text[start..end].strip_prefix('@') {
            let name_len = rest.find(|c| !symbol_name_char(c)).unwrap_or(rest.len());
            if name_len == 0 {
                parsed.errors.push(SyntaxError {
                    span: start..start + 1,
                    message: "expected a symbol name after `@`".to_owned(),
                    label: "symbol definitions are written `@name:`".to_owned(),
                });
                continue;
            }
            let name = rest[..name_len].to_owned();
            let colon = rest[name_len..].starts_with(':');
            let span = start..start + 1 + name_len + usize::from(colon);
            if !colon {
                parsed.errors.push(SyntaxError {
                    span: span.clone(),
                    message: format!("expected `:` after symbol name `@{name}`"),
                    label: format!("write `@{name}:` to define this symbol"),
                });
            }
            let Ok(address) = u32::try_from(parsed.code.len()) else {
                parsed.errors.push(SyntaxError {
                    span,
                    message: "instruction stream exceeds u32 address space".to_owned(),
                    label: String::new(),
                });
                continue;
            };
            parsed.symbols.push(Spanned {
                value: Symbol { name, address },
                span: span.clone(),
            });
            if !colon {
                continue;
            }
            let after = &text[span.end..end];
            start = span.end + after.len() - after.trim_start().len();
            if start >= end {
                continue;
            }
        }
        let statement = start..end;
        match parser.parse(&text[statement.clone()]).into_result() {
            Ok(value) => parsed.code.push(Spanned {
                value,
                span: statement,
            }),
            Err(errors) => {
                if let Some(error) = furthest(&errors) {
                    parsed.errors.push(statement_error(
                        text,
                        statement,
                        error,
                        Kind::Instruction,
                        parses,
                    ));
                }
            }
        }
    }
    parsed.errors.sort_by_key(|error| error.span.start);
    parsed
}

/// Parse header declarations, one per statement, keeping spans and
/// recovering after malformed declarations.
///
/// Accepts exactly the headers accepted by `HeaderDeclarations<D>`: one
/// declaration per line, continued onto following lines only inside an open
/// bracket, with `;` comments. Line numbers in messages count from 1 at the
/// start of `text`; use [`parse_declarations_at`] when `text` is part of a
/// larger file.
pub fn parse_declarations<'src, D>(text: &'src str) -> ParsedDeclarations<D>
where
    D: Parse<'src> + 'src,
{
    parse_declarations_at(text, 1)
}

/// [`parse_declarations`] for a block that starts on file line `first_line`.
pub fn parse_declarations_at<'src, D>(text: &'src str, first_line: usize) -> ParsedDeclarations<D>
where
    D: Parse<'src> + 'src,
{
    let split = statements(text, first_line);
    let mut parsed = ParsedDeclarations {
        declarations: Vec::new(),
        errors: split.errors,
    };
    let parser = D::parser().then_ignore(end());
    let parses = |prefix: &'src str| !prefix.is_empty() && !parser.parse(prefix).has_errors();
    for statement in split.statements {
        if !statement.balanced {
            continue;
        }
        let span = statement.span;
        match parser.parse(&text[span.clone()]).into_result() {
            Ok(value) => parsed.declarations.push(Spanned { value, span }),
            Err(errors) => {
                if let Some(error) = furthest(&errors) {
                    parsed.errors.push(statement_error(
                        text,
                        span,
                        error,
                        Kind::Declaration,
                        parses,
                    ));
                }
            }
        }
    }
    parsed.errors.sort_by_key(|error| error.span.start);
    parsed
}

#[cfg(test)]
mod tests;

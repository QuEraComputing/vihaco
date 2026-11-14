// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Located, error-recovering parsers for `.text` streams and header
//! declarations.
//!
//! [`parse_text`] and [`parse_declarations`] accept exactly the inputs that
//! [`ParsedModule::parse_section`](crate::ParsedModule::parse_section) and
//! `HeaderDeclarations` accept, but keep the byte span of every instruction,
//! symbol definition, and declaration, and keep going after a malformed item so
//! that every problem is reported as a human-readable [`SyntaxError`].
//!
//! Spans are byte ranges relative to the parsed block. Recovery is
//! line-based in `.text` (a malformed item skips the rest of its line) and
//! `;`-based in headers (a malformed declaration skips to its terminator).

use std::ops::Range;

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use chumsky::recovery::via_parser;
use vihaco_parser::Parse;

use crate::Symbol;

type E<'src> = extra::Err<Simple<'src, char>>;

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
    /// a position, such as a missing terminator.
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
/// be recovered: malformed items are skipped and occupy no address.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedText<I> {
    pub code: Vec<Spanned<I>>,
    /// Each symbol's span covers its whole `@name:` definition.
    pub symbols: Vec<Spanned<Symbol>>,
    pub errors: Vec<SyntaxError>,
}

/// Located header declarations and every syntax error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDeclarations<D> {
    /// Each declaration's span excludes its `;` terminator.
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

enum TextItem<I> {
    Instruction(I),
    Symbol { name: String, colon: bool },
    Invalid,
}

/// Characters allowed in a symbol name: see [`crate::symbol_definition`].
fn symbol_name_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            ':' | '@' | ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
        )
}

/// Everything up to (excluding) the end of the current line; at least one
/// character. The fallback for a malformed `.text` item.
fn rest_of_line<'src>() -> impl Parser<'src, &'src str, (), E<'src>> + Clone {
    any()
        .and_is(just('\n').not())
        .repeated()
        .at_least(1)
        .ignored()
}

/// Shrink `span` so it does not end in whitespace.
fn trim_end(text: &str, span: Range<usize>) -> Range<usize> {
    let trimmed = text[span.clone()].trim_end();
    span.start..span.start + trimmed.len()
}

/// The token starting at `offset`: a run of non-whitespace, non-delimiter
/// characters, or a single delimiter.
fn token_at(text: &str, offset: usize) -> Range<usize> {
    let rest = &text[offset..];
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

/// The first whitespace-delimited word starting at `offset`.
fn word_at(text: &str, offset: usize) -> Range<usize> {
    let rest = &text[offset..];
    offset..offset + rest.find(char::is_whitespace).unwrap_or(rest.len())
}

/// The end of the non-blank content of the line containing `offset`.
fn line_content_end(text: &str, offset: usize) -> usize {
    let end = text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index);
    trim_end(text, offset..end).end
}

/// What a chumsky error inside an item found, as text.
struct Found {
    span: Range<usize>,
    /// A well-formed token whose value was rejected, such as an overflowing
    /// integer.
    rejected: bool,
}

/// Whether `token` is shaped like a number literal.
fn numeric(token: &str) -> bool {
    let digits = token.strip_prefix('-').unwrap_or(token);
    digits.starts_with(|c: char| c.is_ascii_digit())
        && digits
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'))
}

fn found(text: &str, error: &Simple<'_, char>) -> Found {
    let span = error.span().into_range();
    let start = span.start.min(text.len());
    let at_end = match error.found() {
        None => span.is_empty(),
        Some(c) => c.is_whitespace(),
    };
    if at_end {
        // The end of input, or of an operand list that cannot continue on the
        // next line.
        return Found {
            span: start..start,
            rejected: false,
        };
    }
    // A value rejected after lexing (such as an overflowing integer) reports
    // no token but spans the value; otherwise the span is the first character.
    let token = token_at(text, start);
    let span = start..token.end.max(span.end.min(text.len()));
    Found {
        rejected: numeric(&text[span.clone()]),
        span,
    }
}

/// Explain an error raised while parsing the `.text` item starting at `start`.
fn text_error(text: &str, start: usize, error: &Simple<'_, char>) -> SyntaxError {
    let head = word_at(text, start);
    let head_text = &text[head.clone()];
    if head_text.starts_with('@') {
        return SyntaxError {
            span: start..start + 1,
            message: "expected a symbol name after `@`".to_owned(),
            label: "symbol definitions are written `@name:`".to_owned(),
        };
    }
    let found = found(text, error);
    let line_end = line_content_end(text, start);
    if found.span.start < head.end {
        SyntaxError {
            message: format!("expected an instruction, found `{head_text}`"),
            span: head,
            label: "unknown instruction".to_owned(),
        }
    } else if found.span.start > line_end || found.span.is_empty() {
        SyntaxError {
            span: line_end..line_end,
            message: format!("expected more operands for `{head_text}`, found end of line"),
            label: "operand expected here".to_owned(),
        }
    } else if found.rejected {
        SyntaxError {
            message: format!(
                "invalid operand `{}` for `{head_text}`",
                &text[found.span.clone()]
            ),
            span: found.span,
            label: "value out of range or malformed".to_owned(),
        }
    } else {
        SyntaxError {
            message: format!(
                "unexpected `{}` in operands of `{head_text}`",
                &text[found.span.clone()]
            ),
            span: found.span,
            label: "unexpected operand".to_owned(),
        }
    }
}

/// Assign a chumsky error to the malformed item it belongs to: the last item
/// starting at or before the error.
fn owner(starts: &[usize], error: &Simple<'_, char>) -> Option<usize> {
    let offset = error.span().start;
    starts
        .iter()
        .copied()
        .take_while(|start| *start <= offset)
        .last()
        .or_else(|| starts.first().copied())
}

/// Parse a `.text` stream, keeping spans and recovering after malformed items.
///
/// Accepts exactly the streams accepted by
/// [`ParsedModule::parse_section`](crate::ParsedModule::parse_section). A
/// symbol written without its trailing `:` is reported as an error but still
/// recorded, so later checks do not report it as missing.
pub fn parse_text<'src, I>(text: &'src str) -> ParsedText<I>
where
    I: Parse<'src> + 'src,
{
    let symbol = just('@')
        .ignore_then(
            any()
                .filter(|c: &char| symbol_name_char(*c))
                .repeated()
                .at_least(1)
                .collect::<String>(),
        )
        .then(just(':').or_not().map(|colon| colon.is_some()))
        .map(|(name, colon)| TextItem::Symbol { name, colon });
    let item = symbol
        .or(I::parser().map(TextItem::Instruction))
        .map_with(|item, e| (item, e.span().into_range()))
        .recover_with(via_parser(
            rest_of_line().map_with(|(), e| (TextItem::Invalid, e.span().into_range())),
        ));
    let (items, errors) = crate::skip()
        .ignore_then(item)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(crate::skip())
        .then_ignore(end())
        .parse(text)
        .into_output_errors();

    let mut parsed = ParsedText {
        code: Vec::new(),
        symbols: Vec::new(),
        errors: Vec::new(),
    };
    let items = items.unwrap_or_default();
    let invalid_starts: Vec<usize> = items
        .iter()
        .filter(|(item, _)| matches!(item, TextItem::Invalid))
        .map(|(_, span)| span.start)
        .collect();
    for (item, span) in items {
        match item {
            TextItem::Instruction(value) => parsed.code.push(Spanned {
                value,
                span: trim_end(text, span),
            }),
            TextItem::Symbol { name, colon } => {
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
                    span,
                });
            }
            TextItem::Invalid => {}
        }
    }
    for error in &errors {
        let start = owner(&invalid_starts, error).unwrap_or(error.span().start);
        parsed.errors.push(text_error(text, start, error));
    }
    parsed.errors.sort_by_key(|error| error.span.start);
    parsed
}

enum DeclarationItem<D> {
    Declaration {
        value: D,
        span: Range<usize>,
        terminated: bool,
    },
    Invalid(usize),
}

/// Explain an error raised while parsing the declaration starting at `start`.
fn declaration_error(text: &str, start: usize, error: &Simple<'_, char>) -> SyntaxError {
    let head = token_at(text, start);
    let head_text = &text[head.clone()];
    let found = found(text, error);
    if found.span.start < head.end {
        SyntaxError {
            message: format!("expected a header declaration, found `{head_text}`"),
            span: head,
            label: "unknown declaration".to_owned(),
        }
    } else if found.span.is_empty() {
        let end = trim_end(text, 0..text.len()).end;
        SyntaxError {
            span: end..end,
            message: format!("unexpected end of header in `{head_text}` declaration"),
            label: "declaration is incomplete".to_owned(),
        }
    } else if found.rejected {
        SyntaxError {
            message: format!(
                "invalid value `{}` in `{head_text}` declaration",
                &text[found.span.clone()]
            ),
            span: found.span,
            label: "value out of range or malformed".to_owned(),
        }
    } else {
        SyntaxError {
            message: format!(
                "unexpected `{}` in `{head_text}` declaration",
                &text[found.span.clone()]
            ),
            span: found.span,
            label: "unexpected token".to_owned(),
        }
    }
}

/// Parse `;`-terminated header declarations, keeping spans and recovering
/// after malformed declarations.
///
/// Accepts exactly the headers accepted by `HeaderDeclarations<D>`:
/// declarations separated by whitespace and `//` comments, each terminated by
/// `;`. A declaration missing its `;` is reported but still recorded.
pub fn parse_declarations<'src, D>(text: &'src str) -> ParsedDeclarations<D>
where
    D: Parse<'src> + 'src,
{
    let declaration = D::parser()
        .map_with(|value, e| (value, e.span().into_range()))
        .then(crate::skip().ignore_then(just(';')).or_not())
        .map(|((value, span), terminator)| DeclarationItem::Declaration {
            value,
            span,
            terminated: terminator.is_some(),
        });
    let fallback = any()
        .and_is(just(';').not())
        .repeated()
        .then(just(';').or_not())
        .to_slice()
        .filter(|skipped: &&str| !skipped.is_empty())
        .map_with(|_, e| {
            let span: SimpleSpan = e.span();
            DeclarationItem::Invalid(span.start)
        });
    let (items, errors) = crate::skip()
        .ignore_then(declaration.recover_with(via_parser(fallback)))
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(crate::skip())
        .then_ignore(end())
        .parse(text)
        .into_output_errors();

    let mut parsed = ParsedDeclarations {
        declarations: Vec::new(),
        errors: Vec::new(),
    };
    let mut invalid_starts = Vec::new();
    for item in items.unwrap_or_default() {
        match item {
            DeclarationItem::Declaration {
                value,
                span,
                terminated,
            } => {
                let span = trim_end(text, span);
                if !terminated {
                    let head: &str = &text[token_at(text, span.start)];
                    parsed.errors.push(SyntaxError {
                        span: span.end..span.end,
                        message: format!("expected `;` after `{head}` declaration"),
                        label: "declarations end with `;`".to_owned(),
                    });
                }
                parsed.declarations.push(Spanned { value, span });
            }
            DeclarationItem::Invalid(start) => invalid_starts.push(start),
        }
    }
    for error in &errors {
        let start = owner(&invalid_starts, error).unwrap_or(error.span().start);
        parsed.errors.push(declaration_error(text, start, error));
    }
    parsed.errors.sort_by_key(|error| error.span.start);
    parsed
}

#[cfg(test)]
mod tests;

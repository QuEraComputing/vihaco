// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! chumsky-0.10 combinators for the parsed-syntax shape.
//!
//! [`ParsedModule::parse_section`] is generic over the consumer's instruction
//! type `I` and section-header type `H`.

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco_parser::{Ident, Parse, QuotedString};

use vihaco_bytecode::SSTHeader;
use vihaco_bytecode::SSTSectionView;

use crate::{ParsedModule, Symbol};

enum TextItem<I> {
    Instruction(I),
    Symbol(String),
}

type E<'src> = extra::Err<Simple<'src, char>>;

/// Whitespace and `//`-to-end-of-line comments. Zero-or-more.
pub fn skip<'src>() -> impl Parser<'src, &'src str, (), E<'src>> + Clone {
    vihaco_parser::trivia()
}

/// `"…"` with `\\`, `\"`, `\n`, `\t`, `\r` escape sequences.
pub fn string_literal<'src>() -> impl Parser<'src, &'src str, QuotedString, E<'src>> + Clone {
    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('0').to('\0'),
    )));
    let char_or_escape = choice((
        escape,
        any().and_is(just('"').not()).and_is(just('\\').not()),
    ));
    just('"')
        .ignore_then(char_or_escape.repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(QuotedString)
}

/// `@name` — leading `@` consumed; name is `ident()`-shaped.
pub fn symbol_ref<'src>() -> impl Parser<'src, &'src str, Ident, E<'src>> + Clone {
    just('@').ignore_then(vihaco_parser::ident().map(Ident))
}

/// Block-body helper: parse a sequence of whitespace-separated `i64`s as
/// **flat rows** between an outer `{` … `}` provided by the caller. The body
/// itself is a sequence of `i64`s with any whitespace (including newlines)
/// between them.
///
/// Real usage: `device slm.filling { 0 1 2 3 };`.
pub fn block_i64_flat<'src>() -> impl Parser<'src, &'src str, Vec<i64>, E<'src>> + Clone {
    let item = just('-')
        .or_not()
        .then(text::int(10))
        .to_slice()
        .map(|s: &str| s.parse::<i64>().unwrap());
    // Allow any whitespace (incl. newlines) between numbers and around them.
    let ws = any().filter(|c: &char| c.is_whitespace()).repeated();
    ws.ignore_then(item)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(ws)
}

/// Block-body helper: parse rows of `i64 i64` pairs separated by whitespace.
/// Within a row the two ints are whitespace-separated; rows themselves are
/// also just whitespace-separated (newlines or other ws — the helper doesn't
/// require row alignment).
///
/// Real usage: `device slm.traps { 1 1\n 5 1\n ... };`,
/// `device camera.detect_sites { ... };`.
pub fn block_i64_pairs<'src>() -> impl Parser<'src, &'src str, Vec<(i64, i64)>, E<'src>> + Clone {
    let signed_int = just('-')
        .or_not()
        .then(text::int(10))
        .to_slice()
        .map(|s: &str| s.parse::<i64>().unwrap());
    let inline_ws = any()
        .filter(|c: &char| c.is_whitespace() && *c != '\n')
        .repeated()
        .at_least(1);
    let ws = any().filter(|c: &char| c.is_whitespace()).repeated();

    let pair = signed_int.then_ignore(inline_ws).then(signed_int);
    ws.ignore_then(pair)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(ws)
}

/// `@name:` — a symbol definition. The name excludes `:` so the colon can
/// terminate it.
pub fn symbol_definition<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    just('@')
        .ignore_then(
            any()
                .filter(|c: &char| {
                    !c.is_whitespace()
                        && !matches!(
                            *c,
                            ':' | '@'
                                | ','
                                | ';'
                                | '('
                                | ')'
                                | '{'
                                | '}'
                                | '['
                                | ']'
                                | '"'
                                | '\''
                                | '`'
                        )
                })
                .repeated()
                .at_least(1)
                .collect::<String>(),
        )
        .then_ignore(just(':'))
}

/// A `.text` stream: instructions and `@name:` symbol definitions, separated by
/// whitespace and comments. Layout and indentation carry no meaning.
fn text_stream<'src, I>() -> impl Parser<'src, &'src str, (Vec<I>, Vec<Symbol>), E<'src>>
where
    I: Parse<'src> + 'src,
{
    let item = symbol_definition()
        .map(TextItem::Symbol)
        .or(I::parser().map(TextItem::Instruction));
    skip()
        .ignore_then(item)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(skip())
        .then_ignore(end())
        .try_map(|items, span| {
            let mut code = Vec::new();
            let mut symbols = Vec::new();
            for item in items {
                match item {
                    TextItem::Instruction(instruction) => code.push(instruction),
                    TextItem::Symbol(name) => {
                        let address =
                            u32::try_from(code.len()).map_err(|_| Simple::new(None, span))?;
                        symbols.push(Symbol { name, address });
                    }
                }
            }
            Ok((code, symbols))
        })
}

impl<I, H> ParsedModule<I, H> {
    /// Parse a source section into its consumer instruction representation.
    ///
    /// Symbol names are not checked for uniqueness here; loading validates the
    /// table against the header metadata.
    pub fn parse_section<'src, C>(section: SSTSectionView<'src, C>) -> eyre::Result<Self>
    where
        H: SSTHeader,
        I: vihaco_parser::Parse<'src> + 'src,
    {
        let header = section.parse_header::<H>()?;
        let (code, symbols) = text_stream::<I>()
            .parse(section.text())
            .into_result()
            .map_err(|errors| {
                eyre::eyre!(
                    "failed to parse `.text` of section `{}`: {:?}",
                    section.name(),
                    errors
                )
            })?;
        Ok(Self {
            header,
            code,
            symbols,
        })
    }
}

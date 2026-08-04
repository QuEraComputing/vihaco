// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! chumsky-0.10 combinators for the parsed-syntax shape.
//!
//! `Parse` impls for `ParsedModule`/`ParsedFunction` are generic over the
//! consumer's instruction type `I`, source type `Ty`, and device-header type
//! `H`.

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco_parser::{Ident, Parse, QuotedString};

use vihaco_bytecode::SstHeader;
use vihaco_bytecode::SstSectionView;
use vihaco_parser::SurfaceInstruction;

use crate::{Param, ParsedFunction, ParsedModule};

type E<'src> = extra::Err<Simple<'src, char>>;

/// Whitespace and `//`-to-end-of-line comments. Zero-or-more.
pub fn skip<'src>() -> impl Parser<'src, &'src str, (), E<'src>> + Clone {
    let ws = any().filter(|c: &char| c.is_whitespace()).ignored();
    let line_comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .ignored();
    ws.or(line_comment).repeated().collect::<()>()
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

/// Parse `i64`/`f64`/etc. parameter list. Currently only accepts empty `()`.
fn param_list<'src, Ty>() -> impl Parser<'src, &'src str, Vec<Param<Ty>>, E<'src>> + Clone {
    just('(')
        .padded()
        .then(just(')').padded())
        .map(|_| Vec::new())
}

fn functions<'src, I, Ty>() -> impl Parser<'src, &'src str, Vec<ParsedFunction<I, Ty>>, E<'src>>
where
    I: SurfaceInstruction + Parse<'src> + 'src,
    Ty: Parse<'src> + 'src,
{
    skip()
        .ignore_then(ParsedFunction::<I, Ty>::parser())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(skip())
}

impl<'src, I, Ty> Parse<'src> for ParsedFunction<I, Ty>
where
    I: SurfaceInstruction + Parse<'src> + 'src,
    Ty: Parse<'src> + 'src,
{
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        let return_ty = just("->").padded().ignore_then(Ty::parser()).or_not();
        let body = skip()
            .ignore_then(I::parser())
            .repeated()
            .collect::<Vec<_>>()
            .then_ignore(skip());

        just("fn")
            .padded()
            .ignore_then(just('@'))
            .ignore_then(Ident::parser())
            .then(param_list::<Ty>())
            .then(return_ty)
            .then_ignore(just('{').padded())
            .then(body)
            .then_ignore(just('}').padded())
            .map(|(((name, params), return_ty), body)| ParsedFunction {
                name,
                params,
                return_ty,
                body,
            })
    }
}

impl<I, Ty, H> ParsedModule<I, Ty, H>
where
    I: SurfaceInstruction,
{
    /// Parse a source section into a pre-resolution module.
    pub fn parse_section<'src, C>(section: SstSectionView<'src, C>) -> eyre::Result<Self>
    where
        H: SstHeader,
        I: vihaco_parser::Parse<'src> + 'src,
        Ty: vihaco_parser::Parse<'src> + 'src,
    {
        let header = section.parse_header::<H>()?;
        let text = section.sst();
        let functions = functions::<I, Ty>()
            .parse(text)
            .into_result()
            .map_err(|errors| eyre::eyre!("failed to parse SST functions: {:?}", errors))?;

        Ok(Self { header, functions })
    }
}

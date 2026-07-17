// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! chumsky-0.10 combinators for the parsed-syntax shape.
//!
//! `Parse` impls for `ParsedModule`/`ParsedFunction` are generic over the
//! consumer's instruction type `I` and device-header type `H`. The body
//! parser tries `I::parser()` first (with `.rewind()` so failure restores
//! input) and falls back to [`raw_form`] for everything `I` can't accept.

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco_parser_core::Parse;

use crate::SstHeader;
use crate::SstSectionView;
use crate::syntax::{BodyItem, Param, ParsedFunction, ParsedModule, RawForm, RawOperand, RawType};
use crate::traits::Instruction;

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
pub fn string_literal<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
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
}

/// `@name` — leading `@` consumed; name is `ident()`-shaped.
pub fn symbol_ref<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    just('@').ignore_then(vihaco_parser_core::ident())
}

/// One operand: tries each shape in order. `Ident` is the catch-all and runs
/// last so numeric literals get their typed shape.
pub fn raw_operand<'src>() -> impl Parser<'src, &'src str, RawOperand, E<'src>> + Clone {
    // Float: int `.` digits, optional `e[+-]?digits` for scientific notation.
    // Required `.` distinguishes from ints; ints would otherwise match the
    // leading-integer prefix.
    let exp = || {
        one_of("eE")
            .then(one_of("+-").or_not())
            .then(text::digits(10))
    };
    let float_lit = text::int(10)
        .then(just('.').then(text::digits(10)))
        .then(exp().or_not())
        .to_slice()
        .map(|s: &str| RawOperand::Float(s.parse().unwrap()));
    let neg_float = just('-')
        .then(text::int(10))
        .then(just('.').then(text::digits(10)))
        .then(exp().or_not())
        .to_slice()
        .map(|s: &str| RawOperand::Float(s.parse().unwrap()));
    let neg_int = just('-')
        .then(text::int(10))
        .to_slice()
        .map(|s: &str| RawOperand::Int(s.parse().unwrap()));
    let uint_lit = text::int(10).map(|s: &str| RawOperand::UInt(s.parse().unwrap()));
    let bool_lit = choice((
        just("true").to(RawOperand::Bool(true)),
        just("false").to(RawOperand::Bool(false)),
    ));
    let str_lit = string_literal().map(RawOperand::StringLit);
    let sym = symbol_ref().map(RawOperand::Symbol);
    let id = vihaco_parser_core::ident().map(RawOperand::Ident);

    choice((
        str_lit, sym, bool_lit, neg_float, float_lit, neg_int, uint_lit, id,
    ))
}

/// `mnemonic operand (, operand)* ` — one source line's worth, sans
/// terminator.
pub fn raw_form<'src>() -> impl Parser<'src, &'src str, RawForm, E<'src>> + Clone {
    let inline_ws = any()
        .filter(|c: &char| c.is_whitespace() && *c != '\n')
        .repeated();
    let operand_sep = inline_ws.then(just(',').or_not()).then(inline_ws);
    vihaco_parser_core::ident()
        .then(
            inline_ws
                .ignore_then(raw_operand())
                .separated_by(operand_sep.ignored())
                .collect::<Vec<_>>(),
        )
        .map(|(mnemonic, operands)| RawForm { mnemonic, operands })
}

/// `RawType` is a bare identifier — `i64`, `f64`, …. Resolver translates.
pub fn raw_type<'src>() -> impl Parser<'src, &'src str, RawType, E<'src>> + Clone {
    vihaco_parser_core::ident().map(RawType)
}

/// Block-body helper: parse a sequence of whitespace-separated `i64`s as
/// **flat rows** between an outer `{` … `}` provided by the caller (typically
/// the `#[derive(Parse)]`-emitted delimiters). The body itself is a sequence
/// of `i64`s with any whitespace (including newlines) between them.
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

/// Lookahead-only end-of-statement marker. A body item ends at a newline,
/// `}`, or end-of-input — anything else means the canonical parse consumed
/// only a prefix of a longer source line (e.g. matching the unit variant
/// `fpga::Play` on `fpga::Play 5`, which is actually the sugar form).
///
/// `.rewind()` so we don't consume the terminator — the outer body parser's
/// `skip()` handles whitespace/newlines between items.
fn statement_end<'src>() -> impl Parser<'src, &'src str, (), E<'src>> + Clone {
    let inline_ws = any()
        .filter(|c: &char| c.is_whitespace() && *c != '\n')
        .repeated();
    let terminator = choice((just('\n').ignored(), just('}').ignored(), end())).rewind();
    inline_ws.ignore_then(terminator)
}

/// Body item: try `I::parser()` first, but only accept it if the remainder of
/// the line is empty (canonical form must own the whole statement). On
/// failure, fall back to `raw_form()` which captures sugar and symbolic
/// operand forms.
pub fn body_item<'src, I>() -> impl Parser<'src, &'src str, BodyItem<I>, E<'src>>
where
    I: Parse<'src> + 'src,
{
    let direct = I::parser()
        .then_ignore(statement_end())
        .map(BodyItem::Direct);
    let raw = raw_form().map(BodyItem::Raw);
    direct.boxed().or(raw.boxed())
}

/// Parse `i64`/`f64`/etc. parameter list. Currently only accepts empty `()`.
fn param_list<'src>() -> impl Parser<'src, &'src str, Vec<Param>, E<'src>> + Clone {
    just('(').padded().then(just(')').padded()).to(Vec::new())
}

fn functions<'src, I>() -> impl Parser<'src, &'src str, Vec<ParsedFunction<I>>, E<'src>>
where
    I: Parse<'src> + 'src,
{
    skip()
        .ignore_then(ParsedFunction::<I>::parser())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(skip())
}

impl<'src, I> Parse<'src> for ParsedFunction<I>
where
    I: Parse<'src> + 'src,
{
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        let return_ty = just("->").padded().ignore_then(raw_type()).or_not();
        let body = skip()
            .ignore_then(body_item::<I>())
            .repeated()
            .collect::<Vec<_>>()
            .then_ignore(skip());

        just("fn")
            .padded()
            .ignore_then(just('@'))
            .ignore_then(vihaco_parser_core::ident())
            .then(param_list())
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

impl<I, H> ParsedModule<I, H> {
    /// Parse a source section into a pre-resolution module.
    pub fn parse_section<'src, C>(section: SstSectionView<'src, C>) -> eyre::Result<Self>
    where
        H: SstHeader,
        I: Instruction + vihaco_parser_core::Parse<'src> + 'src,
    {
        let header = section.parse_header::<H>()?;
        let text = section.sst();
        let functions = functions::<I>()
            .parse(text)
            .into_result()
            .map_err(|errors| eyre::eyre!("failed to parse SST functions: {:?}", errors))?;

        Ok(Self { header, functions })
    }
}

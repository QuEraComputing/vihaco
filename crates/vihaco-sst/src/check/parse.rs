// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Building blocks that run the standard parsers over one block and turn
//! their errors into diagnostics. Returned spans are file offsets.

use vihaco_bytecode::{SSTGlobalContext, SSTHeader};
use vihaco_parser::Parse;
use vihaco_syntax::{ParsedText, Spanned, SyntaxError};

use super::Block;
use crate::diagnostic::Diagnostics;
use crate::source::Span;

fn report(block: &Block<'_>, errors: Vec<SyntaxError>, out: &mut Diagnostics<'_>) {
    let base = block.span().start;
    for error in errors {
        let span = Span::from(error.span).shift(base);
        out.error(span, error.message, error.label);
    }
}

fn shift<T>(base: usize, spanned: Spanned<T>) -> Spanned<T> {
    Spanned {
        value: spanned.value,
        span: base + spanned.span.start..base + spanned.span.end,
    }
}

/// Parse a `.text` block with instruction syntax `I`, reporting every syntax
/// error. Instruction and symbol spans in the result are file offsets; the
/// result's `errors` field is left empty because they were moved into `out`.
pub fn parse_instructions<'src, I>(block: &Block<'src>, out: &mut Diagnostics<'_>) -> ParsedText<I>
where
    I: Parse<'src> + 'src,
{
    let first_line = out.source().location(block.span().start).line;
    let parsed = vihaco_syntax::parse_text_at::<I>(block.text(), first_line);
    let base = block.span().start;
    report(block, parsed.errors, out);
    ParsedText {
        code: parsed
            .code
            .into_iter()
            .map(|item| shift(base, item))
            .collect(),
        symbols: parsed
            .symbols
            .into_iter()
            .map(|item| shift(base, item))
            .collect(),
        errors: Vec::new(),
    }
}

/// Parse a `.header` block as declarations `D`, one per line (the grammar of
/// `vihaco_syntax::HeaderDeclarations<D>`), reporting every syntax error.
///
/// Returns `None` if any declaration was malformed. Spans are file offsets.
pub fn parse_declarations<'src, D>(
    block: &Block<'src>,
    out: &mut Diagnostics<'_>,
) -> Option<Vec<Spanned<D>>>
where
    D: Parse<'src> + 'src,
{
    let errors_before = out.error_count();
    let declarations = parse_declarations_recovering(block, out);
    (out.error_count() == errors_before).then_some(declarations)
}

/// Parse a header and return successfully parsed declarations even when some
/// statements are malformed.
pub(crate) fn parse_declarations_recovering<'src, D>(
    block: &Block<'src>,
    out: &mut Diagnostics<'_>,
) -> Vec<Spanned<D>>
where
    D: Parse<'src> + 'src,
{
    let first_line = out.source().location(block.span().start).line;
    let parsed = vihaco_syntax::parse_declarations_at::<D>(block.text(), first_line);
    report(block, parsed.errors, out);
    let base = block.span().start;
    parsed
        .declarations
        .into_iter()
        .map(|item| shift(base, item))
        .collect()
}

/// Parse a header with any [`SSTHeader`]. Its parser reports no positions, so
/// an error points at the `.header` directive (or the section directive when
/// the block is omitted, passed as `anchor`).
pub fn parse_opaque_header<H: SSTHeader>(
    block: &Block<'_>,
    anchor: Span,
    out: &mut Diagnostics<'_>,
) -> Option<H> {
    match H::from_text(block.text()) {
        Ok(header) => Some(header),
        Err(error) => {
            let span = block.directive().unwrap_or(anchor);
            out.error(span, format!("invalid header: {error:#}"), "in this header");
            None
        }
    }
}

/// Parse the `.global` block as context `C`. Errors point at the `.global`
/// directive, or the version line when the block is omitted (`anchor`).
pub fn check_global_context<C: SSTGlobalContext>(
    global: &Block<'_>,
    anchor: Span,
    out: &mut Diagnostics<'_>,
) -> Option<C> {
    match C::from_text(global.text()) {
        Ok(context) => Some(context),
        Err(error) => {
            let span = global.directive().unwrap_or(anchor);
            out.error(
                span,
                format!("invalid global context: {error:#}"),
                "in this block",
            );
            None
        }
    }
}

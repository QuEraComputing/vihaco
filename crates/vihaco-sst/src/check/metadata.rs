// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Multi-error checks of header directives and symbol bindings.
//!
//! Header problems come from `ProgramHeader::assemble`, which reports every
//! problem with the directive it belongs to; this module locates each one in
//! its directive's line. Bindings mirror `ProgramMetadata::resolve_functions`
//! but report every function without a symbol instead of the first.

use std::collections::HashMap;

use vihaco_module::module::{Focus, HeaderError, ProgramMetadata, Symbol};
use vihaco_syntax::Spanned;

use crate::diagnostic::Diagnostics;
use crate::source::{SourceFile, Span};

/// The part of the directive at `directive` that `focus` names.
pub(crate) fn focus_span(source: &SourceFile, directive: Span, focus: Focus) -> Span {
    let text = source.slice(directive);
    let at = |start: usize, end: usize| Span::new(directive.start + start, directive.start + end);
    match focus {
        Focus::Directive => directive,
        Focus::Symbol => match text.find('@') {
            Some(start) => {
                let rest = &text[start + 1..];
                let len = rest
                    .find(|c: char| c.is_whitespace() || "(),;:@[]{}\"".contains(c))
                    .unwrap_or(rest.len());
                at(start, start + 1 + len)
            }
            None => directive,
        },
        Focus::Locals => match text.rfind("locals") {
            Some(start) => at(start, text.len()),
            None => directive,
        },
        Focus::Argument => match text.find(char::is_whitespace) {
            Some(end) => {
                let rest = &text[end..];
                at(end + rest.len() - rest.trim_start().len(), text.len())
            }
            None => directive,
        },
    }
}

/// Report every header error. `directives` holds the file span of each
/// standard directive, indexed like the errors' directive positions.
pub(crate) fn report_header_errors(
    errors: &[HeaderError],
    directives: &[Span],
    out: &mut Diagnostics<'_>,
) {
    for error in errors {
        let span = focus_span(out.source(), directives[error.directive], error.focus);
        let mut diagnostic = out
            .build_error(span, error.message.clone())
            .with_label(error.label.clone());
        if let Some((related, focus, message)) = &error.related {
            let span = focus_span(out.source(), directives[*related], *focus);
            diagnostic = diagnostic.with_secondary(span, message.clone());
        }
        out.push(diagnostic);
    }
}

/// Report every symbol defined more than once, at its redefinition.
pub(crate) fn check_duplicate_symbols(symbols: &[Spanned<Symbol>], out: &mut Diagnostics<'_>) {
    let mut first = HashMap::new();
    for symbol in symbols {
        let span = Span::from(symbol.span.clone());
        if let Some(previous) = first.insert(symbol.value.name.as_str(), span) {
            first.insert(symbol.value.name.as_str(), previous);
            let diagnostic = out
                .build_error(span, format!("duplicate symbol `@{}`", symbol.value.name))
                .with_label("redefined here")
                .with_secondary(previous, "first defined here")
                .with_note("symbol names must be unique within a section");
            out.push(diagnostic);
        }
    }
}

/// Report `.func` directives whose name has no `@name:` definition.
/// `functions` holds the file span of each function's `.func` directive.
pub(crate) fn check_bindings<V, Ty, Info>(
    metadata: &ProgramMetadata<V, Ty, Info>,
    functions: &[Span],
    symbols: &[Spanned<Symbol>],
    out: &mut Diagnostics<'_>,
) {
    for (index, directive) in functions.iter().enumerate() {
        let Ok(name) = metadata.function_name(index) else {
            continue;
        };
        if symbols.iter().any(|symbol| symbol.value.name == name) {
            continue;
        }
        let span = focus_span(out.source(), *directive, Focus::Symbol);
        let diagnostic = out
            .build_error(
                span,
                format!(
                    "function `@{name}` (functions[{index}]) has no `@{name}:` symbol definition"
                ),
            )
            .with_label("declared here")
            .with_note(format!(
                "add `@{name}:` before the function's first instruction in `.text`"
            ));
        out.push(diagnostic);
    }
}

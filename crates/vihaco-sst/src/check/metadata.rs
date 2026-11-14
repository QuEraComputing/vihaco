// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Multi-error checks of `ProgramMetadata` tables and symbol bindings.
//!
//! These mirror `ProgramMetadata::resolve_functions` and `into_module`, but
//! report every problem instead of the first. Problems are located by finding
//! the field inside the declaration's record text (best effort); when a field
//! cannot be found, the declaration's head is used.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use vihaco_module::module::{ProgramMetadata, Symbol};
use vihaco_syntax::Spanned;

use super::program::ConstantReferences;
use crate::diagnostic::Diagnostics;
use crate::source::{SourceFile, Span};

/// One step of a path to a metadata field.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Segment<'a> {
    Key(&'a str),
    Index(usize),
}

use Segment::{Index, Key};

/// Skip whitespace and `;` comments starting at `index`.
fn skip_trivia(text: &str, mut index: usize) -> usize {
    loop {
        let rest = &text[index..];
        let trimmed = rest.trim_start();
        index += rest.len() - trimmed.len();
        if trimmed.starts_with(';') {
            index += trimmed.find('\n').unwrap_or(trimmed.len());
        } else {
            return index;
        }
    }
}

/// Split the comma-separated items between the delimiter at `open` and its
/// matching closer, honoring nesting, strings, and comments. Items are trimmed
/// of leading trivia and trailing whitespace; empty items are dropped.
fn items(text: &str, open: usize) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = open + 1;
    let mut index = open + 1;
    let mut push = |start: usize, end: usize| {
        let start = skip_trivia(text, start).min(end);
        let end = start + text[start..end].trim_end().len();
        if start < end {
            items.push(start..end);
        }
    };
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
            }
            b';' => {
                index += text[index..].find('\n').unwrap_or(text.len() - index);
                continue;
            }
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => {
                if depth == 0 {
                    push(start, index);
                    return items;
                }
                depth -= 1;
            }
            b',' if depth == 0 => {
                push(start, index);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    items
}

/// The value range of `key` in the record whose `{` is at `open`.
fn field(text: &str, open: usize, key: &str) -> Option<Range<usize>> {
    items(text, open).into_iter().find_map(|item| {
        let entry = &text[item.clone()];
        let colon = entry.find(':')?;
        let name = entry[..colon].trim().trim_matches('"');
        (name == key).then(|| skip_trivia(text, item.start + colon + 1)..item.end)
    })
}

/// Locate `path` inside a declaration's text, returning the range of the
/// deepest segment found (`None` if not even the first one was found).
pub(crate) fn locate(text: &str, path: &[Segment<'_>]) -> Option<Range<usize>> {
    let mut current: Option<Range<usize>> = None;
    for segment in path {
        let scope = current.clone().unwrap_or(0..text.len());
        let next = match segment {
            Key(key) => {
                let open = scope.start + text[scope.clone()].find('{')?;
                field(text, open, key)
            }
            Index(index) => {
                let open = scope.start + text[scope.clone()].find('[')?;
                items(text, open).into_iter().nth(*index)
            }
        };
        match next {
            Some(range) => current = Some(range),
            None => break,
        }
    }
    current
}

/// Where a metadata declaration lives, for locating its fields.
pub(crate) struct Site<'s> {
    source: &'s SourceFile,
    declaration: Span,
}

impl<'s> Site<'s> {
    pub(crate) fn new(source: &'s SourceFile, declaration: Span) -> Self {
        Self {
            source,
            declaration,
        }
    }

    /// The declaration's first word, such as `metadata`.
    pub(crate) fn head(&self) -> Span {
        let text = self.source.slice(self.declaration);
        let len = text
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(text.len())
            .max(1)
            .min(text.len());
        Span::new(self.declaration.start, self.declaration.start + len)
    }

    /// The span of the field at `path`, or of its closest located ancestor,
    /// or the declaration head.
    pub(crate) fn span(&self, path: &[Segment<'_>]) -> Span {
        let text = self.source.slice(self.declaration);
        locate(text, path).map_or_else(
            || self.head(),
            |range| Span::from(range).shift(self.declaration.start),
        )
    }
}

fn entries(count: usize) -> String {
    match count {
        1 => "1 entry".to_owned(),
        n => format!("{n} entries"),
    }
}

fn out_of_range(path: &str, what: &str, index: u32, table: &str, len: usize) -> String {
    format!(
        "{path}: {what} index {index} is out of range (metadata.{table} has {})",
        entries(len)
    )
}

/// Check table indices and per-declaration rules that need no symbols.
pub(crate) fn check_tables<V: ConstantReferences, Ty, Info>(
    metadata: &ProgramMetadata<V, Ty, Info>,
    site: &Site<'_>,
    out: &mut Diagnostics<'_>,
) {
    let strings = metadata.strings.len();
    let functions = metadata.functions.len();

    if let Some(main) = metadata.main_function
        && main as usize >= functions
    {
        out.error(
            site.span(&[Key("main_function")]),
            out_of_range(
                "metadata.main_function",
                "function",
                main,
                "functions",
                functions,
            ),
            "no such function",
        );
    }

    let mut names = BTreeMap::new();
    for (index, function) in metadata.functions.iter().enumerate() {
        let at = |field: &'static str| [Key("functions"), Index(index), Key(field)];
        let path = format!("metadata.functions[{index}]");
        match metadata.strings.get(function.name as usize) {
            None => out.error(
                site.span(&at("name")),
                out_of_range(
                    &format!("{path}.name"),
                    "string",
                    function.name,
                    "strings",
                    strings,
                ),
                "no such string",
            ),
            Some(name) => {
                if let Some(previous) = names.insert(name.as_str(), index) {
                    let diagnostic = out
                        .build_error(
                            site.span(&at("name")),
                            format!(
                                "function `@{name}` is declared by both \
                                 metadata.functions[{previous}] and {path}"
                            ),
                        )
                        .with_label("declared again here")
                        .with_secondary(
                            site.span(&[Key("functions"), Index(previous), Key("name")]),
                            "first declared here",
                        );
                    out.push(diagnostic);
                }
            }
        }
        if function.file as usize >= strings {
            out.error(
                site.span(&at("file")),
                out_of_range(
                    &format!("{path}.file"),
                    "string",
                    function.file,
                    "strings",
                    strings,
                ),
                "no such string",
            );
        }
        let params = function.signature.params.len();
        if (function.local_count as usize) < params {
            out.error(
                site.span(&at("local_count")),
                format!(
                    "{path}.local_count: {} local slots cannot hold {params} parameters",
                    function.local_count
                ),
                format!("must be at least {params}"),
            );
        }
        for (param, parameter) in function.signature.params.iter().enumerate() {
            if parameter.name as usize >= strings {
                out.error(
                    site.span(&[
                        Key("functions"),
                        Index(index),
                        Key("signature"),
                        Key("params"),
                        Index(param),
                        Key("name"),
                    ]),
                    out_of_range(
                        &format!("{path}.signature.params[{param}].name"),
                        "string",
                        parameter.name,
                        "strings",
                        strings,
                    ),
                    "no such string",
                );
            }
        }
    }

    for (index, constant) in metadata.constants.iter().enumerate() {
        let span = site.span(&[Key("constants"), Index(index)]);
        let path = format!("metadata.constants[{index}]");
        if let Some(id) = constant.string_index()
            && id as usize >= strings
        {
            out.error(
                span,
                out_of_range(&path, "string", id, "strings", strings),
                "no such string",
            );
        }
        if let Some(id) = constant.function_index()
            && id as usize >= functions
        {
            out.error(
                span,
                out_of_range(&path, "function", id, "functions", functions),
                "no such function",
            );
        }
    }

    let mut seen = HashMap::new();
    for (position, symbol) in metadata.source_symbols.iter().enumerate() {
        if let Some(first) = seen.insert(symbol.index, position) {
            let at = |position| [Key("source_symbols"), Index(position), Key("index")];
            let diagnostic = out
                .build_error(
                    site.span(&at(position)),
                    format!(
                        "metadata.source_symbols[{position}]: duplicate index {}",
                        symbol.index
                    ),
                )
                .with_label("used again here")
                .with_secondary(site.span(&at(first)), "first used here");
            out.push(diagnostic);
        }
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

/// Report function declarations whose name has no `@name:` definition.
pub(crate) fn check_bindings<V, Ty, Info>(
    metadata: &ProgramMetadata<V, Ty, Info>,
    symbols: &[Spanned<Symbol>],
    site: &Site<'_>,
    out: &mut Diagnostics<'_>,
) {
    for (index, function) in metadata.functions.iter().enumerate() {
        let Some(name) = metadata.strings.get(function.name as usize) else {
            continue;
        };
        if symbols.iter().any(|symbol| &symbol.value.name == name) {
            continue;
        }
        let diagnostic = out
            .build_error(
                site.span(&[Key("functions"), Index(index), Key("name")]),
                format!(
                    "function `@{name}` (metadata.functions[{index}]) has no `@{name}:` \
                     symbol definition"
                ),
            )
            .with_label(format!("names `{name}`"))
            .with_note(format!(
                "add `@{name}:` before the function's first instruction in `.text`"
            ));
        out.push(diagnostic);
    }
}

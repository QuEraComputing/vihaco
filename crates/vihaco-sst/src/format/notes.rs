// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Generated `;;` notes: facts looked up in a section's metadata and symbol
//! table, attached to the header directives and `.text` items they explain.
//!
//! Each note is produced by one small function, so adding an annotation means
//! adding a function and calling it from [`header_notes`] or [`SectionInfo`].

use std::collections::HashMap;

use vihaco_module::module::{
    ConstantDirective, ConstantValue, ProgramDirective, ProgramMetadata, Signature, Symbol,
};
use vihaco_parser::{Print, print_quoted};

fn quoted(text: &str) -> String {
    let mut out = String::new();
    match print_quoted(text, &mut out) {
        Ok(()) => out,
        Err(_) => format!("{text:?}"),
    }
}

fn type_name<Ty: Print>(ty: &Ty) -> String {
    ty.to_source().unwrap_or_else(|_| "?".to_owned())
}

/// `fn name(a: u64, b: i64) -> u64`, with names resolved through `strings`.
fn signature<Ty: Print>(name: &str, signature: &Signature<Ty>, strings: &[String]) -> String {
    let params: Vec<String> = signature
        .params
        .iter()
        .map(|param| {
            let name = strings.get(param.name as usize).map_or("_", String::as_str);
            format!("{name}: {}", type_name(&param.ty))
        })
        .collect();
    let ret = match signature.ret.as_slice() {
        [] => String::new(),
        [ty] => format!(" -> {}", type_name(ty)),
        types => {
            let types: Vec<String> = types.iter().map(type_name).collect();
            format!(" -> ({})", types.join(", "))
        }
    };
    format!("fn {name}({}){ret}", params.join(", "))
}

/// An address as written in notes.
pub(crate) fn address(address: u32) -> String {
    format!("0x{address:04x}")
}

/// The string table after lowering: `base`, then every string literal of the
/// printed instructions in address order, each once.
///
/// This mirrors `MetadataLoweringContext`, which interns string operands as
/// instructions lower in address order.
pub(crate) fn lowered_strings<'a>(
    base: &[String],
    instructions: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let mut strings = base.to_vec();
    for instruction in instructions {
        for literal in literals(instruction) {
            if !strings.contains(&literal) {
                strings.push(literal);
            }
        }
    }
    strings
}

/// What notes need to know about one section.
pub(crate) struct SectionInfo<'a> {
    pub symbols: &'a [Symbol],
    pub strings: &'a [String],
    /// Symbol name to address; the first definition wins.
    addresses: HashMap<&'a str, u32>,
    /// Function name to metadata index and its `.text` note.
    functions: HashMap<&'a str, (usize, String)>,
}

impl<'a> SectionInfo<'a> {
    /// `strings` is the table after lowering (see [`lowered_strings`]).
    pub fn new<V, Ty: Print, Info>(
        symbols: &'a [Symbol],
        metadata: Option<&'a ProgramMetadata<V, Ty, Info>>,
        strings: &'a [String],
    ) -> Self {
        let mut addresses = HashMap::new();
        for symbol in symbols {
            addresses
                .entry(symbol.name.as_str())
                .or_insert(symbol.address);
        }
        let mut functions = HashMap::new();
        if let Some(metadata) = metadata {
            for (index, function) in metadata.functions.iter().enumerate() {
                let Some(name) = metadata.strings.get(function.name as usize) else {
                    continue;
                };
                let note = format!(
                    "{}, locals {}, functions[{index}]",
                    signature(name, &function.signature, &metadata.strings),
                    function.local_count
                );
                functions.entry(name.as_str()).or_insert((index, note));
            }
        }
        Self {
            symbols,
            strings,
            addresses,
            functions,
        }
    }

    /// The note on a `@name:` definition: a function's signature, locals, and
    /// table index, or a plain symbol's address.
    pub fn symbol_note(&self, symbol: &Symbol) -> String {
        match self.functions.get(symbol.name.as_str()) {
            Some((_, note)) => note.clone(),
            None => address(symbol.address),
        }
    }

    /// Notes for the `@name` operands in a printed instruction: each
    /// referenced symbol's address, and its table index if it is a function.
    pub fn reference_notes(&self, printed: &str) -> Vec<String> {
        let mut notes = Vec::new();
        for name in references(printed) {
            let Some(&target) = self.addresses.get(name) else {
                continue;
            };
            let mut note = format!("@{name} = {}", address(target));
            if let Some((index, _)) = self.functions.get(name) {
                note.push_str(&format!(" (functions[{index}])"));
            }
            if !notes.contains(&note) {
                notes.push(note);
            }
        }
        notes
    }

    /// Notes for the string literals in a printed instruction: the index
    /// lowering interns them at.
    pub fn literal_notes(&self, printed: &str) -> Vec<String> {
        literals(printed)
            .into_iter()
            .filter_map(|literal| {
                let index = self.strings.iter().position(|text| *text == literal)?;
                Some(format!("{} = strings[{index}]", quoted(&literal)))
            })
            .collect()
    }
}

/// The notes of each standard directive of a section, in order:
///
/// - `.func`: `functions[i], @name = 0x0000..0x0006` (its table index and
///   executable range).
/// - `.entry`: `functions[i]`.
/// - `.file`: `strings[i]`.
/// - `.const`: `constants[i]`, with `= strings[j]` for a string and
///   `= functions[j]` for a function reference.
pub(crate) fn header_notes<V: ConstantValue, Ty: Clone, Info>(
    directives: &[&ProgramDirective<V, Ty>],
    metadata: &ProgramMetadata<V, Ty, Info>,
    symbols: &[Symbol],
    instructions: usize,
) -> Vec<Vec<String>> {
    let ranges = metadata.resolve_functions(symbols, instructions).ok();
    let string = |text: &str| metadata.strings.iter().position(|entry| entry == text);
    let mut function = 0usize;
    let mut constant = 0usize;
    directives
        .iter()
        .map(|directive| match directive {
            ProgramDirective::Function(declared) => {
                let index = function;
                function += 1;
                let range = ranges
                    .as_ref()
                    .and_then(|ranges| ranges.get(index))
                    .map(|info| {
                        format!(
                            " = {}..{}",
                            address(info.start_address),
                            address(info.end_address)
                        )
                    })
                    .unwrap_or_default();
                vec![format!("functions[{index}], @{}{range}", declared.name)]
            }
            ProgramDirective::Entry(_) => metadata
                .main_function
                .map(|index| vec![format!("functions[{index}]")])
                .unwrap_or_default(),
            ProgramDirective::File(path) => string(path)
                .map(|index| vec![format!("strings[{index}]")])
                .unwrap_or_default(),
            ProgramDirective::Const(kind) => {
                let index = constant;
                constant += 1;
                let value = metadata.constants.get(index);
                let target = match kind {
                    ConstantDirective::Str(_) => value
                        .and_then(ConstantValue::string_index)
                        .map(|id| format!(" = strings[{id}]")),
                    ConstantDirective::Function(_) => value
                        .and_then(ConstantValue::function_index)
                        .map(|id| format!(" = functions[{id}]")),
                    ConstantDirective::Value(_) => None,
                };
                vec![format!("constants[{index}]{}", target.unwrap_or_default())]
            }
            ProgramDirective::SourceSymbol(_) => Vec::new(),
        })
        .collect()
}

/// The unescaped contents of the `"…"` literals in printed text.
fn literals(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut literal = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => literal.push(match chars.next() {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('r') => '\r',
                    Some('0') => '\0',
                    Some(other) => other,
                    None => break,
                }),
                c => literal.push(c),
            }
        }
        found.push(literal);
    }
    found
}

/// The `@name` tokens of printed instruction text, outside string literals.
fn references(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (index, c) in text.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '@' => {
                let rest = &text[index + 1..];
                let end = rest
                    .find(|c: char| !symbol_name_char(c))
                    .unwrap_or(rest.len());
                if end > 0 {
                    names.push(&rest[..end]);
                }
            }
            _ => {}
        }
    }
    names
}

/// Characters allowed in a symbol name (mirrors the `.text` parser).
fn symbol_name_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            ':' | '@' | ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_skip_string_literals() {
        assert_eq!(references("t.call 0, @helper"), ["helper"]);
        assert_eq!(references("t.print \"@no\" @yes"), ["yes"]);
        assert!(references("t.halt").is_empty());
        assert_eq!(literals(r#"t.print "a\"b\n" "c""#), ["a\"b\n", "c"]);
    }

    #[test]
    fn lowered_strings_append_new_literals_in_order() {
        let base = vec!["main".to_owned(), "hi".to_owned()];
        let strings = lowered_strings(
            &base,
            ["t.print \"hi\"", "t.print \"new\"", "t.print \"new\""].into_iter(),
        );
        assert_eq!(strings, ["main", "hi", "new"]);
    }
}

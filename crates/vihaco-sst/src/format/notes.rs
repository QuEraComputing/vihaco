// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Generated `//!` notes: facts looked up in a section's metadata and symbol
//! table, attached to the header fields and `.text` items they explain.
//!
//! Each note is produced by one small function, so adding an annotation means
//! adding a function and calling it from [`header_notes`] or [`SectionInfo`].

use std::collections::HashMap;

use vihaco_module::module::{ProgramMetadata, Signature, Symbol};
use vihaco_parser::{Print, print_quoted};

use super::layout::{Group, field, value_group};
use crate::check::ConstantReferences;

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
    pub fn new<V, Ty: Print, Info>(
        symbols: &'a [Symbol],
        metadata: Option<&'a ProgramMetadata<V, Ty, Info>>,
    ) -> Self {
        let mut addresses = HashMap::new();
        for symbol in symbols {
            addresses
                .entry(symbol.name.as_str())
                .or_insert(symbol.address);
        }
        let mut functions = HashMap::new();
        let strings = metadata.map_or(&[][..], |metadata| metadata.strings.as_slice());
        if let Some(metadata) = metadata {
            for (index, function) in metadata.functions.iter().enumerate() {
                let Some(name) = strings.get(function.name as usize) else {
                    continue;
                };
                let note = format!(
                    "{}, locals {}, functions[{index}]",
                    signature(name, &function.signature, strings),
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

    /// Notes for the string literals in a printed instruction that appear in
    /// `metadata.strings`: the index lowering resolves them to.
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

/// Attach notes to the fields of a printed `metadata { ... }` record.
pub(crate) fn header_notes<V: ConstantReferences, Ty: Print, Info>(
    metadata: &ProgramMetadata<V, Ty, Info>,
    record: &mut Group,
    text: &str,
) {
    let strings = &metadata.strings;
    let string = |index: u32| strings.get(index as usize).map(|text| quoted(text));
    let function = |index: u32| {
        metadata
            .function_name(index as usize)
            .ok()
            .map(|name| format!("@{name}"))
    };

    if let Some(functions) = field(record, text, "functions").and_then(value_group) {
        for (decl, item) in metadata.functions.iter().zip(&mut functions.items) {
            let Some(fields) = value_group(item) else {
                continue;
            };
            if let (Some(item), Some(note)) = (field(fields, text, "name"), string(decl.name)) {
                item.notes.push(note);
            }
            if !decl.signature.params.is_empty()
                && let Some(item) = field(fields, text, "signature")
            {
                let name = strings.get(decl.name as usize).map_or("_", String::as_str);
                item.notes.push(signature(name, &decl.signature, strings));
            }
            if let (Some(item), Some(note)) = (field(fields, text, "file"), string(decl.file)) {
                item.notes.push(note);
            }
        }
    }

    if let (Some(item), Some(note)) = (
        field(record, text, "main_function"),
        metadata.main_function.and_then(function),
    ) {
        item.notes.push(note);
    }

    if let Some(constants) = field(record, text, "constants").and_then(value_group) {
        for (constant, item) in metadata.constants.iter().zip(&mut constants.items) {
            let note = constant
                .string_index()
                .and_then(string)
                .or_else(|| constant.function_index().and_then(function));
            if let Some(note) = note {
                item.notes.push(note);
            }
        }
    }
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
}

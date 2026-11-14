// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Symbol lookups and string interning for metadata-backed instruction
//! lowering.

use crate::module::{ProgramMetadata, Symbol};
use std::collections::HashMap;

/// A lowering context that interns string literals into the section's string
/// table.
///
/// `vihaco_syntax::ParsedModule::lower_with_metadata` uses it to move the
/// completed table into the module. Contexts that wrap
/// [`MetadataLoweringContext`] implement it by delegating.
pub trait InternedStrings {
    /// Take the string table: the metadata's strings followed by every string
    /// interned during lowering, in first-occurrence order.
    fn take_strings(&mut self) -> Vec<String>;
}

/// Symbol lookups and string interning backed by program metadata and the
/// section's symbol table.
///
/// Every `@name` operand resolves through one section-wide symbol table:
/// [`label_address`](Self::label_address) accepts any symbol, while
/// [`function_address`](Self::function_address) additionally requires a
/// function declaration with that name. Function indices retain metadata table
/// order, independently of executable addresses.
///
/// [`string_index`](Self::string_index) interns: a string already in the
/// table keeps its index, and a new one is appended. Lowering a code stream
/// in address order therefore appends its literals in address order, after
/// the strings the header derived. Take the completed table with
/// [`into_strings`](Self::into_strings) and store it in the metadata before
/// assembling the module. The context is independent of any instruction set;
/// dialect crates implement their lowering-capability traits for it.
#[derive(Debug)]
pub struct MetadataLoweringContext {
    table: Vec<String>,
    strings: HashMap<String, u32>,
    functions: HashMap<String, u32>,
    symbols: HashMap<String, u32>,
}

impl MetadataLoweringContext {
    /// Prepare section-wide string, function, and symbol lookups.
    ///
    /// # Errors
    /// Rejects invalid function-name indices, duplicate function names,
    /// duplicate symbols, and tables whose indices cannot be represented by
    /// `u32`. Full program validation remains the responsibility of the
    /// metadata loading driver.
    pub fn new<V, Ty, Info>(
        metadata: &ProgramMetadata<V, Ty, Info>,
        symbols: &[Symbol],
    ) -> eyre::Result<Self> {
        let mut strings = HashMap::new();
        for (index, text) in metadata.strings.iter().enumerate() {
            strings.entry(text.clone()).or_insert(u32::try_from(index)?);
        }
        let string_table = metadata.strings.clone();
        let mut functions = HashMap::new();
        for index in 0..metadata.functions.len() {
            let name = metadata.function_name(index)?;
            eyre::ensure!(
                functions
                    .insert(name.to_owned(), u32::try_from(index)?)
                    .is_none(),
                "function `@{name}` has duplicate metadata"
            );
        }
        let mut table = HashMap::with_capacity(symbols.len());
        for symbol in symbols {
            eyre::ensure!(
                table.insert(symbol.name.clone(), symbol.address).is_none(),
                "duplicate symbol `@{}`",
                symbol.name
            );
        }
        Ok(Self {
            table: string_table,
            strings,
            functions,
            symbols: table,
        })
    }

    /// Look up a function's executable start address: its symbol's address.
    pub fn function_address(&self, name: &str) -> eyre::Result<u32> {
        eyre::ensure!(
            self.functions.contains_key(name),
            "undefined function `@{name}`"
        );
        self.symbols
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("function `@{name}` has no `@{name}:` symbol definition"))
    }

    /// Look up a function's index in the supplied metadata table.
    pub fn function_index(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined function `@{name}`"))
    }

    /// Look up any symbol's address in the section's symbol table.
    pub fn label_address(&self, name: &str) -> eyre::Result<u32> {
        self.symbols
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined symbol `@{name}`"))
    }

    /// Intern decoded string contents: the index of the first equal entry, or
    /// of a new entry appended to the table.
    ///
    /// # Errors
    /// Fails only if the table cannot represent another `u32` index.
    pub fn string_index(&mut self, text: &str) -> eyre::Result<u32> {
        if let Some(&index) = self.strings.get(text) {
            return Ok(index);
        }
        let index = u32::try_from(self.table.len())
            .map_err(|_| eyre::eyre!("string table exceeds u32 index space"))?;
        self.table.push(text.to_owned());
        self.strings.insert(text.to_owned(), index);
        Ok(index)
    }

    /// The string table so far: the metadata's strings, then every interned
    /// string.
    pub fn strings(&self) -> &[String] {
        &self.table
    }

    /// The completed string table, for `metadata.strings` before
    /// `metadata.into_module(...)`.
    pub fn into_strings(self) -> Vec<String> {
        self.table
    }
}

impl InternedStrings for MetadataLoweringContext {
    fn take_strings(&mut self) -> Vec<String> {
        self.strings.clear();
        std::mem::take(&mut self.table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{FunctionDecl, NoInfo, Signature};

    fn metadata() -> ProgramMetadata {
        let function = |name| FunctionDecl {
            name,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            file: 0,
        };
        ProgramMetadata {
            strings: vec![
                "later".into(),
                "first".into(),
                "hello".into(),
                "hello".into(),
            ],
            functions: vec![function(0), function(1)],
            constants: vec![],
            main_function: Some(0),
            source_symbols: vec![],
            extra: NoInfo,
        }
    }

    fn symbols() -> Vec<Symbol> {
        [("first", 0), ("done", 6), ("later", 7)]
            .into_iter()
            .map(|(name, address)| Symbol {
                name: name.into(),
                address,
            })
            .collect()
    }

    #[test]
    fn lookups_preserve_table_indices_and_resolve_through_symbols() -> eyre::Result<()> {
        let metadata = metadata();
        let mut context = MetadataLoweringContext::new(&metadata, &symbols())?;
        assert_eq!(context.function_index("later")?, 0);
        assert_eq!(context.function_address("later")?, 7);
        assert_eq!(context.function_index("first")?, 1);
        assert_eq!(context.function_address("first")?, 0);
        assert_eq!(context.label_address("done")?, 6);
        // Function symbols are ordinary symbols too.
        assert_eq!(context.label_address("later")?, 7);
        assert_eq!(context.string_index("hello")?, 2);
        // New strings are appended once, in first-occurrence order.
        assert_eq!(context.string_index("new")?, 4);
        assert_eq!(context.string_index("newer")?, 5);
        assert_eq!(context.string_index("new")?, 4);
        assert_eq!(metadata.strings.len(), 4);
        assert_eq!(&context.strings()[4..], ["new", "newer"]);
        assert_eq!(context.into_strings().len(), 6);
        Ok(())
    }

    #[test]
    fn invalid_symbols_return_errors() -> eyre::Result<()> {
        let mut metadata = metadata();
        let context = MetadataLoweringContext::new(&metadata, &symbols())?;
        assert!(context.function_index("missing").is_err());
        assert!(context.label_address("missing").is_err());
        // `done` is a symbol but not a declared function.
        assert!(context.function_address("done").is_err());

        let mut duplicate = symbols();
        duplicate.push(Symbol {
            name: "done".into(),
            address: 1,
        });
        assert!(MetadataLoweringContext::new(&metadata, &duplicate).is_err());

        let context = MetadataLoweringContext::new(&metadata, &[])?;
        let err = context.function_address("first").unwrap_err();
        assert!(err.to_string().contains("no `@first:` symbol"), "{err}");

        metadata.functions[0].name = 99;
        assert!(MetadataLoweringContext::new(&metadata, &symbols()).is_err());
        metadata.functions[0].name = 1;
        assert!(MetadataLoweringContext::new(&metadata, &symbols()).is_err());
        Ok(())
    }

    #[test]
    fn resolve_functions_derives_ranges_from_symbols() -> eyre::Result<()> {
        let metadata = metadata();
        let functions = metadata.resolve_functions(&symbols(), 9)?;
        // Table order is preserved; ranges follow address order.
        assert_eq!(
            functions
                .iter()
                .map(|f| (f.name, f.start_address, f.end_address))
                .collect::<Vec<_>>(),
            [(0, 7, 9), (1, 0, 7)]
        );

        let err = metadata.resolve_functions(&symbols(), 5).unwrap_err();
        assert!(
            err.to_string().contains("outside the instruction stream"),
            "{err}"
        );

        let err = metadata.resolve_functions(&symbols()[..2], 9).unwrap_err();
        assert!(
            err.to_string()
                .contains("function `@later` (metadata.functions[0]) has no `@later:` symbol"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn map_extra_preserves_all_tables_without_requiring_clone() {
        struct Extra(String);
        let original = metadata();
        let converted = metadata().map_extra(|_| Extra("device".into()));
        let converted = converted.map_extra(|extra| extra.0.len());
        assert_eq!(converted.extra, 6);
        assert_eq!(converted.functions, original.functions);
        assert_eq!(converted.strings, original.strings);
        assert_eq!(converted.constants, original.constants);
        assert_eq!(converted.source_symbols, original.source_symbols);
        assert_eq!(converted.main_function, original.main_function);
    }
}

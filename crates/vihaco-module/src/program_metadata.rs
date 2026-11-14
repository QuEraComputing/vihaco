// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard program-header schema and section-local metadata validation.
//!
//! Every table is supplied by the header, even when empty. Array order defines
//! table indices; names and files are explicit string-table indices. Loading
//! never interns strings, invents signatures, or infers an entrypoint.
//!
//! The header says *what* each function is; the section's symbol table says
//! *where* it is. A function declaration is bound to the `@name:` symbol with
//! the same name, and its executable range runs from that symbol to the next
//! function symbol (or the end of the code).
//!
//! Constants use their type's ordinary `Parse` implementation. Standard `Value`
//! constants use typed SST literals such as `u64 42` or `string 0`; `f32_bits`
//! and `f64_bits` followed by unsigned integers preserve all IEEE-754 encodings.
//! Custom extra records use the same delimited metadata derive as the header.

use std::collections::{BTreeMap, HashMap};

use crate::module::{FunctionDecl, FunctionInfo, LocalModule, NoInfo, SourceSymbolInfo, Symbol};
use vihaco_abi::{Type, Value};

/// All non-code fields of a `LocalModule` that a section header supplies.
/// The ordinary metadata derive owns object parsing, including nested records.
/// Fields parse in any order; `Print` writes them in declaration order.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "metadata", open = "{", close = "}")]
pub struct ProgramMetadata<V = Value, Ty = Type, Info = NoInfo> {
    pub strings: Vec<String>,
    pub functions: Vec<FunctionDecl<Ty>>,
    pub constants: Vec<V>,
    pub main_function: Option<u32>,
    pub source_symbols: Vec<SourceSymbolInfo>,
    pub extra: Info,
}

fn check_index(index: u32, len: usize, what: &str) -> eyre::Result<()> {
    eyre::ensure!(
        (index as usize) < len,
        "{what}: index {index} is outside table of length {len}"
    );
    Ok(())
}

/// Check a symbol table against its code length and index it by name.
///
/// Names must be unique within a section, and every address must lie within
/// the code (an address equal to the code length marks the end).
pub fn index_symbols(
    symbols: &[Symbol],
    instruction_count: usize,
) -> eyre::Result<HashMap<&str, u32>> {
    let mut table = HashMap::with_capacity(symbols.len());
    for symbol in symbols {
        eyre::ensure!(
            symbol.address as usize <= instruction_count,
            "symbol `@{}` at {} is outside the instruction stream of length {instruction_count}",
            symbol.name,
            symbol.address
        );
        eyre::ensure!(
            table.insert(symbol.name.as_str(), symbol.address).is_none(),
            "duplicate symbol `@{}`",
            symbol.name
        );
    }
    Ok(table)
}

impl<V, Ty, Info> ProgramMetadata<V, Ty, Info> {
    /// Transform consumer-specific extra metadata while preserving every table.
    pub fn map_extra<NewInfo>(
        self,
        map: impl FnOnce(Info) -> NewInfo,
    ) -> ProgramMetadata<V, Ty, NewInfo> {
        ProgramMetadata {
            functions: self.functions,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: map(self.extra),
        }
    }

    /// The name of function declaration `index`, resolved through `strings`.
    pub fn function_name(&self, index: usize) -> eyre::Result<&str> {
        let function = self
            .functions
            .get(index)
            .ok_or_else(|| eyre::eyre!("metadata.functions[{index}] does not exist"))?;
        self.strings
            .get(function.name as usize)
            .map(String::as_str)
            .ok_or_else(|| {
                eyre::eyre!(
                    "metadata.functions[{index}].name: index {} is outside table of length {}",
                    function.name,
                    self.strings.len()
                )
            })
    }

    /// Validate table references and bind declarations to the symbol table,
    /// without altering the supplied metadata.
    pub fn validate(&self, symbols: &[Symbol], instruction_count: usize) -> eyre::Result<()>
    where
        Ty: Clone,
    {
        self.resolve_functions(symbols, instruction_count)
            .map(|_| ())
    }

    /// Bind every function declaration to its symbol and derive its range.
    ///
    /// Returns runtime function records in metadata table order. A function
    /// starts at its symbol's address and ends at the next function symbol's
    /// address, or at `instruction_count` for the last function.
    pub fn resolve_functions(
        &self,
        symbols: &[Symbol],
        instruction_count: usize,
    ) -> eyre::Result<Vec<FunctionInfo<Ty>>>
    where
        Ty: Clone,
    {
        eyre::ensure!(
            u32::try_from(instruction_count).is_ok(),
            "instruction stream exceeds u32 address space"
        );
        let table = index_symbols(symbols, instruction_count)?;
        if let Some(main) = self.main_function {
            check_index(main, self.functions.len(), "metadata.main_function")?;
        }

        let mut starts = Vec::with_capacity(self.functions.len());
        let mut names = BTreeMap::new();
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}]");
            let name = self.function_name(index)?;
            check_index(function.file, self.strings.len(), &format!("{path}.file"))?;
            if let Some(previous) = names.insert(name, index) {
                eyre::bail!(
                    "function `@{name}` is declared by both metadata.functions[{previous}] and {path}"
                );
            }
            eyre::ensure!(
                function.local_count as usize >= function.signature.params.len(),
                "{path}.local_count: fewer slots than parameters"
            );
            for parameter in &function.signature.params {
                check_index(
                    parameter.name,
                    self.strings.len(),
                    &format!("{path}.signature.params.name"),
                )?;
            }
            let start = *table.get(name).ok_or_else(|| {
                eyre::eyre!("function `@{name}` ({path}) has no `@{name}:` symbol definition")
            })?;
            starts.push(start);
        }

        let mut boundaries = starts.clone();
        boundaries.sort_unstable();
        boundaries.dedup();
        let end_of = |start: u32| {
            boundaries
                .iter()
                .copied()
                .find(|&boundary| boundary > start)
                .unwrap_or(instruction_count as u32)
        };

        let mut seen = std::collections::BTreeSet::new();
        for symbol in &self.source_symbols {
            eyre::ensure!(
                seen.insert(symbol.index),
                "metadata.source_symbols: duplicate index {}",
                symbol.index
            );
        }

        Ok(self
            .functions
            .iter()
            .zip(starts)
            .map(|(function, start)| FunctionInfo {
                name: function.name,
                signature: function.signature.clone(),
                local_count: function.local_count,
                start_address: start,
                end_address: end_of(start),
                file: function.file,
            })
            .collect())
    }
}

impl<Ty: Clone, Info> ProgramMetadata<Value, Ty, Info> {
    /// Assemble the standard value model, checking the supplied tables once.
    /// Custom value models can assemble `LocalModule` directly after
    /// [`resolve_functions`](Self::resolve_functions).
    pub fn into_module<I>(
        self,
        code: Vec<I>,
        symbols: Vec<Symbol>,
    ) -> eyre::Result<LocalModule<I, Value, Ty, Info>> {
        let functions = self.resolve_functions(&symbols, code.len())?;
        for (index, constant) in self.constants.iter().enumerate() {
            match constant {
                Value::String(id) => check_index(
                    *id,
                    self.strings.len(),
                    &format!("metadata.constants[{index}] string"),
                )?,
                Value::FunctionRef(id) => check_index(
                    *id,
                    self.functions.len(),
                    &format!("metadata.constants[{index}] function"),
                )?,
                _ => {}
            }
        }
        Ok(LocalModule {
            code,
            functions,
            symbols,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: self.extra,
        })
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard program-header schema and section-local metadata validation.
//!
//! Every table is supplied by the header, even when empty. Array order defines
//! table indices; names and files are explicit string-table indices. Addresses
//! count decoded instructions, with an exclusive function end. Loading never
//! interns strings, invents signatures, or infers an entrypoint.
//!
//! Constants use their type's ordinary `Parse` implementation. Standard `Value`
//! constants use typed SST literals such as `u64 42` or `string 0`; `f32_bits`
//! and `f64_bits` followed by unsigned integers preserve all IEEE-754 encodings.
//! Custom extra records use the same delimited metadata derive as the header.

use crate::module::{
    FunctionInfo, LabelInfo, LocalModule, NoInfo, SourceSymbolInfo, StructInfo, StructTypeRef,
};
use vihaco_abi::{Type, Value};

/// All non-code fields of a `LocalModule`, supplied explicitly by a section header.
/// The ordinary metadata derive owns object parsing, including nested records.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "metadata", open = "{", close = "}")]
pub struct ProgramMetadata<V = Value, Ty = Type, Info = NoInfo> {
    pub functions: Vec<FunctionInfo<Ty>>,
    pub structs: Vec<StructInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub constants: Vec<V>,
    pub strings: Vec<String>,
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

impl<V, Ty, Info> ProgramMetadata<V, Ty, Info> {
    /// Transform consumer-specific extra metadata while preserving every table.
    pub fn map_extra<NewInfo>(
        self,
        map: impl FnOnce(Info) -> NewInfo,
    ) -> ProgramMetadata<V, Ty, NewInfo> {
        ProgramMetadata {
            functions: self.functions,
            structs: self.structs,
            labels: self.labels,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: map(self.extra),
        }
    }

    /// Validate references and instruction ranges without altering supplied metadata.
    pub fn validate(&self, instruction_count: usize) -> eyre::Result<()> {
        eyre::ensure!(
            u32::try_from(instruction_count).is_ok(),
            "instruction stream exceeds u32 address space"
        );
        if let Some(main) = self.main_function {
            check_index(main, self.functions.len(), "metadata.main_function")?;
        }
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}]");
            check_index(function.name, self.strings.len(), &format!("{path}.name"))?;
            check_index(function.file, self.strings.len(), &format!("{path}.file"))?;
            eyre::ensure!(
                function.start_address <= function.end_address
                    && function.end_address as usize <= instruction_count,
                "{path}: invalid instruction range {}..{} for {instruction_count} instructions",
                function.start_address,
                function.end_address
            );
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
        }
        let mut struct_names = std::collections::BTreeSet::new();
        for (index, info) in self.structs.iter().enumerate() {
            let path = format!("metadata.structs[{index}]");
            check_index(info.name, self.strings.len(), &format!("{path}.name"))?;
            let name = &self.strings[info.name as usize];
            eyre::ensure!(
                struct_names.insert(name),
                "{path}.name: duplicate struct `{name}`"
            );
            let mut field_names = std::collections::BTreeSet::new();
            for (field_index, field) in info.fields.iter().enumerate() {
                let field_path = format!("{path}.fields[{field_index}].name");
                check_index(field.name, self.strings.len(), &field_path)?;
                let field_name = &self.strings[field.name as usize];
                eyre::ensure!(
                    field_names.insert(field_name),
                    "{field_path}: duplicate field `{field_name}` in struct `{name}`"
                );
            }
        }
        for (index, label) in self.labels.iter().enumerate() {
            check_index(
                label.name,
                self.strings.len(),
                &format!("metadata.labels[{index}].name"),
            )?;
            eyre::ensure!(
                label.address as usize <= instruction_count,
                "metadata.labels[{index}].address: outside instruction stream"
            );
        }
        let mut symbols = std::collections::BTreeSet::new();
        for symbol in &self.source_symbols {
            eyre::ensure!(
                symbols.insert(symbol.index),
                "metadata.source_symbols: duplicate index {}",
                symbol.index
            );
        }
        Ok(())
    }
}

impl<V, Ty: StructTypeRef, Info> ProgramMetadata<V, Ty, Info> {
    /// Check that every struct type in signatures and struct fields names an
    /// entry of the struct table. Structs may refer to themselves or each other.
    pub fn validate_struct_types(&self) -> eyre::Result<()> {
        let check = |ty: &Ty, what: &dyn Fn() -> String| match ty.struct_index() {
            Some(index) => check_index(index, self.structs.len(), &what()),
            None => Ok(()),
        };
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}].signature");
            for (param, parameter) in function.signature.params.iter().enumerate() {
                check(&parameter.ty, &|| format!("{path}.params[{param}].ty"))?;
            }
            for (ret, ty) in function.signature.ret.iter().enumerate() {
                check(ty, &|| format!("{path}.ret[{ret}]"))?;
            }
        }
        for (index, info) in self.structs.iter().enumerate() {
            for (field, info) in info.fields.iter().enumerate() {
                check(&info.ty, &|| {
                    format!("metadata.structs[{index}].fields[{field}].ty")
                })?;
            }
        }
        Ok(())
    }
}

impl<Ty: StructTypeRef, Info> ProgramMetadata<Value, Ty, Info> {
    /// Assemble the standard value model, checking the supplied tables once.
    /// Custom value models can assemble `LocalModule` directly after `validate`
    /// and `validate_struct_types`.
    pub fn into_module<I>(self, code: Vec<I>) -> eyre::Result<LocalModule<I, Value, Ty, Info>> {
        self.validate(code.len())?;
        self.validate_struct_types()?;
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
            functions: self.functions,
            structs: self.structs,
            labels: self.labels,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: self.extra,
        })
    }
}

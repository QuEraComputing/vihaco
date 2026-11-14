// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Function scopes and lookups for metadata-backed instruction lowering.

use crate::module::ProgramMetadata;
use std::collections::HashMap;

/// Prepare a function's local symbols before lowering any of its instructions.
///
/// Consumers with additional lowering capabilities can implement this trait on
/// their own context and delegate to [`MetadataLoweringContext`]. Label offsets
/// count instructions within the function; declarations occupy no code slots.
pub trait FunctionScope {
    /// Replace the active function's labels, including forward references.
    fn enter_function<'a>(
        &mut self,
        name: &str,
        labels: impl IntoIterator<Item = (&'a str, usize)>,
    ) -> eyre::Result<()>;
}

/// Symbol lookups backed by explicit, immutable program metadata.
///
/// Function indices retain metadata table order, independently of executable
/// addresses. String lookups never append to or reorder the supplied table.
/// The context is independent of any instruction set; dialect crates provide
/// implementations of their lowering-capability traits for it.
#[derive(Debug)]
pub struct MetadataLoweringContext {
    strings: HashMap<String, u32>,
    functions: HashMap<String, (u32, u32)>,
    /// Struct name to struct-table index.
    struct_names: HashMap<String, u32>,
    /// Per struct-table entry: its name, field count, and field name to slot index.
    structs: Vec<StructFields>,
    labels: HashMap<String, u32>,
}

#[derive(Debug)]
struct StructFields {
    name: String,
    count: u32,
    slots: HashMap<String, u32>,
}

impl MetadataLoweringContext {
    /// Prepare section-wide string and function lookups.
    ///
    /// # Errors
    /// Rejects invalid function-name indices, duplicate function names, and
    /// tables whose indices cannot be represented by `u32`. Full program
    /// validation remains the responsibility of the metadata loading driver.
    pub fn new<V, Ty, Info>(metadata: &ProgramMetadata<V, Ty, Info>) -> eyre::Result<Self> {
        let mut strings = HashMap::new();
        for (index, text) in metadata.strings.iter().enumerate() {
            strings.entry(text.clone()).or_insert(u32::try_from(index)?);
        }
        let mut functions = HashMap::new();
        for (index, function) in metadata.functions.iter().enumerate() {
            let name = metadata
                .strings
                .get(function.name as usize)
                .ok_or_else(|| {
                    eyre::eyre!(
                        "metadata.functions[{index}].name: invalid string index {}",
                        function.name
                    )
                })?;
            eyre::ensure!(
                functions
                    .insert(
                        name.clone(),
                        (u32::try_from(index)?, function.start_address)
                    )
                    .is_none(),
                "function `@{name}` has duplicate metadata"
            );
        }
        let name_at = |index: u32, what: &dyn Fn() -> String| {
            metadata
                .strings
                .get(index as usize)
                .ok_or_else(|| eyre::eyre!("{}: invalid string index {index}", what()))
        };
        let mut struct_names = HashMap::new();
        let mut structs = Vec::with_capacity(metadata.structs.len());
        for (index, info) in metadata.structs.iter().enumerate() {
            let name = name_at(info.name, &|| format!("metadata.structs[{index}].name"))?;
            let mut slots = HashMap::new();
            for (slot, field) in info.fields.iter().enumerate() {
                let field_name = name_at(field.name, &|| {
                    format!("metadata.structs[{index}].fields[{slot}].name")
                })?;
                eyre::ensure!(
                    slots
                        .insert(field_name.clone(), u32::try_from(slot)?)
                        .is_none(),
                    "struct `{name}` has duplicate field `{field_name}`"
                );
            }
            eyre::ensure!(
                struct_names
                    .insert(name.clone(), u32::try_from(index)?)
                    .is_none(),
                "struct `{name}` has duplicate metadata"
            );
            structs.push(StructFields {
                name: name.clone(),
                count: u32::try_from(info.fields.len())?,
                slots,
            });
        }
        Ok(Self {
            strings,
            functions,
            struct_names,
            structs,
            labels: HashMap::new(),
        })
    }

    /// Look up a struct's index in the supplied metadata struct table.
    pub fn struct_index(&self, name: &str) -> eyre::Result<u32> {
        self.struct_names
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined struct `{name}`"))
    }

    fn struct_entry(&self, struct_index: u32) -> eyre::Result<&StructFields> {
        self.structs.get(struct_index as usize).ok_or_else(|| {
            eyre::eyre!(
                "struct {struct_index} is outside the struct table of length {}",
                self.structs.len()
            )
        })
    }

    /// The number of fields of a struct-table entry.
    pub fn struct_field_count(&self, struct_index: u32) -> eyre::Result<u32> {
        Ok(self.struct_entry(struct_index)?.count)
    }

    /// Look up a field's slot index within a struct-table entry.
    pub fn field_index(&self, struct_index: u32, field: &str) -> eyre::Result<u32> {
        let entry = self.struct_entry(struct_index)?;
        entry
            .slots
            .get(field)
            .copied()
            .ok_or_else(|| eyre::eyre!("struct `{}` has no field `{field}`", entry.name))
    }

    /// Look up a function's executable start address.
    pub fn function_address(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|function| function.1)
            .ok_or_else(|| eyre::eyre!("undefined function `@{name}`"))
    }

    /// Look up a function's index in the supplied metadata table.
    pub fn function_index(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|function| function.0)
            .ok_or_else(|| eyre::eyre!("undefined function `@{name}`"))
    }

    /// Look up a label in the current function's scope.
    pub fn label_address(&self, name: &str) -> eyre::Result<u32> {
        self.labels
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined label `@{name}`"))
    }

    /// Look up decoded string contents without modifying the supplied table.
    /// Equal strings resolve to the first matching table entry.
    pub fn string_index(&self, text: &str) -> eyre::Result<u32> {
        self.strings
            .get(text)
            .copied()
            .ok_or_else(|| eyre::eyre!("string literal `{text}` is missing from metadata.strings"))
    }
}

impl FunctionScope for MetadataLoweringContext {
    fn enter_function<'a>(
        &mut self,
        name: &str,
        labels: impl IntoIterator<Item = (&'a str, usize)>,
    ) -> eyre::Result<()> {
        self.labels.clear();
        let start = self.function_address(name)?;
        for (label, offset) in labels {
            let address = start
                .checked_add(u32::try_from(offset)?)
                .ok_or_else(|| eyre::eyre!("label address overflow"))?;
            eyre::ensure!(
                self.labels.insert(label.to_owned(), address).is_none(),
                "duplicate label `@{label}` in function `@{name}`"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{FieldInfo, FunctionInfo, NoInfo, Parameter, Signature, StructInfo};
    use vihaco_abi::Type;

    fn metadata() -> ProgramMetadata {
        ProgramMetadata {
            strings: vec![
                "later".into(),
                "first".into(),
                "hello".into(),
                "hello".into(),
                "Point".into(),
                "x".into(),
                "y".into(),
            ],
            structs: vec![StructInfo {
                name: 4,
                fields: vec![
                    FieldInfo {
                        name: 5,
                        ty: Type::F64,
                    },
                    FieldInfo {
                        name: 6,
                        ty: Type::I64,
                    },
                ],
            }],
            functions: vec![
                FunctionInfo {
                    name: 0,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 3,
                    start_address: 7,
                    end_address: 9,
                    file: 0,
                },
                FunctionInfo {
                    name: 1,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 0,
                    start_address: 0,
                    end_address: 7,
                    file: 0,
                },
            ],
            labels: vec![],
            constants: vec![],
            main_function: Some(0),
            source_symbols: vec![],
            extra: NoInfo,
        }
    }

    #[test]
    fn lookups_preserve_table_indices_and_function_local_scopes() -> eyre::Result<()> {
        let metadata = metadata();
        let mut context = MetadataLoweringContext::new(&metadata)?;
        assert_eq!(context.function_index("later")?, 0);
        assert_eq!(context.function_address("later")?, 7);
        assert_eq!(context.function_index("first")?, 1);
        assert_eq!(context.function_address("first")?, 0);
        assert_eq!(context.string_index("hello")?, 2);
        context.enter_function("first", [("done", 6), ("only_first", 1)])?;
        assert_eq!(context.label_address("done")?, 6);
        context.enter_function("later", [("done", 1)])?;
        assert_eq!(context.label_address("done")?, 8);
        assert!(context.label_address("only_first").is_err());
        assert!(context.string_index("missing").is_err());
        assert_eq!(metadata.strings.len(), 7);
        Ok(())
    }

    #[test]
    fn struct_lookups_resolve_table_and_field_slot_indices() -> eyre::Result<()> {
        let mut metadata = metadata();
        let context = MetadataLoweringContext::new(&metadata)?;
        assert_eq!(context.struct_index("Point")?, 0);
        assert_eq!(context.struct_field_count(0)?, 2);
        assert_eq!(context.field_index(0, "x")?, 0);
        assert_eq!(context.field_index(0, "y")?, 1);
        let err = context.field_index(0, "z").unwrap_err();
        assert!(
            err.to_string().contains("struct `Point` has no field `z`"),
            "{err}"
        );
        assert!(context.struct_index("Line").is_err());
        let err = context.struct_field_count(1).unwrap_err();
        assert!(
            err.to_string().contains("outside the struct table"),
            "{err}"
        );
        assert!(context.field_index(1, "x").is_err());

        metadata.structs.push(metadata.structs[0].clone());
        let err = MetadataLoweringContext::new(&metadata).unwrap_err();
        assert!(err.to_string().contains("duplicate"), "{err}");
        metadata.structs.pop();
        metadata.structs[0].fields[1].name = 5;
        let err = MetadataLoweringContext::new(&metadata).unwrap_err();
        assert!(err.to_string().contains("duplicate field `x`"), "{err}");
        metadata.structs[0].fields[1].name = 99;
        assert!(MetadataLoweringContext::new(&metadata).is_err());
        Ok(())
    }

    #[test]
    fn struct_tables_are_validated_at_module_assembly() {
        let into_module = |metadata: ProgramMetadata| metadata.into_module(vec![(); 9]);
        assert!(into_module(metadata()).is_ok());

        // Struct types in signatures and fields must name a struct-table entry.
        let mut bad = metadata();
        bad.functions[0].signature.ret.push(Type::Struct(1));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("signature.ret[0]"), "{err}");
        let mut bad = metadata();
        bad.functions[0].signature.params.push(Parameter {
            name: 0,
            ty: Type::Struct(1),
        });
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("signature.params[0].ty"), "{err}");
        let mut bad = metadata();
        bad.structs[0].fields[0].ty = Type::Struct(1);
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("structs[0].fields[0].ty"), "{err}");
        // Struct types nested in array element types are checked too.
        let mut bad = metadata();
        bad.functions[0]
            .signature
            .ret
            .push(Type::array(Type::array(Type::Struct(1))));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("signature.ret[0]"), "{err}");
        let mut arrays = metadata();
        arrays.structs[0].fields[0].ty = Type::array(Type::Struct(0));
        assert!(into_module(arrays).is_ok());

        // Self-reference (a linked node) is allowed.
        let mut linked = metadata();
        linked.structs[0].fields[0].ty = Type::Struct(0);
        assert!(into_module(linked).is_ok());

        // Names are string-table indices, unique per table and per struct.
        let mut bad = metadata();
        bad.structs[0].name = 99;
        assert!(into_module(bad).is_err());
        let mut bad = metadata();
        bad.structs[0].fields[1].name = 5;
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("duplicate field `x`"), "{err}");
        let mut bad = metadata();
        bad.structs.push(bad.structs[0].clone());
        let err = into_module(bad).unwrap_err();
        assert!(
            err.to_string().contains("duplicate struct `Point`"),
            "{err}"
        );
    }

    #[test]
    fn invalid_symbols_and_address_overflow_return_errors() -> eyre::Result<()> {
        let mut metadata = metadata();
        let mut context = MetadataLoweringContext::new(&metadata)?;
        assert!(context.function_index("missing").is_err());
        assert!(context.enter_function("missing", []).is_err());
        assert!(
            context
                .enter_function("first", [("same", 0), ("same", 1)])
                .is_err()
        );
        metadata.functions[0].start_address = u32::MAX;
        let mut context = MetadataLoweringContext::new(&metadata)?;
        assert!(context.enter_function("later", [("overflow", 1)]).is_err());
        metadata.functions[0].name = 99;
        assert!(MetadataLoweringContext::new(&metadata).is_err());
        metadata.functions[0].name = 1;
        assert!(MetadataLoweringContext::new(&metadata).is_err());
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
        assert_eq!(converted.structs, original.structs);
        assert_eq!(converted.labels, original.labels);
        assert_eq!(converted.strings, original.strings);
        assert_eq!(converted.constants, original.constants);
        assert_eq!(converted.source_symbols, original.source_symbols);
        assert_eq!(converted.main_function, original.main_function);
    }
}

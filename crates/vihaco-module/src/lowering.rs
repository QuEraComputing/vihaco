// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Function scopes and lookups for metadata-backed instruction lowering.

use crate::module::ProgramMetadata;
use crate::program_metadata::{NameTables, StructFields};
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
    /// Named-type name to named-type-table index.
    type_names: HashMap<String, u32>,
    /// The length of the named-type table.
    type_count: u32,
    labels: HashMap<String, u32>,
}

impl MetadataLoweringContext {
    /// Prepare section-wide string and function lookups.
    ///
    /// # Errors
    /// Rejects names that are not string-table entries, duplicate function,
    /// struct, field and named-type names, and tables whose indices cannot be
    /// represented by `u32`, with the same messages as
    /// [`ProgramMetadata::validate`]. Full program validation remains the
    /// responsibility of the metadata loading driver.
    pub fn new<V, Ty, Info>(metadata: &ProgramMetadata<V, Ty, Info>) -> eyre::Result<Self> {
        let mut strings = HashMap::new();
        for (index, text) in metadata.strings.iter().enumerate() {
            strings.entry(text.clone()).or_insert(u32::try_from(index)?);
        }
        let NameTables {
            functions,
            struct_names,
            structs,
            type_names,
        } = metadata.name_tables()?;
        Ok(Self {
            strings,
            functions,
            struct_names,
            structs,
            type_names,
            type_count: u32::try_from(metadata.types.len())?,
            labels: HashMap::new(),
        })
    }

    /// Look up a struct's index in the supplied metadata struct table.
    pub fn struct_index(&self, name: &str) -> eyre::Result<u32> {
        self.struct_names
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined struct `@{name}`"))
    }

    fn struct_entry(&self, struct_index: u32) -> eyre::Result<&StructFields> {
        self.structs.get(struct_index as usize).ok_or_else(|| {
            eyre::eyre!(
                "struct {struct_index} is outside the struct table (length {})",
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

    /// Look up a named type's index in the supplied metadata named-type table.
    pub fn named_type_index(&self, name: &str) -> eyre::Result<u32> {
        self.type_names
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("undefined named type `@{name}`"))
    }

    /// Check that `index` is an entry of the named-type table.
    pub fn check_named_type(&self, index: u32) -> eyre::Result<()> {
        eyre::ensure!(
            index < self.type_count,
            "named type {index} is outside the named-type table (length {})",
            self.type_count
        );
        Ok(())
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
    use crate::module::{
        FieldInfo, FunctionInfo, NamedTypeInfo, NoInfo, Parameter, Signature, StructInfo,
    };
    use vihaco_abi::Type;

    fn metadata() -> ProgramMetadata<vihaco_abi::Value, Type> {
        ProgramMetadata {
            strings: vec![
                "later".into(),
                "first".into(),
                "hello".into(),
                "hello".into(),
                "Point".into(),
                "x".into(),
                "y".into(),
                "location".into(),
                "lane".into(),
            ],
            types: vec![
                NamedTypeInfo {
                    name: 7,
                    repr: Type::U64,
                },
                NamedTypeInfo {
                    name: 8,
                    repr: Type::U64,
                },
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
                    locals: vec![Type::U64, Type::F64, Type::Struct(0)],
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
                    locals: vec![],
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
        assert_eq!(metadata.strings.len(), 9);
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
        let into_module =
            |metadata: ProgramMetadata<vihaco_abi::Value, Type>| metadata.into_module(vec![(); 9]);
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
        bad.functions[0].locals[0] = Type::Struct(1);
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
        let mut bad = metadata();
        bad.functions[0].locals[2] = Type::array(Type::Struct(1));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("functions[0].locals[2]"), "{err}");
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
    fn named_type_lookups_resolve_table_indices() -> eyre::Result<()> {
        let mut metadata = metadata();
        let context = MetadataLoweringContext::new(&metadata)?;
        assert_eq!(context.named_type_index("location")?, 0);
        assert_eq!(context.named_type_index("lane")?, 1);
        let err = context.named_type_index("zone").unwrap_err();
        assert!(
            err.to_string().contains("undefined named type `@zone`"),
            "{err}"
        );
        context.check_named_type(1)?;
        let err = context.check_named_type(2).unwrap_err();
        assert!(
            err.to_string().contains("outside the named-type table"),
            "{err}"
        );

        metadata.types.push(metadata.types[0].clone());
        let err = MetadataLoweringContext::new(&metadata).unwrap_err();
        assert!(err.to_string().contains("duplicate"), "{err}");
        metadata.types.pop();
        metadata.types[0].name = 99;
        assert!(MetadataLoweringContext::new(&metadata).is_err());
        Ok(())
    }

    #[test]
    fn named_type_tables_are_validated_at_module_assembly() {
        let into_module =
            |metadata: ProgramMetadata<vihaco_abi::Value, Type>| metadata.into_module(vec![(); 9]);

        // Every scalar word type may represent a named type.
        for repr in [
            Type::I32,
            Type::I64,
            Type::U32,
            Type::U64,
            Type::F32,
            Type::F64,
            Type::Bool,
        ] {
            let mut good = metadata();
            good.types[0].repr = repr;
            assert!(into_module(good).is_ok());
        }
        // Nothing else does: not strings, references, heap types, or names.
        for repr in [
            Type::String,
            Type::FunctionRef,
            Type::HeapRef,
            Type::Struct(0),
            Type::array(Type::U64),
            Type::Named(1),
        ] {
            let mut bad = metadata();
            bad.types[0].repr = repr.clone();
            let err = into_module(bad).unwrap_err();
            assert!(err.to_string().contains("types[0].repr"), "{repr}: {err}");
        }

        // Names are in range and unique.
        let mut bad = metadata();
        bad.types[1].name = 99;
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("types[1].name"), "{err}");
        let mut bad = metadata();
        bad.types[1].name = 7;
        let err = into_module(bad).unwrap_err();
        assert!(
            err.to_string().contains("duplicate named type `location`"),
            "{err}"
        );

        // Every `type N` reference names a table entry, at any nesting depth.
        let mut good = metadata();
        good.functions[0].signature.ret.push(Type::Named(1));
        good.structs[0].fields[0].ty = Type::array(Type::Named(0));
        good.functions[0].locals[0] = Type::Named(0);
        assert!(into_module(good).is_ok());
        let mut bad = metadata();
        bad.functions[0].signature.ret.push(Type::Named(2));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("signature.ret[0]"), "{err}");
        let mut bad = metadata();
        bad.functions[0].signature.params.push(Parameter {
            name: 0,
            ty: Type::Named(2),
        });
        bad.functions[0].locals[0] = Type::Named(2);
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("signature.params[0].ty"), "{err}");
        let mut bad = metadata();
        bad.functions[0].locals[1] = Type::array(Type::array(Type::Named(5)));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("functions[0].locals[1]"), "{err}");
        let mut bad = metadata();
        bad.structs[0].fields[1].ty = Type::array(Type::Named(2));
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("structs[0].fields[1].ty"), "{err}");
        let mut none = metadata();
        none.types.clear();
        none.functions[0].locals[0] = Type::Named(0);
        let err = into_module(none).unwrap_err();
        assert!(
            err.to_string().contains("metadata.functions[0].locals[0]: named type 0 is outside the named-type table (length 0)"),
            "{err}"
        );
    }

    #[test]
    fn declared_locals_are_validated() {
        let into_module =
            |metadata: ProgramMetadata<vihaco_abi::Value, Type>| metadata.into_module(vec![(); 9]);

        // At most `MAX_LOCALS` locals.
        let mut bad = metadata();
        bad.functions[1].locals = vec![Type::U64; crate::module::MAX_LOCALS + 1];
        let err = into_module(bad).unwrap_err();
        assert!(err.to_string().contains("exceed the maximum"), "{err}");
        let mut most = metadata();
        most.functions[1].locals = vec![Type::U64; crate::module::MAX_LOCALS];
        assert!(into_module(most).is_ok());

        // At least one local per parameter.
        let mut bad = metadata();
        bad.functions[1].signature.params = vec![Parameter {
            name: 0,
            ty: Type::U64,
        }];
        let err = into_module(bad).unwrap_err();
        assert!(
            err.to_string()
                .contains("metadata.functions[1].locals: 0 locals declared, but the signature has 1 parameter (locals start with one per parameter)"),
            "{err}"
        );

        // The leading locals are the parameter types.
        let mut bad = metadata();
        bad.functions[0].signature.params = vec![
            Parameter {
                name: 0,
                ty: Type::U64,
            },
            Parameter {
                name: 0,
                ty: Type::I64,
            },
        ];
        let err = into_module(bad).unwrap_err();
        assert!(
            err.to_string()
                .contains("metadata.functions[0].locals[1]: local 1 is declared f64, but parameter 1 (later) is i64 (locals start with the parameter types)"),
            "{err}"
        );
        let mut good = metadata();
        good.functions[0].signature.params = vec![Parameter {
            name: 0,
            ty: Type::U64,
        }];
        assert!(into_module(good).is_ok());
        assert_eq!(metadata().functions[0].local_count(), 3);
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
    fn validation_and_lowering_report_names_with_one_message_each() {
        // Both entry points build the same name tables, so they agree.
        let both = |metadata: &ProgramMetadata<vihaco_abi::Value, Type>| {
            let validated = metadata.validate(9).unwrap_err().to_string();
            let lowered = MetadataLoweringContext::new(metadata)
                .unwrap_err()
                .to_string();
            assert_eq!(validated, lowered);
            validated
        };
        let mut bad = metadata();
        bad.structs.push(bad.structs[0].clone());
        assert_eq!(
            both(&bad),
            "metadata.structs[1].name: duplicate struct `Point` (first declared at \
             metadata.structs[0].name)"
        );
        let mut bad = metadata();
        bad.structs[0].fields[1].name = 5;
        assert_eq!(
            both(&bad),
            "metadata.structs[0].fields[1].name: duplicate field `x` in struct `Point` (first \
             declared at metadata.structs[0].fields[0].name)"
        );
        let mut bad = metadata();
        bad.functions[1].name = 0;
        assert_eq!(
            both(&bad),
            "metadata.functions[1].name: duplicate function `later` (first declared at \
             metadata.functions[0].name)"
        );
        let mut bad = metadata();
        bad.types[1].name = 99;
        assert_eq!(
            both(&bad),
            "metadata.types[1].name: string 99 is outside the string table (length 9)"
        );
        let mut bad = metadata();
        bad.main_function = Some(4);
        assert_eq!(
            bad.validate(9).unwrap_err().to_string(),
            "metadata.main_function: function 4 is outside the function table (length 2)"
        );
        let mut bad = metadata();
        bad.functions[0].end_address = 10;
        assert_eq!(
            bad.validate(9).unwrap_err().to_string(),
            "metadata.functions[0]: instruction range 7..10 does not fit in the 9 \
             instructions of the section"
        );
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
        assert_eq!(converted.types, original.types);
        assert_eq!(converted.labels, original.labels);
        assert_eq!(converted.strings, original.strings);
        assert_eq!(converted.constants, original.constants);
        assert_eq!(converted.source_symbols, original.source_symbols);
        assert_eq!(converted.main_function, original.main_function);
    }
}

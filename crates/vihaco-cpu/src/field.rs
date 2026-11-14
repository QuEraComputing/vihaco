// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Struct field operands for `get_field` / `set_field` and their lowering.
//!
//! A field is written as a numeric slot index (`get_field 1`) or as
//! `@Struct.field` (`get_field @Point.y`). Symbols are resolved against the
//! section's struct table through [`StructLookup`] when the program is lowered.

use crate::cpu;
use vihaco_abi::{Lower, Syntax};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_parser::Ident;

/// A struct field slot index, or a `@Struct.field` symbol to resolve during loading.
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum FieldOperand {
    #[pattern = "$0"]
    Resolved(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

/// Resolve struct field names in the section currently being lowered.
pub trait StructLookup {
    /// The slot index of `field` within struct `struct_name`.
    fn field_index(&self, struct_name: &str, field: &str) -> eyre::Result<u32>;
}

impl StructLookup for MetadataLoweringContext {
    fn field_index(&self, struct_name: &str, field: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::field_index(self, struct_name, field)
    }
}

fn field_slot<C: StructLookup + ?Sized>(operand: FieldOperand, context: &C) -> eyre::Result<u32> {
    match operand {
        FieldOperand::Resolved(index) => Ok(index),
        FieldOperand::Symbol(symbol) => {
            let symbol = symbol.as_str();
            let (struct_name, field) = symbol
                .rsplit_once('.')
                .filter(|(struct_name, field)| !struct_name.is_empty() && !field.is_empty())
                .ok_or_else(|| {
                    eyre::eyre!("field operand `@{symbol}` must have the form `@Struct.field`")
                })?;
            context.field_index(struct_name, field)
        }
    }
}

impl<C: StructLookup + ?Sized> Lower<C> for cpu::GetField<Syntax> {
    type Runtime = cpu::GetField;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::GetField(field_slot(self.0, context)?))
    }
}

impl<C: StructLookup + ?Sized> Lower<C> for cpu::SetField<Syntax> {
    type Runtime = cpu::SetField;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::SetField(field_slot(self.0, context)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_runtime::{Parse, chumsky::Parser as _};

    struct Points;

    impl StructLookup for Points {
        fn field_index(&self, struct_name: &str, field: &str) -> eyre::Result<u32> {
            match (struct_name, field) {
                ("Point", "x") => Ok(0),
                ("Point", "y") => Ok(1),
                _ => Err(eyre::eyre!("no field {struct_name}.{field}")),
            }
        }
    }

    fn lower_get(text: &str) -> eyre::Result<cpu::GetField> {
        cpu::GetField::<Syntax>::parser()
            .parse(text)
            .into_result()
            .map_err(|errors| eyre::eyre!("{errors:?}"))?
            .lower(&mut Points)
    }

    #[test]
    fn field_operands_accept_slot_indices_and_struct_symbols() {
        assert_eq!(lower_get("get_field 3").unwrap(), cpu::GetField(3));
        assert_eq!(lower_get("get_field @Point.y").unwrap(), cpu::GetField(1));
        let set = cpu::SetField::<Syntax>::parser()
            .parse("set_field @Point.x")
            .into_result()
            .unwrap()
            .lower(&mut Points)
            .unwrap();
        assert_eq!(set, cpu::SetField(0));
    }

    #[test]
    fn malformed_or_unknown_field_symbols_are_rejected() {
        assert!(lower_get("get_field @Point.z").is_err());
        for symbol in ["@Point", "@.y", "@Point."] {
            let err = lower_get(&format!("get_field {symbol}")).unwrap_err();
            assert!(err.to_string().contains("@Struct.field"), "{err}");
        }
    }
}

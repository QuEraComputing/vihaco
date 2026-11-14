// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Struct operands for `struct_new`, `get_field`, and `set_field`, and their lowering.
//!
//! Structs are written by name (`struct_new @Point`, `get_field @Point.y`) or
//! by struct-table index (`struct_new 0`, `get_field 0, 1`). Both forms are
//! resolved against the section's struct table through [`StructLookup`] when
//! the program is lowered, so the [`StructLayout`] and [`FieldSlot`] stored in
//! the runtime instruction always agree with the table. The CPU then checks
//! each heap object against them at runtime.

use crate::{FieldSlot, StructLayout, cpu};
use vihaco_abi::{Lower, Syntax};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_parser::Ident;

/// A struct, by struct-table index or `@Struct` name.
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum StructOperand {
    #[pattern = "$0"]
    Resolved(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

/// A struct field, as `struct_index, slot` or `@Struct.field`.
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum FieldOperand {
    #[pattern = "$0 `,` $1"]
    Resolved(u32, u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

/// Resolve structs and fields against the section's struct table.
pub trait StructLookup {
    /// The struct-table index of the struct named `name`.
    fn struct_index(&self, name: &str) -> eyre::Result<u32>;
    /// The number of fields of struct-table entry `struct_index`.
    fn struct_field_count(&self, struct_index: u32) -> eyre::Result<u32>;
    /// The slot index of `field` within struct-table entry `struct_index`.
    fn field_index(&self, struct_index: u32, field: &str) -> eyre::Result<u32>;
}

impl StructLookup for MetadataLoweringContext {
    fn struct_index(&self, name: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::struct_index(self, name)
    }
    fn struct_field_count(&self, struct_index: u32) -> eyre::Result<u32> {
        MetadataLoweringContext::struct_field_count(self, struct_index)
    }
    fn field_index(&self, struct_index: u32, field: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::field_index(self, struct_index, field)
    }
}

fn struct_layout<C: StructLookup + ?Sized>(
    operand: StructOperand,
    context: &C,
) -> eyre::Result<StructLayout> {
    let struct_id = match operand {
        StructOperand::Resolved(index) => index,
        StructOperand::Symbol(name) => context.struct_index(name.as_str())?,
    };
    Ok(StructLayout {
        struct_id,
        field_count: context.struct_field_count(struct_id)?,
    })
}

fn field_slot<C: StructLookup + ?Sized>(
    operand: FieldOperand,
    context: &C,
) -> eyre::Result<FieldSlot> {
    match operand {
        FieldOperand::Resolved(struct_id, slot) => {
            let count = context.struct_field_count(struct_id)?;
            eyre::ensure!(
                slot < count,
                "struct {struct_id} has no field slot {slot} ({count} fields)"
            );
            Ok(FieldSlot { struct_id, slot })
        }
        FieldOperand::Symbol(symbol) => {
            let symbol = symbol.as_str();
            let (struct_name, field) = symbol
                .rsplit_once('.')
                .filter(|(struct_name, field)| !struct_name.is_empty() && !field.is_empty())
                .ok_or_else(|| {
                    eyre::eyre!("field operand `@{symbol}` must have the form `@Struct.field`")
                })?;
            let struct_id = context.struct_index(struct_name)?;
            Ok(FieldSlot {
                struct_id,
                slot: context.field_index(struct_id, field)?,
            })
        }
    }
}

impl<C: StructLookup + ?Sized> Lower<C> for cpu::StructNew<Syntax> {
    type Runtime = cpu::StructNew;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::StructNew(struct_layout(self.0, context)?))
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

    /// Struct 0 is `Point { x, y }`; struct 1 is `Unit {}`.
    struct Points;

    impl StructLookup for Points {
        fn struct_index(&self, name: &str) -> eyre::Result<u32> {
            match name {
                "Point" => Ok(0),
                "Unit" => Ok(1),
                _ => Err(eyre::eyre!("undefined struct `{name}`")),
            }
        }
        fn struct_field_count(&self, struct_index: u32) -> eyre::Result<u32> {
            match struct_index {
                0 => Ok(2),
                1 => Ok(0),
                _ => Err(eyre::eyre!(
                    "struct {struct_index} is outside the struct table"
                )),
            }
        }
        fn field_index(&self, struct_index: u32, field: &str) -> eyre::Result<u32> {
            match (struct_index, field) {
                (0, "x") => Ok(0),
                (0, "y") => Ok(1),
                _ => Err(eyre::eyre!("struct {struct_index} has no field `{field}`")),
            }
        }
    }

    fn lower<I>(text: &str) -> eyre::Result<I::Runtime>
    where
        I: for<'src> Parse<'src> + Lower<Points>,
    {
        I::parser()
            .parse(text)
            .into_result()
            .map_err(|errors| eyre::eyre!("{errors:?}"))?
            .lower(&mut Points)
    }

    fn slot(struct_id: u32, slot: u32) -> FieldSlot {
        FieldSlot { struct_id, slot }
    }

    #[test]
    fn struct_new_takes_its_field_count_from_the_table() {
        let point = StructLayout {
            struct_id: 0,
            field_count: 2,
        };
        assert_eq!(
            lower::<cpu::StructNew<Syntax>>("struct_new @Point").unwrap(),
            cpu::StructNew(point)
        );
        assert_eq!(
            lower::<cpu::StructNew<Syntax>>("struct_new 0").unwrap(),
            cpu::StructNew(point)
        );
        assert!(lower::<cpu::StructNew<Syntax>>("struct_new @Line").is_err());
        assert!(lower::<cpu::StructNew<Syntax>>("struct_new 7").is_err());
    }

    #[test]
    fn field_operands_resolve_names_and_check_numeric_slots() {
        assert_eq!(
            lower::<cpu::GetField<Syntax>>("get_field @Point.y").unwrap(),
            cpu::GetField(slot(0, 1))
        );
        assert_eq!(
            lower::<cpu::GetField<Syntax>>("get_field 0, 1").unwrap(),
            cpu::GetField(slot(0, 1))
        );
        assert_eq!(
            lower::<cpu::SetField<Syntax>>("set_field @Point.x").unwrap(),
            cpu::SetField(slot(0, 0))
        );
        let err = lower::<cpu::GetField<Syntax>>("get_field 0, 2").unwrap_err();
        assert!(err.to_string().contains("no field slot 2"), "{err}");
        assert!(lower::<cpu::GetField<Syntax>>("get_field 1, 0").is_err());
        assert!(lower::<cpu::GetField<Syntax>>("get_field 9, 0").is_err());
        assert!(lower::<cpu::GetField<Syntax>>("get_field @Point.z").is_err());
        assert!(lower::<cpu::GetField<Syntax>>("get_field @Line.x").is_err());
    }

    #[test]
    fn malformed_field_symbols_are_rejected() {
        for symbol in ["@Point", "@.y", "@Point."] {
            let err = lower::<cpu::GetField<Syntax>>(&format!("get_field {symbol}")).unwrap_err();
            assert!(err.to_string().contains("@Struct.field"), "{err}");
        }
    }
}

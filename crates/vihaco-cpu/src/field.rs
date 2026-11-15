// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Struct and type operands for the struct and array instructions, and their lowering.
//!
//! Structs are written by name (`struct_new @Point`, `get_field @Point.y`) or
//! by struct-table index (`struct_new 0`, `get_field 0, 1`). Array element
//! types are ordinary types, where a struct may also be named:
//! `array_new struct @Point, 4`, `array_get array f64`. Every form is resolved
//! against the section's struct table through [`StructLookup`] when the
//! program is lowered, so the [`StructLayout`], [`FieldSlot`], and element
//! [`Type`] stored in the runtime instruction always agree with the table. The
//! CPU then checks each heap object against them at runtime.

use crate::{FieldSlot, StructLayout, cpu};
use chumsky::{error::Simple, extra, prelude::*};
use vihaco_abi::{Lower, Syntax, Type};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_parser::{Ident, Parse};

/// A type as written in source: any [`Type`], where struct types may also be
/// named (`struct @Point`), at any nesting depth (`array array struct @Point`).
#[derive(Clone, Debug, PartialEq)]
pub enum TypeOperand {
    /// A type with no names to resolve, such as `f64` or `array struct 0`.
    Resolved(Type),
    /// `array <elem>`, where the element type may contain names.
    Array(Box<TypeOperand>),
    /// `struct @Name`.
    StructSymbol(Ident),
}

impl<'src> Parse<'src> for TypeOperand {
    fn parser() -> impl Parser<'src, &'src str, Self, extra::Err<Simple<'src, char>>> {
        // `array` prefixes are counted rather than parsed recursively, so deep
        // nesting is rejected (past `Type::MAX_ARRAY_DEPTH`) instead of
        // overflowing the stack.
        let prefixes = just("array")
            .then(text::whitespace().at_least(1))
            .repeated()
            .count();
        let named_struct = just("struct")
            .then(text::whitespace().at_least(1))
            .then(just('@'))
            .ignore_then(vihaco_parser::ident())
            .map(|name| TypeOperand::StructSymbol(Ident(name)));
        let element = choice((named_struct, Type::parser().map(TypeOperand::Resolved)));
        prefixes.then(element).try_map(|(depth, elem), span| {
            TypeOperand::nested(depth, elem).ok_or(Simple::new(None, span))
        })
    }
}

impl TypeOperand {
    /// Wrap `elem` in `depth` array levels, or `None` past `Type::MAX_ARRAY_DEPTH`.
    fn nested(depth: usize, elem: TypeOperand) -> Option<TypeOperand> {
        match elem {
            // Keep fully resolved types in one piece.
            TypeOperand::Resolved(ty) => Type::nested_array(depth, ty)
                .ok()
                .map(TypeOperand::Resolved),
            elem => (depth <= Type::MAX_ARRAY_DEPTH)
                .then(|| (0..depth).fold(elem, |ty, _| TypeOperand::Array(Box::new(ty)))),
        }
    }
}

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

/// Resolve struct names in a type, and check that every struct type it refers
/// to (named or numeric) exists in the struct table.
fn resolve_type<C: StructLookup + ?Sized>(operand: TypeOperand, context: &C) -> eyre::Result<Type> {
    let ty = match operand {
        TypeOperand::Resolved(ty) => ty,
        TypeOperand::Array(elem) => Type::array(resolve_type(*elem, context)?),
        TypeOperand::StructSymbol(name) => Type::Struct(context.struct_index(name.as_str())?),
    };
    let mut result = Ok(());
    ty.visit_struct_refs(&mut |index| {
        if result.is_ok() {
            result = context.struct_field_count(index).map(drop);
        }
    });
    result.map(|()| ty)
}

impl<C: StructLookup + ?Sized> Lower<C> for cpu::ArrayNew<Syntax> {
    type Runtime = cpu::ArrayNew;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::ArrayNew(resolve_type(self.0, context)?, self.1))
    }
}

macro_rules! lower_element_type {
    ($($instruction:ident),* $(,)?) => {$(
        impl<C: StructLookup + ?Sized> Lower<C> for cpu::$instruction<Syntax> {
            type Runtime = cpu::$instruction;
            fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
                Ok(cpu::$instruction(resolve_type(self.0, context)?))
            }
        }
    )*};
}

lower_element_type!(ArrayGet, ArraySet, ArrayReserve, ArrayPush);

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_runtime::Parse;

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
    fn element_types_resolve_struct_names_at_any_depth() {
        assert_eq!(
            lower::<cpu::ArrayNew<Syntax>>("array_new f64, 3").unwrap(),
            cpu::ArrayNew(Type::F64, 3)
        );
        assert_eq!(
            lower::<cpu::ArrayNew<Syntax>>("array_new struct @Point, 4").unwrap(),
            cpu::ArrayNew(Type::Struct(0), 4)
        );
        assert_eq!(
            lower::<cpu::ArrayGet<Syntax>>("array_get array array struct @Unit").unwrap(),
            cpu::ArrayGet(Type::array(Type::array(Type::Struct(1))))
        );
        assert_eq!(
            lower::<cpu::ArrayPush<Syntax>>("array_push array struct 0").unwrap(),
            cpu::ArrayPush(Type::array(Type::Struct(0)))
        );
        assert_eq!(
            lower::<cpu::ArraySet<Syntax>>("array_set u64").unwrap(),
            cpu::ArraySet(Type::U64)
        );
        assert_eq!(
            lower::<cpu::ArrayReserve<Syntax>>("array_reserve str").unwrap(),
            cpu::ArrayReserve(Type::String)
        );
    }

    #[test]
    fn element_types_must_name_existing_structs() {
        let err = lower::<cpu::ArrayNew<Syntax>>("array_new struct @Line, 1").unwrap_err();
        assert!(err.to_string().contains("undefined struct `Line`"), "{err}");
        // Numeric struct types are checked against the table too, even nested.
        assert!(lower::<cpu::ArrayGet<Syntax>>("array_get struct 7").is_err());
        assert!(lower::<cpu::ArrayGet<Syntax>>("array_get array struct 7").is_err());
        assert!(lower::<cpu::ArrayNew<Syntax>>("array_new nope, 1").is_err());
    }

    #[test]
    fn element_type_nesting_is_limited() {
        let max = Type::MAX_ARRAY_DEPTH;
        let at_limit = format!("array_get {}struct @Point", "array ".repeat(max));
        let lowered = lower::<cpu::ArrayGet<Syntax>>(&at_limit).unwrap();
        assert_eq!(lowered.0.array_depth(), max);
        for elem in ["struct @Point", "u64"] {
            let past = format!("array_get {}{elem}", "array ".repeat(max + 1));
            assert!(lower::<cpu::ArrayGet<Syntax>>(&past).is_err(), "{elem}");
        }
        let hostile = format!("array_get {}u64", "array ".repeat(200_000));
        assert!(lower::<cpu::ArrayGet<Syntax>>(&hostile).is_err());
    }

    #[test]
    fn malformed_field_symbols_are_rejected() {
        for symbol in ["@Point", "@.y", "@Point."] {
            let err = lower::<cpu::GetField<Syntax>>(&format!("get_field {symbol}")).unwrap_err();
            assert!(err.to_string().contains("@Struct.field"), "{err}");
        }
    }
}

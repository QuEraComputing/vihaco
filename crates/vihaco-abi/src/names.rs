// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Showing types and instructions by name.
//!
//! A [`Type`] refers to structs and named types by table index (`struct 0`,
//! `type 1`). Messages meant for people should show the names the program
//! declared instead (`struct Point`, `type location`). [`TypeNames`] looks the
//! names up, typically in a module's struct and named-type tables, and
//! [`Type::display_with`] and [`FunctionType::display_with`] render with them.
//! An index without a name falls back to the numeric form.
//!
//! Instructions render the same way: [`InstructionText`] writes an
//! instruction in its source syntax (`set_field @Point.y`, `call 1, @helper`),
//! with each operand written by its syntax type's [`OperandText`] impl and
//! names looked up through [`SymbolNames`]. Typed dialects generate
//! `InstructionText` from each instruction's `#[pattern]`, and composites
//! from their leaves.

use std::fmt;
use std::marker::PhantomData;

use crate::word::{Word, decode_bool, decode_u32};
use crate::{FunctionType, Type};

/// Names for the struct and named-type tables, used to show types by name.
///
/// `vihaco-module` implements this for modules and program metadata.
pub trait TypeNames {
    /// The name of struct-table entry `index`, if it has one.
    fn struct_name(&self, index: u32) -> Option<&str>;

    /// The name of named-type-table entry `index`, if it has one.
    fn named_type_name(&self, index: u32) -> Option<&str>;
}

/// No names: every type renders in its numeric form, as [`Type`]'s own
/// `Display` does.
impl TypeNames for () {
    fn struct_name(&self, _: u32) -> Option<&str> {
        None
    }

    fn named_type_name(&self, _: u32) -> Option<&str> {
        None
    }
}

impl<T: TypeNames + ?Sized> TypeNames for &T {
    fn struct_name(&self, index: u32) -> Option<&str> {
        (**self).struct_name(index)
    }

    fn named_type_name(&self, index: u32) -> Option<&str> {
        (**self).named_type_name(index)
    }
}

impl Type {
    /// Display this type with struct and named types shown by name:
    /// `struct Point`, `array type location`. Indices without a name keep
    /// their numeric form.
    ///
    /// ```
    /// # use vihaco_abi::{Type, names::TypeNames};
    /// struct Tables;
    /// impl TypeNames for Tables {
    ///     fn struct_name(&self, index: u32) -> Option<&str> {
    ///         (index == 0).then_some("Point")
    ///     }
    ///     fn named_type_name(&self, _: u32) -> Option<&str> {
    ///         None
    ///     }
    /// }
    /// let ty = Type::array(Type::Struct(0));
    /// assert_eq!(ty.display_with(&Tables).to_string(), "array struct Point");
    /// assert_eq!(Type::Struct(4).display_with(&Tables).to_string(), "struct 4");
    /// ```
    pub fn display_with<'a>(&'a self, names: &'a dyn TypeNames) -> impl fmt::Display + 'a {
        Named(move |f: &mut fmt::Formatter<'_>| write_type(f, self, Some(names)))
    }
}

impl FunctionType {
    /// Display this function type with its struct and named types shown by
    /// name, as [`Type::display_with`] does: `(struct Point) -> f64`.
    pub fn display_with<'a>(&'a self, names: &'a dyn TypeNames) -> impl fmt::Display + 'a {
        Named(move |f: &mut fmt::Formatter<'_>| write_function_type(f, self, Some(names)))
    }
}

/// Names for instruction operands: types (through [`TypeNames`]), struct
/// fields, functions, labels and strings.
///
/// Every method defaults to "no name", so an operand renders in its numeric
/// source form. `vihaco-module` implements this for modules and program
/// metadata.
pub trait SymbolNames: TypeNames {
    /// The name of field `field` of struct `struct_id`.
    fn field_name(&self, struct_id: u32, field: u32) -> Option<&str> {
        let _ = (struct_id, field);
        None
    }

    /// The name of function `function`, without the leading `@`.
    fn function_name(&self, function: u32) -> Option<&str> {
        let _ = function;
        None
    }

    /// The function whose first instruction is at `address` (the first such
    /// entry in table order, as a direct `call` resolves it).
    fn function_starting_at(&self, address: u32) -> Option<u32> {
        let _ = address;
        None
    }

    /// The name of a label at `address`, without the leading `@`.
    fn label_at(&self, address: u32) -> Option<&str> {
        let _ = address;
        None
    }

    /// String-table entry `index`.
    fn string(&self, index: u32) -> Option<&str> {
        let _ = index;
        None
    }

    /// `@name` for a jump or call target at `address`: the function starting
    /// there, else a label there, else the address itself.
    fn write_target(&self, address: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .function_starting_at(address)
            .and_then(|function| self.function_name(function))
            .or_else(|| self.label_at(address));
        match name {
            Some(name) => write!(f, "@{name}"),
            None => write!(f, "{address}"),
        }
    }
}

/// No names: every operand renders in its numeric form.
impl SymbolNames for () {}

impl<T: SymbolNames + ?Sized> SymbolNames for &T {
    fn field_name(&self, struct_id: u32, field: u32) -> Option<&str> {
        (**self).field_name(struct_id, field)
    }

    fn function_name(&self, function: u32) -> Option<&str> {
        (**self).function_name(function)
    }

    fn function_starting_at(&self, address: u32) -> Option<u32> {
        (**self).function_starting_at(address)
    }

    fn label_at(&self, address: u32) -> Option<&str> {
        (**self).label_at(address)
    }

    fn string(&self, index: u32) -> Option<&str> {
        (**self).string(index)
    }
}

/// How a syntax operand type writes the runtime value it lowers to, in source
/// syntax.
///
/// `Self` is the operand's syntax type (the `T` in a pattern's `$0:T`), and
/// `Runtime` the instruction field it lowers to. For example the CPU's
/// `FieldOperand` writes a runtime `FieldSlot` as `@Point.y`, or `0, 1` when
/// the struct has no name. Operands whose syntax type has no impl fall back
/// to the runtime value's `Display`, then its `Debug`.
pub trait OperandText<Runtime: ?Sized> {
    /// Write `value` as this operand type's source syntax.
    fn write_operand(
        value: &Runtime,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result;
}

/// Writes the type in source syntax, by name where `names` has one.
impl OperandText<Type> for Type {
    fn write_operand(
        value: &Type,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write_type(f, value, Some(names))
    }
}

/// Writes the function type in source syntax, by name where `names` has one.
impl OperandText<FunctionType> for FunctionType {
    fn write_operand(
        value: &FunctionType,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write_function_type(f, value, Some(names))
    }
}

/// An encoded `bool` word, as `true` or `false`.
impl OperandText<Word> for bool {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", decode_bool(*value))
    }
}

/// An encoded `u32` word.
impl OperandText<Word> for u32 {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", decode_u32(*value))
    }
}

/// An instruction written in its source syntax, with operands named through
/// [`SymbolNames`].
///
/// Typed dialects generate this from each instruction's `#[pattern]`, and
/// composites from their leaves (prefixed with the instruction's route as SST
/// writes it, `cpu::cpu::set_field @Point.y`). A composite instruction's
/// `Display` is its text without names.
pub trait InstructionText {
    /// Write this instruction in source syntax, naming operands through `names`.
    fn write_text(&self, names: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    /// This instruction's text with names, as a `Display` value.
    fn text_with<'a>(&'a self, names: &'a dyn SymbolNames) -> impl fmt::Display + 'a
    where
        Self: Sized,
    {
        Named(move |f: &mut fmt::Formatter<'_>| self.write_text(names, f))
    }
}

/// Text that is already source syntax writes itself, for hand-written
/// fixtures and tools that keep instructions as text.
impl InstructionText for str {
    fn write_text(&self, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl<T: InstructionText + ?Sized> InstructionText for &T {
    fn write_text(&self, names: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).write_text(names, f)
    }
}

/// Support for generated [`InstructionText`] impls: picks [`OperandText`] for
/// an operand when its syntax type has one, else the runtime value's
/// `Display`, else its `Debug`, by autoref-based method selection.
#[doc(hidden)]
pub mod operand_text {
    use super::*;

    /// One operand: the runtime `value`, written as syntax type `S`.
    pub struct OperandRef<'a, S: ?Sized, R: ?Sized>(pub &'a R, pub PhantomData<fn() -> *const S>);

    impl<'a, S: ?Sized, R: ?Sized> OperandRef<'a, S, R> {
        pub fn new(value: &'a R) -> Self {
            OperandRef(value, PhantomData)
        }
    }

    pub trait ViaOperandText {
        fn write_operand_text(
            &self,
            names: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result;
    }

    impl<S: OperandText<R> + ?Sized, R: ?Sized> ViaOperandText for &&OperandRef<'_, S, R> {
        fn write_operand_text(
            &self,
            names: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            S::write_operand(self.0, names, f)
        }
    }

    pub trait ViaDisplay {
        fn write_operand_text(
            &self,
            names: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result;
    }

    impl<S: ?Sized, R: fmt::Display + ?Sized> ViaDisplay for &OperandRef<'_, S, R> {
        fn write_operand_text(
            &self,
            _: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            fmt::Display::fmt(self.0, f)
        }
    }

    pub trait ViaDebug {
        fn write_operand_text(
            &self,
            names: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result;
    }

    impl<S: ?Sized, R: fmt::Debug + ?Sized> ViaDebug for OperandRef<'_, S, R> {
        fn write_operand_text(
            &self,
            _: &dyn SymbolNames,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            fmt::Debug::fmt(self.0, f)
        }
    }
}

/// A `Display` adapter over a formatting closure.
struct Named<F>(F);

impl<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result> fmt::Display for Named<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}

pub(crate) fn write_type(
    f: &mut fmt::Formatter<'_>,
    ty: &Type,
    names: Option<&dyn TypeNames>,
) -> fmt::Result {
    match ty {
        Type::Undefined => f.write_str("undefined"),
        Type::String => f.write_str("str"),
        Type::Bool => f.write_str("bool"),
        Type::I64 => f.write_str("i64"),
        Type::I32 => f.write_str("i32"),
        Type::U32 => f.write_str("u32"),
        Type::U64 => f.write_str("u64"),
        Type::F64 => f.write_str("f64"),
        Type::F32 => f.write_str("f32"),
        Type::FunctionRef => f.write_str("fn"),
        Type::HeapRef => f.write_str("heap"),
        Type::Struct(index) => match names.and_then(|names| names.struct_name(*index)) {
            Some(name) => write!(f, "struct {name}"),
            None => write!(f, "struct {index}"),
        },
        Type::Array(elem) => {
            f.write_str("array ")?;
            write_type(f, elem, names)
        }
        Type::Named(index) => match names.and_then(|names| names.named_type_name(*index)) {
            Some(name) => write!(f, "type {name}"),
            None => write!(f, "type {index}"),
        },
    }
}

pub(crate) fn write_function_type(
    f: &mut fmt::Formatter<'_>,
    ty: &FunctionType,
    names: Option<&dyn TypeNames>,
) -> fmt::Result {
    let list = |f: &mut fmt::Formatter<'_>, types: &[Type]| {
        f.write_str("(")?;
        for (index, ty) in types.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write_type(f, ty, names)?;
        }
        f.write_str(")")
    };
    list(f, &ty.params)?;
    f.write_str(" -> ")?;
    match ty.results.as_slice() {
        [single] => write_type(f, single, names),
        results => list(f, results),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tables;

    impl TypeNames for Tables {
        fn struct_name(&self, index: u32) -> Option<&str> {
            ["Point", "Pair"].get(index as usize).copied()
        }

        fn named_type_name(&self, index: u32) -> Option<&str> {
            ["location"].get(index as usize).copied()
        }
    }

    #[test]
    fn types_show_table_names_and_fall_back_to_indices() {
        let names = |ty: &Type| ty.display_with(&Tables).to_string();
        assert_eq!(names(&Type::Struct(1)), "struct Pair");
        assert_eq!(names(&Type::Struct(7)), "struct 7");
        assert_eq!(names(&Type::array(Type::Named(0))), "array type location");
        assert_eq!(names(&Type::Named(3)), "type 3");
        assert_eq!(names(&Type::F64), "f64");
        assert_eq!(Type::Struct(1).display_with(&()).to_string(), "struct 1");
        let ty = FunctionType::new([Type::Struct(0), Type::Named(0)], [Type::U64]);
        assert_eq!(
            ty.display_with(&Tables).to_string(),
            "(struct Point, type location) -> u64"
        );
        assert_eq!(ty.to_string(), "(struct 0, type 0) -> u64");
        assert_eq!(FunctionType::new([], []).to_string(), "() -> ()");
    }
}

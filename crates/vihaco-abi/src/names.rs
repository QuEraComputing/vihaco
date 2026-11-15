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

    /// The name of a label at `address`, without the leading `@`, that a
    /// jump in the instruction being written may name. Labels resolve per
    /// function, so a label of another function must not be returned.
    fn label_at(&self, address: u32) -> Option<&str> {
        let _ = address;
        None
    }

    /// String-table entry `index`.
    fn string(&self, index: u32) -> Option<&str> {
        let _ = index;
        None
    }

    /// A call target at `address`, as a call resolves it: `@name` of the
    /// function starting there, else the address itself.
    fn write_call_target(&self, address: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .function_starting_at(address)
            .and_then(|function| self.function_name(function));
        write_symbol_or_number(name, address, f)
    }

    /// A jump target at `address`, as a jump resolves it: `@name` of a label
    /// there, else the address itself. Never a function name, which a jump
    /// cannot name, even when the address is a function's first instruction.
    fn write_jump_target(&self, address: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_symbol_or_number(self.label_at(address), address, f)
    }

    /// Function `function` (a function reference), as `@name`, else its index.
    fn write_function(&self, function: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_symbol_or_number(self.function_name(function), function, f)
    }
}

fn write_symbol_or_number(
    name: Option<&str>,
    number: u32,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match name {
        Some(name) => write!(f, "@{name}"),
        None => write!(f, "{number}"),
    }
}

/// Write `text` as an SST string literal that parses back to `text`.
///
/// The SST lexer accepts any character raw inside quotes except `"` and `\`,
/// and the escapes `\"`, `\\`, `\n`, `\t`, `\r` and `\0`. So those characters
/// are escaped (line breaks, tabs and NUL too, to keep the literal on one
/// line), and everything else, combining marks and other control characters
/// included, is written raw.
///
/// ```
/// # use vihaco_abi::names::StringLiteral;
/// assert_eq!(StringLiteral("say \"hi\"\n").to_string(), r#""say \"hi\"\n""#);
/// assert_eq!(StringLiteral("e\u{301}").to_string(), "\"e\u{301}\"");
/// ```
pub struct StringLiteral<'a>(pub &'a str);

impl fmt::Display for StringLiteral<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;
        f.write_char('"')?;
        for ch in self.0.chars() {
            match ch {
                '"' => f.write_str("\\\"")?,
                '\\' => f.write_str("\\\\")?,
                '\n' => f.write_str("\\n")?,
                '\t' => f.write_str("\\t")?,
                '\r' => f.write_str("\\r")?,
                '\0' => f.write_str("\\0")?,
                ch => f.write_char(ch)?,
            }
        }
        f.write_char('"')
    }
}

/// Write a float as an SST literal that parses back to the same value.
///
/// Finite values, `-0.0` included, use their shortest round-tripping form.
/// The SST float syntax has no infinity, so infinities are written as
/// `1e999` and `-1e999`, which overflow back to them. NaN has no SST
/// literal at all; it is written as `NaN`, which does not parse.
///
/// ```
/// # use vihaco_abi::names::FloatLiteral;
/// assert_eq!(FloatLiteral(2.5f64).to_string(), "2.5");
/// assert_eq!(FloatLiteral(-0.0f64).to_string(), "-0.0");
/// assert_eq!(FloatLiteral(f64::INFINITY).to_string(), "1e999");
/// assert_eq!(FloatLiteral(f32::NEG_INFINITY).to_string(), "-1e999");
/// ```
pub struct FloatLiteral<T>(pub T);

impl fmt::Display for FloatLiteral<f64> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_float(f, self.0.is_infinite(), self.0.is_sign_negative(), &self.0)
    }
}

impl fmt::Display for FloatLiteral<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_float(f, self.0.is_infinite(), self.0.is_sign_negative(), &self.0)
    }
}

fn write_float(
    f: &mut fmt::Formatter<'_>,
    infinite: bool,
    negative: bool,
    value: &dyn fmt::Debug,
) -> fmt::Result {
    match (infinite, negative) {
        (true, false) => f.write_str("1e999"),
        (true, true) => f.write_str("-1e999"),
        // `Debug` keeps the `.0` of integral values and the sign of `-0.0`.
        _ => write!(f, "{value:?}"),
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
/// to the runtime value's `Display`; an operand with neither is a compile
/// error at the instruction's derive.
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

/// A float as a literal that parses back (see [`FloatLiteral`]).
impl OperandText<f64> for f64 {
    fn write_operand(value: &f64, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", FloatLiteral(*value))
    }
}

/// A float as a literal that parses back (see [`FloatLiteral`]).
impl OperandText<f32> for f32 {
    fn write_operand(value: &f32, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", FloatLiteral(*value))
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
/// `Display`, by autoref-based method selection. An operand with neither does
/// not compile: its text could not be parsed back.
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
}

/// A type that can show itself with the module's names, for messages in code
/// generic over the type model: [`Type`] and [`FunctionType`] show struct and
/// named types by name (as [`Type::display_with`] does).
pub trait DisplayNamed {
    /// Write this type, by name where `names` has one.
    fn fmt_named(&self, names: &dyn TypeNames, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    /// This type with names, as a `Display` value.
    fn named<'a>(&'a self, names: &'a dyn TypeNames) -> impl fmt::Display + 'a
    where
        Self: Sized,
    {
        Named(move |f: &mut fmt::Formatter<'_>| self.fmt_named(names, f))
    }
}

impl DisplayNamed for Type {
    fn fmt_named(&self, names: &dyn TypeNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_type(f, self, Some(names))
    }
}

impl DisplayNamed for FunctionType {
    fn fmt_named(&self, names: &dyn TypeNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_function_type(f, self, Some(names))
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

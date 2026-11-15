// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Showing types by name.
//!
//! A [`Type`] refers to structs and named types by table index (`struct 0`,
//! `type 1`). Messages meant for people should show the names the program
//! declared instead (`struct Point`, `type location`). [`TypeNames`] looks the
//! names up, typically in a module's struct and named-type tables, and
//! [`Type::display_with`] and [`FunctionType::display_with`] render with them.
//! An index without a name falls back to the numeric form.

use std::fmt;

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

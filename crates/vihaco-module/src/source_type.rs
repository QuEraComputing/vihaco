// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Types as written in source, where structs and named types may be named.
//!
//! [`SourceType`] accepts every [`Type`] form plus `struct @Point` and
//! `type @location`, at any nesting depth (`array struct @Point`). Parse a
//! header and function signatures with it, then resolve the names against the
//! section's own tables with [`ProgramMetadata::resolve_types`]; the
//! signature check during lowering resolves function headers the same way
//! (through [`ResolveType`]).

use std::collections::HashMap;
use std::fmt;

use chumsky::{error::Simple, extra, prelude::*};
use vihaco_abi::Type;
use vihaco_parser::{Ident, Parse};

use crate::module::ProgramMetadata;
use vihaco_abi::names::{DisplayNamed, TypeNames};

/// A type as written in source: any [`Type`], where struct and named types may
/// also be named (`struct @Point`, `type @location`), at any nesting depth
/// (`array array struct @Point`).
///
/// ```
/// # use vihaco_module::module::SourceType;
/// # use vihaco_abi::traits::FromText;
/// use vihaco_parser::Parse as _;
/// use chumsky::Parser as _;
/// let ty = SourceType::parser().parse("array struct @Point").into_result().unwrap();
/// assert_eq!(ty.to_string(), "array struct @Point");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SourceType {
    /// A type with no names to resolve, such as `f64` or `array struct 0`.
    Resolved(Type),
    /// `array <elem>`, where the element type contains names.
    Array(Box<SourceType>),
    /// `struct @Name`.
    StructSymbol(Ident),
    /// `type @name`.
    NamedSymbol(Ident),
}

impl From<Type> for SourceType {
    fn from(ty: Type) -> Self {
        SourceType::Resolved(ty)
    }
}

impl<'src> Parse<'src> for SourceType {
    fn parser() -> impl Parser<'src, &'src str, Self, extra::Err<Simple<'src, char>>> {
        // `array` prefixes are counted rather than parsed recursively, so a
        // type nested past `Type::MAX_ARRAY_DEPTH` is rejected instead of
        // overflowing the stack.
        let prefixes = just("array")
            .then(text::whitespace().at_least(1))
            .repeated()
            .count();
        let named_struct = just("struct")
            .then(text::whitespace().at_least(1))
            .then(just('@'))
            .ignore_then(vihaco_parser::ident())
            .map(|name| SourceType::StructSymbol(Ident(name)));
        let named_type = just("type")
            .then(text::whitespace().at_least(1))
            .then(just('@'))
            .ignore_then(vihaco_parser::ident())
            .map(|name| SourceType::NamedSymbol(Ident(name)));
        let element = choice((
            named_struct,
            named_type,
            Type::parser().map(SourceType::Resolved),
        ));
        prefixes.then(element).try_map(|(depth, elem), span| {
            SourceType::nested(depth, elem).ok_or(Simple::new(None, span))
        })
    }
}

impl SourceType {
    /// Wrap `elem` in `depth` array levels, or `None` past
    /// [`Type::MAX_ARRAY_DEPTH`].
    fn nested(depth: usize, elem: SourceType) -> Option<SourceType> {
        match elem {
            // Keep fully resolved types in one piece.
            SourceType::Resolved(ty) => {
                Type::nested_array(depth, ty).ok().map(SourceType::Resolved)
            }
            elem => (depth <= Type::MAX_ARRAY_DEPTH)
                .then(|| (0..depth).fold(elem, |ty, _| SourceType::Array(Box::new(ty)))),
        }
    }
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceType::Resolved(ty) => write!(f, "{ty}"),
            SourceType::Array(elem) => write!(f, "array {elem}"),
            SourceType::StructSymbol(name) => write!(f, "struct @{}", name.as_str()),
            SourceType::NamedSymbol(name) => write!(f, "type @{}", name.as_str()),
        }
    }
}

/// A source type already names what it refers to.
impl DisplayNamed for SourceType {
    fn fmt_named(&self, _: &dyn TypeNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Struct and named-type names of a section, for resolving [`SourceType`]s.
#[derive(Debug, Default)]
pub struct TypeLookup {
    structs: HashMap<String, u32>,
    types: HashMap<String, u32>,
}

impl TypeLookup {
    /// The struct and named-type names of `metadata`, checked as
    /// [`ProgramMetadata::validate`] checks them (it builds the same name
    /// tables): a name outside the string table, or a duplicate, is an error
    /// at its metadata path rather than silently resolving to one entry.
    pub fn new<V, Ty, Info>(metadata: &ProgramMetadata<V, Ty, Info>) -> eyre::Result<Self> {
        let tables = metadata.name_tables()?;
        Ok(Self {
            structs: tables.struct_names,
            types: tables.type_names,
        })
    }

    /// The struct-table index of the struct named `name`.
    pub fn struct_index(&self, name: &str) -> Option<u32> {
        self.structs.get(name).copied()
    }

    /// The named-type-table index of the named type called `name`.
    pub fn named_type_index(&self, name: &str) -> Option<u32> {
        self.types.get(name).copied()
    }
}

/// A source type that resolves to the type model `T` against a section's
/// tables.
///
/// Every type resolves to itself, so code generic over the source type
/// accepts both plain [`Type`]s and [`SourceType`]s.
pub trait ResolveType<T> {
    /// Resolve the names in this type.
    ///
    /// # Errors
    /// A name with no table entry: `undefined struct `@Pointt``.
    fn resolve_type(&self, lookup: &TypeLookup) -> eyre::Result<T>;
}

impl<T: Clone> ResolveType<T> for T {
    fn resolve_type(&self, _: &TypeLookup) -> eyre::Result<T> {
        Ok(self.clone())
    }
}

impl ResolveType<Type> for SourceType {
    fn resolve_type(&self, lookup: &TypeLookup) -> eyre::Result<Type> {
        Ok(match self {
            SourceType::Resolved(ty) => ty.clone(),
            SourceType::Array(elem) => {
                Type::array(ResolveType::<Type>::resolve_type(&**elem, lookup)?)
            }
            SourceType::StructSymbol(name) => Type::Struct(
                lookup
                    .struct_index(name.as_str())
                    .ok_or_else(|| eyre::eyre!("undefined struct `@{}`", name.as_str()))?,
            ),
            SourceType::NamedSymbol(name) => Type::Named(
                lookup
                    .named_type_index(name.as_str())
                    .ok_or_else(|| eyre::eyre!("undefined named type `@{}`", name.as_str()))?,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{FieldInfo, FunctionInfo, Parameter, ProgramMetadata, Signature};
    use vihaco_abi::Value;
    use vihaco_abi::traits::FromText;

    fn parse(text: &str) -> SourceType {
        SourceType::parser().parse(text).into_result().unwrap()
    }

    #[test]
    fn source_types_accept_names_at_any_depth() {
        assert_eq!(parse("f64"), SourceType::Resolved(Type::F64));
        assert_eq!(
            parse("array struct 0"),
            SourceType::Resolved(Type::array(Type::Struct(0)))
        );
        let nested = parse("array array struct @Point");
        assert_eq!(nested.to_string(), "array array struct @Point");
        assert_eq!(parse("type @location").to_string(), "type @location");
    }

    #[test]
    fn source_type_nesting_is_limited() {
        let max = Type::MAX_ARRAY_DEPTH;
        let deep = parse(&format!("{}struct @Point", "array ".repeat(max)));
        assert!(matches!(deep, SourceType::Array(_)));
        for elem in ["struct @Point", "u64"] {
            let past = format!("{}{elem}", "array ".repeat(max + 1));
            assert!(SourceType::parser().parse(&past).into_result().is_err());
        }
        let hostile = format!("{}u64", "array ".repeat(200_000));
        assert!(SourceType::parser().parse(&hostile).into_result().is_err());
    }

    #[test]
    fn headers_resolve_names_against_their_own_tables() {
        let text = "metadata {
            functions: [{ name: 0, signature: { params: [{ name: 1, ty: struct @Point }],
                                                ret: [type @location] },
                          locals: [struct @Point, array type @location],
                          start_address: 0, end_address: 1, file: 0 }],
            structs: [{ name: 2, fields: [{ name: 3, ty: array struct @Point }] }],
            types: [{ name: 4, repr: u64 }],
            labels: [], constants: [],
            strings: [\"main\", \"p0\", \"Point\", \"next\", \"location\"],
            main_function: 0, source_symbols: [], extra: {},
        }";
        let metadata = ProgramMetadata::<Value, SourceType>::from_text(text)
            .unwrap()
            .resolve_types::<Type>()
            .unwrap();
        let expected = FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![Parameter {
                    name: 1,
                    ty: Type::Struct(0),
                }],
                ret: vec![Type::Named(0)],
            },
            locals: vec![Type::Struct(0), Type::array(Type::Named(0))],
            start_address: 0,
            end_address: 1,
            file: 0,
        };
        assert_eq!(metadata.functions, [expected]);
        assert_eq!(
            metadata.structs[0].fields,
            [FieldInfo {
                name: 3,
                ty: Type::array(Type::Struct(0))
            }]
        );
        metadata.into_module::<()>(vec![()]).unwrap();

        let typo = text.replace("locals: [struct @Point", "locals: [struct @Pointt");
        let err = ProgramMetadata::<Value, SourceType>::from_text(&typo)
            .unwrap()
            .resolve_types::<Type>()
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "metadata.functions[0].locals[0]: undefined struct `@Pointt`"
        );
    }
}

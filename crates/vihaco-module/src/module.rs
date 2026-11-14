// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::color::Themed;
pub use crate::program_metadata::ProgramMetadata;

/// A module for a single section.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalModule<I, V, Ty, Info = NoInfo> {
    pub code: Vec<I>,
    pub functions: Vec<FunctionInfo<Ty>>,
    pub structs: Vec<StructInfo<Ty>>,
    pub types: Vec<NamedTypeInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub constants: Vec<V>,
    pub strings: Vec<String>,
    pub main_function: Option<u32>,
    pub source_symbols: Vec<SourceSymbolInfo>,
    pub extra: Info,
}

impl<I, V, Ty, Info> Default for LocalModule<I, V, Ty, Info>
where
    Info: Default,
{
    fn default() -> Self {
        Self {
            code: Vec::new(),
            functions: Vec::new(),
            structs: Vec::new(),
            types: Vec::new(),
            labels: Vec::new(),
            constants: Vec::new(),
            strings: Vec::new(),
            main_function: None,
            source_symbols: Vec::new(),
            extra: Info::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct SourceSymbolInfo {
    pub index: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct FunctionInfo<Type> {
    pub name: u32, // index into the string interner
    pub signature: Signature<Type>,
    /// The declared type of every local slot, parameters first: the leading
    /// `signature.params.len()` entries are the parameter types. Written
    /// `locals: [u64, f64]` in metadata headers. At most [`MAX_LOCALS`]
    /// entries, checked by [`ProgramMetadata::validate`].
    pub locals: Vec<Type>,
    pub start_address: u32, // corresponds to a label noop
    pub end_address: u32,   // corresponds to a label noop
    pub file: u32,
}

/// The largest number of local slots, parameters included, a function may
/// declare. [`ProgramMetadata::validate`] enforces it, and runtime frame entry
/// re-checks it before allocating, since modules can also be built directly.
pub const MAX_LOCALS: usize = 65_536;

impl<Type> FunctionInfo<Type> {
    /// The number of local slots, parameters included (`locals.len()`).
    pub fn local_count(&self) -> usize {
        self.locals.len()
    }
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct Signature<Ty> {
    pub params: Vec<Parameter<Ty>>,
    pub ret: Vec<Ty>,
}

impl<Ty> Signature<Ty> {
    /// Whether this signature has exactly the parameter and result types of
    /// `ty`, ignoring parameter names; `call_indirect` checks its target this
    /// way before calling.
    pub fn matches(&self, ty: &vihaco_abi::FunctionType) -> bool
    where
        Ty: PartialEq<vihaco_abi::Type>,
    {
        self.params.len() == ty.params.len()
            && self.ret.len() == ty.results.len()
            && self.params.iter().zip(&ty.params).all(|(p, t)| p.ty == *t)
            && self.ret.iter().zip(&ty.results).all(|(r, t)| r == t)
    }
}

impl Signature<vihaco_abi::Type> {
    /// The parameter and result types, without parameter names.
    pub fn function_type(&self) -> vihaco_abi::FunctionType {
        vihaco_abi::FunctionType {
            params: self.params.iter().map(|param| param.ty.clone()).collect(),
            results: self.ret.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct Parameter<Ty> {
    pub name: u32, // index into the string interner
    pub ty: Ty,
}

/// A struct type declaration. `Type::Struct(i)` refers to entry `i` of the
/// struct table. At runtime a struct is a heap object tagged with its struct
/// index, with one word slot per field in declaration order.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct StructInfo<Ty> {
    pub name: u32, // index into the string interner
    pub fields: Vec<FieldInfo<Ty>>,
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct FieldInfo<Ty> {
    pub name: u32, // index into the string interner
    pub ty: Ty,
}

/// A named type declaration: a nominal type represented by a scalar word.
/// `Type::Named(i)` (written `type i`) refers to entry `i` of the named-type
/// table. At runtime a value of a named type is the same word as its `repr`;
/// the name exists only for verification, where `type i` satisfies only
/// itself.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct NamedTypeInfo<Ty> {
    pub name: u32, // index into the string interner
    /// The scalar word type representing values of this type: one of `i32`,
    /// `i64`, `u32`, `u64`, `f32`, `f64` or `bool`.
    pub repr: Ty,
}

/// Exposes the table references carried by a type, so module assembly can
/// check `Type::Struct` indices against the struct table and `Type::Named`
/// indices against the named-type table, and check named-type
/// representations.
pub trait StructTypeRef {
    /// Visit every struct-table index this type refers to, including ones
    /// nested inside compound types such as array element types.
    fn visit_struct_refs(&self, visit: &mut dyn FnMut(u32));

    /// Visit every named-type-table index this type refers to, including ones
    /// nested inside compound types. Type models without named types keep the
    /// default, which visits nothing.
    fn visit_named_refs(&self, visit: &mut dyn FnMut(u32)) {
        let _ = visit;
    }

    /// Whether this type may represent a named type (a scalar word type).
    /// Type models without named types keep the default, `false`, so their
    /// named-type tables must be empty.
    fn is_named_type_repr(&self) -> bool {
        false
    }
}

impl StructTypeRef for vihaco_abi::Type {
    fn visit_struct_refs(&self, visit: &mut dyn FnMut(u32)) {
        vihaco_abi::Type::visit_struct_refs(self, visit)
    }

    fn visit_named_refs(&self, visit: &mut dyn FnMut(u32)) {
        vihaco_abi::Type::visit_named_refs(self, visit)
    }

    fn is_named_type_repr(&self) -> bool {
        self.is_scalar_word()
    }
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct LabelInfo {
    pub address: u32,
    pub name: u32, // index into the string interner
}

#[derive(Debug, Clone, PartialEq, Default, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
#[pattern = "`{` `}`"]
pub struct NoInfo;

impl std::fmt::Display for NoInfo {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl<I, V, Ty, Info> std::fmt::Display for LocalModule<I, V, Ty, Info>
where
    I: std::fmt::Display,
    V: std::fmt::Display,
    Ty: std::fmt::Display,
    Info: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ".text".keyword().fmt(f)?;
        writeln!(f)?;

        for (addr, inst) in self.code.iter().enumerate() {
            write!(f, "0x{:04X} ", addr)?;
            inst.fmt(f)?;
            writeln!(f)?;
        }

        writeln!(f)?;
        ".const".keyword().fmt(f)?;
        writeln!(f)?;
        for (const_idx, constant) in self.constants.iter().enumerate() {
            write!(f, "0x{:04X} ", const_idx)?;
            constant.fmt(f)?;
            writeln!(f)?;
        }

        writeln!(f)?;
        ".string".keyword().fmt(f)?;
        writeln!(f)?;
        for (string_idx, string) in self.strings.iter().enumerate() {
            if string_idx > 0 {
                writeln!(f)?;
            }
            write!(f, "0x{:04X} \"{}\"", string_idx, string.escape_default())?;
        }

        writeln!(f)?;
        if !self.structs.is_empty() {
            writeln!(f)?;
            ".struct".keyword().fmt(f)?;
            writeln!(f)?;
            let name = |index: u32| {
                self.strings
                    .get(index as usize)
                    .map_or_else(|| format!("#{index}"), Clone::clone)
            };
            for (struct_idx, info) in self.structs.iter().enumerate() {
                write!(f, "0x{:04X} {} {{", struct_idx, name(info.name))?;
                for (field_idx, field) in info.fields.iter().enumerate() {
                    let sep = if field_idx == 0 { " " } else { ", " };
                    write!(f, "{sep}{}: {}", name(field.name), field.ty)?;
                }
                let close = if info.fields.is_empty() { "}" } else { " }" };
                writeln!(f, "{close}")?;
            }
        }
        if !self.types.is_empty() {
            writeln!(f)?;
            ".type".keyword().fmt(f)?;
            writeln!(f)?;
            for (type_idx, info) in self.types.iter().enumerate() {
                let name = self
                    .strings
                    .get(info.name as usize)
                    .map_or_else(|| format!("#{}", info.name), Clone::clone);
                writeln!(f, "0x{:04X} {} = {}", type_idx, name, info.repr)?;
            }
        }
        if !self.source_symbols.is_empty() {
            ".machine".keyword().fmt(f)?;
            writeln!(f, " symbols {{")?;
            for symbol in &self.source_symbols {
                writeln!(f, "  {}: {},", symbol.index, symbol.name)?;
            }
            writeln!(f, "}}")?;
            writeln!(f)?;
        }
        writeln!(f)?;
        ".feature".keyword().fmt(f)?;
        writeln!(f)?;
        self.extra.fmt(f)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum MockInst {
        Push(u64),
        Add,
        Print,
        Halt,
    }

    impl std::fmt::Display for MockInst {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                MockInst::Push(v) => write!(f, "push {}", v),
                MockInst::Add => write!(f, "add"),
                MockInst::Print => write!(f, "print"),
                MockInst::Halt => write!(f, "halt"),
            }
        }
    }

    #[test]
    fn test_module() {
        let mut module: LocalModule<MockInst, u64, &str> = LocalModule::default();
        module.code.push(MockInst::Push(42));
        module.code.push(MockInst::Push(42));
        module.code.push(MockInst::Add);
        module.code.push(MockInst::Print);
        module.code.push(MockInst::Halt);

        module.constants.push(42);
        module.constants.push(100);

        module.strings.push("hello".to_string());
        module.strings.push("world".to_string());
        println!("{:#}", module);
    }

    #[test]
    fn display_lists_struct_table_with_field_names() {
        let module: LocalModule<MockInst, u64, &str> = LocalModule {
            strings: vec!["Point".into(), "x".into(), "y".into()],
            structs: vec![
                StructInfo {
                    name: 0,
                    fields: vec![
                        FieldInfo { name: 1, ty: "f64" },
                        FieldInfo { name: 2, ty: "i64" },
                    ],
                },
                StructInfo {
                    name: 0,
                    fields: vec![],
                },
            ],
            ..LocalModule::default()
        };
        let text = module.to_string();
        assert!(text.contains("0x0000 Point { x: f64, y: i64 }\n"), "{text}");
        assert!(text.contains("0x0001 Point {}\n"), "{text}");
    }

    #[test]
    fn signatures_match_function_types_ignoring_parameter_names() {
        use vihaco_abi::{FunctionType, Type};
        let signature = Signature {
            params: vec![
                Parameter {
                    name: 3,
                    ty: Type::U64,
                },
                Parameter {
                    name: 4,
                    ty: Type::F64,
                },
            ],
            ret: vec![Type::Bool],
        };
        let ty = FunctionType::new([Type::U64, Type::F64], [Type::Bool]);
        assert_eq!(signature.function_type(), ty);
        assert!(signature.matches(&ty));
        assert!(!signature.matches(&FunctionType::new([Type::U64, Type::F64], [])));
        assert!(!signature.matches(&FunctionType::new([Type::U64], [Type::Bool])));
        assert!(!signature.matches(&FunctionType::new([Type::F64, Type::U64], [Type::Bool])));
    }

    #[test]
    fn display_lists_named_type_table() {
        let module: LocalModule<MockInst, u64, &str> = LocalModule {
            strings: vec!["location".into(), "lane".into()],
            types: vec![
                NamedTypeInfo {
                    name: 0,
                    repr: "u64",
                },
                NamedTypeInfo {
                    name: 7,
                    repr: "u64",
                },
            ],
            ..LocalModule::default()
        };
        let text = module.to_string();
        assert!(text.contains(".type\n"), "{text}");
        assert!(text.contains("0x0000 location = u64\n"), "{text}");
        assert!(text.contains("0x0001 #7 = u64\n"), "{text}");
    }
}

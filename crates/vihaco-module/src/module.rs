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
    /// Reserved local slots, including parameters, for this function.
    pub local_count: u32,
    pub start_address: u32, // corresponds to a label noop
    pub end_address: u32,   // corresponds to a label noop
    pub file: u32,
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
pub struct Signature<Ty> {
    pub params: Vec<Parameter<Ty>>,
    pub ret: Vec<Ty>,
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

/// Exposes the struct-table reference carried by a type, if any, so module
/// assembly can check `Type::Struct` indices against the struct table.
pub trait StructTypeRef {
    fn struct_index(&self) -> Option<u32>;
}

impl StructTypeRef for vihaco_abi::Type {
    fn struct_index(&self) -> Option<u32> {
        match self {
            vihaco_abi::Type::Struct(index) => Some(*index),
            _ => None,
        }
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
}

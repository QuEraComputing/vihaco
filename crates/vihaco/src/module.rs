// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::color::Themed;

#[derive(Debug, Clone, PartialEq)]
pub struct Module<I, V, Ty, Info = NoInfo> {
    pub code: Vec<I>,
    pub functions: Vec<FunctionInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub constants: Vec<V>,
    pub strings: Vec<String>,
    pub main_function: Option<u32>,
    pub file: u32,
    pub source_symbols: Vec<SourceSymbolInfo>,
    pub extra: Info,
}

impl<I, V, Ty, Info> Default for Module<I, V, Ty, Info>
where
    Info: Default,
{
    fn default() -> Self {
        Self {
            code: Vec::new(),
            functions: Vec::new(),
            labels: Vec::new(),
            constants: Vec::new(),
            strings: Vec::new(),
            main_function: None,
            file: 0,
            source_symbols: Vec::new(),
            extra: Info::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSymbolInfo {
    pub index: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionInfo<Type> {
    pub name: u32, // index into the string interner
    pub signature: Signature<Type>,
    pub local_count: u32,
    pub start_address: u32, // corresponds to a label noop
    pub end_address: u32,   // corresponds to a label noop
    pub file: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signature<Ty> {
    pub params: Vec<Parameter<Ty>>,
    pub ret: Vec<Ty>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter<Ty> {
    pub name: u32, // index into the string interner
    pub ty: Ty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelInfo {
    pub address: u32,
    pub name: u32, // index into the string interner
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoInfo;

impl std::fmt::Display for NoInfo {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl<I, V, Ty, Info> std::fmt::Display for Module<I, V, Ty, Info>
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
        let mut module: Module<MockInst, u64, &str> = Module::default();
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
}

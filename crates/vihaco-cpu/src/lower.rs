// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned CPU lowering, separate from the dialect definitions.

use crate::{
    cpu,
    operand::{StringOperand, Target},
};
use vihaco_abi::{Lower, Syntax};
use vihaco_module::loader::MetadataLoweringContext;

/// Resolve `@name` branch targets through the section's symbol table.
///
/// The loader prepares the whole table before lowering so forward references
/// work. Addresses count executable instructions; `@name:` definitions occupy
/// no slot.
pub trait LabelLookup {
    fn label_address(&self, name: &str) -> eyre::Result<u32>;
}

/// Resolve function names in the section currently being lowered.
///
/// Calls need an executable address; reference constants need a function-table
/// index. These are separate values even when a program happens to make them equal.
pub trait FunctionLookup {
    fn function_address(&self, name: &str) -> eyre::Result<u32>;
    fn function_index(&self, name: &str) -> eyre::Result<u32>;
}

/// Intern decoded string contents in the current section's string table.
///
/// Equal contents must return the same index, including contents already in the
/// table. Implementations report an error if the table cannot represent an index.
pub trait StringIntern {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32>;
}

impl LabelLookup for MetadataLoweringContext {
    fn label_address(&self, name: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::label_address(self, name)
    }
}

impl FunctionLookup for MetadataLoweringContext {
    fn function_address(&self, name: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::function_address(self, name)
    }

    fn function_index(&self, name: &str) -> eyre::Result<u32> {
        MetadataLoweringContext::function_index(self, name)
    }
}

impl StringIntern for MetadataLoweringContext {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32> {
        self.string_index(value)
    }
}

fn label_target<C: LabelLookup + ?Sized>(target: Target, context: &C) -> eyre::Result<u32> {
    match target {
        Target::Resolved(address) => Ok(address),
        Target::Symbol(name) => context.label_address(name.as_str()),
    }
}

impl<C: LabelLookup + ?Sized> Lower<C> for cpu::Branch<Syntax> {
    type Runtime = cpu::Branch;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::Branch(label_target(self.0, context)?))
    }
}

impl<C: LabelLookup + ?Sized> Lower<C> for cpu::ConditionalBranch<Syntax> {
    type Runtime = cpu::ConditionalBranch;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(cpu::ConditionalBranch(
            label_target(self.0, context)?,
            label_target(self.1, context)?,
        ))
    }
}

impl<C: FunctionLookup + ?Sized> Lower<C> for cpu::Call<Syntax> {
    type Runtime = cpu::Call;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let address = match self.1 {
            Target::Resolved(address) => address,
            Target::Symbol(name) => context.function_address(name.as_str())?,
        };
        Ok(cpu::Call(self.0, address))
    }
}

impl<C: FunctionLookup + ?Sized> Lower<C> for cpu::ConstFunctionRef<Syntax> {
    type Runtime = cpu::ConstFunctionRef;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let index = match self.0 {
            Target::Resolved(index) => index,
            Target::Symbol(name) => context.function_index(name.as_str())?,
        };
        Ok(cpu::ConstFunctionRef(crate::encode_function_ref(index)))
    }
}

impl<C: StringIntern + ?Sized> Lower<C> for cpu::ConstString<Syntax> {
    type Runtime = cpu::ConstString;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let index = match self.0 {
            StringOperand::Resolved(index) => index,
            StringOperand::Literal(value) => context.intern_string(value.as_str())?,
        };
        Ok(cpu::ConstString(crate::encode_string_id(index)))
    }
}

// Literal encoding is instruction-owned lowering and needs no loader capabilities.
macro_rules! lower_literal {
    ($($instruction:ident => $convert:expr),* $(,)?) => {$(
        impl<C: ?Sized> vihaco_abi::Lower<C> for cpu::$instruction<vihaco_abi::Syntax> {
            type Runtime = cpu::$instruction;
            fn lower(self, _: &mut C) -> eyre::Result<Self::Runtime> {
                Ok(cpu::$instruction(($convert)(self.0)))
            }
        }
    )*};
}

lower_literal! {
    ConstI32 => crate::Word::from,
    ConstI64 => crate::Word::from,
    ConstF32 => crate::Word::from,
    ConstF64 => crate::Word::from,
    ConstU32 => crate::word::encode_u32,
    ConstBool => crate::word::encode_bool,
    ConstHeapRef => crate::word::encode_heap_ref,
}

#[cfg(test)]
mod metadata_context_tests {
    use super::*;
    use vihaco_module::module::{FunctionDecl, NoInfo, ProgramMetadata, Signature, Symbol};
    use vihaco_parser::{Ident, QuotedString};

    #[test]
    fn metadata_context_supplies_cpu_capabilities_and_interns_strings() -> eyre::Result<()> {
        let metadata: ProgramMetadata = ProgramMetadata {
            strings: vec!["helper".into(), "main".into(), "hello".into()],
            functions: vec![
                FunctionDecl {
                    name: 0,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 0,
                    file: 0,
                },
                FunctionDecl {
                    name: 1,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 0,
                    file: 0,
                },
            ],
            constants: vec![],
            main_function: Some(1),
            source_symbols: vec![],
            extra: NoInfo,
        };
        let symbols = [("main", 0), ("done", 4), ("helper", 5)].map(|(name, address)| Symbol {
            name: name.into(),
            address,
        });
        let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
        let branch =
            cpu::Branch::<Syntax>(Target::Symbol(Ident("done".into()))).lower(&mut context)?;
        assert_eq!(branch, cpu::Branch(4));
        let call =
            cpu::Call::<Syntax>(0, Target::Symbol(Ident("helper".into()))).lower(&mut context)?;
        assert_eq!(call, cpu::Call(0, 5));
        let function = cpu::ConstFunctionRef::<Syntax>(Target::Symbol(Ident("helper".into())))
            .lower(&mut context)?;
        assert_eq!(
            function,
            cpu::ConstFunctionRef(crate::encode_function_ref(0))
        );
        let string =
            cpu::ConstString::<Syntax>(StringOperand::Literal(QuotedString("hello".into())))
                .lower(&mut context)?;
        assert_eq!(string, cpu::ConstString(crate::encode_string_id(2)));
        let new = cpu::ConstString::<Syntax>(StringOperand::Literal(QuotedString("new".into())))
            .lower(&mut context)?;
        assert_eq!(new, cpu::ConstString(crate::encode_string_id(3)));
        assert_eq!(metadata.strings, ["helper", "main", "hello"]);
        assert_eq!(context.into_strings(), ["helper", "main", "hello", "new"]);
        Ok(())
    }
}

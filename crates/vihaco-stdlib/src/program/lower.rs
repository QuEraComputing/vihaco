// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned program lowering, separate from the dialect definitions.

use super::{
    operand::{StringOperand, Target},
    program,
};

use vihaco_abi::{Lower, Syntax};
use vihaco_module::loader::MetadataLoweringContext;

/// Resolve labels in the function currently being lowered.
///
/// The loader prepares label addresses before lowering so forward references
/// work. Addresses count executable instructions, excluding label declarations.
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

impl<C: LabelLookup + ?Sized> Lower<C> for program::Branch<Syntax> {
    type Runtime = program::Branch;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(program::Branch(label_target(self.0, context)?))
    }
}

impl<C: LabelLookup + ?Sized> Lower<C> for program::ConditionalBranch<Syntax> {
    type Runtime = program::ConditionalBranch;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(program::ConditionalBranch(
            label_target(self.0, context)?,
            label_target(self.1, context)?,
        ))
    }
}

impl<C: FunctionLookup + ?Sized> Lower<C> for program::Call<Syntax> {
    type Runtime = program::Call;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let address = match self.1 {
            Target::Resolved(address) => address,
            Target::Symbol(name) => context.function_address(name.as_str())?,
        };
        Ok(program::Call(self.0, address))
    }
}

impl<C: FunctionLookup + ?Sized> Lower<C> for program::ConstFunctionRef<Syntax> {
    type Runtime = program::ConstFunctionRef;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let index = match self.0 {
            Target::Resolved(index) => index,
            Target::Symbol(name) => context.function_index(name.as_str())?,
        };
        Ok(program::ConstFunctionRef(
            vihaco_abi::word::encode_function_ref(index),
        ))
    }
}

impl<C: StringIntern + ?Sized> Lower<C> for program::ConstString<Syntax> {
    type Runtime = program::ConstString;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        let index = match self.0 {
            StringOperand::Resolved(index) => index,
            StringOperand::Literal(value) => context.intern_string(value.as_str())?,
        };
        Ok(program::ConstString(vihaco_abi::word::encode_string_id(
            index,
        )))
    }
}

// Literal encoding is instruction-owned lowering and needs no loader capabilities.
macro_rules! lower_literal {
    ($($instruction:ident => $convert:expr),* $(,)?) => {$(
        impl<C: ?Sized> vihaco_abi::Lower<C> for program::$instruction<vihaco_abi::Syntax> {
            type Runtime = program::$instruction;
            fn lower(self, _: &mut C) -> eyre::Result<Self::Runtime> {
                Ok(program::$instruction(($convert)(self.0)))
            }
        }
    )*};
}

lower_literal! {
    ConstI32 => vihaco_abi::word::Word::from,
    ConstI64 => vihaco_abi::word::Word::from,
    ConstF32 => vihaco_abi::word::Word::from,
    ConstF64 => vihaco_abi::word::Word::from,
    ConstU32 => vihaco_abi::word::encode_u32,
    ConstBool => vihaco_abi::word::encode_bool,
    ConstHeapRef => vihaco_abi::word::encode_heap_ref,
}

// String interning belongs to module construction, before immutable loading.
impl<I, V, Ty, Info> StringIntern for vihaco_module::module::LocalModule<I, V, Ty, Info> {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32> {
        if let Some(index) = self.strings.iter().position(|existing| existing == value) {
            return Ok(u32::try_from(index)?);
        }
        let index = u32::try_from(self.strings.len())?;
        self.strings.push(value.to_owned());
        Ok(index)
    }
}

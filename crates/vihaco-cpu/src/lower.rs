// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned CPU lowering, separate from the dialect definitions.

use crate::{
    cpu,
    operand::{StringOperand, Target},
};
use vihaco::{Lower, Syntax};

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

impl<I, C, V, Ty, Info> StringIntern for vihaco::ProgramImage<I, C, V, Ty, Info> {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32> {
        if let Some(index) = self
            .module
            .strings
            .iter()
            .position(|existing| existing == value)
        {
            return Ok(u32::try_from(index)?);
        }
        let index = u32::try_from(self.module.strings.len())?;
        self.module.strings.push(value.to_owned());
        Ok(index)
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
        impl<C: ?Sized> vihaco::Lower<C> for cpu::$instruction<vihaco::Syntax> {
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

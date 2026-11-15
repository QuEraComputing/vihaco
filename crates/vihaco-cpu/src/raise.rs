// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned CPU raising: the inverse of [`lower`](crate::lower).
//!
//! Each leaf whose source operands differ from its runtime operands raises
//! with the capabilities it needs from the context, as
//! [`vihaco_module::raising`] traits: branch targets name the symbol at their
//! address, calls and function references name the function, string
//! constants spell their text, and literal words are decoded back to the
//! literal they were lowered from. A word that no literal lowers to is an
//! error rather than a different value.

use crate::{
    cpu,
    literal::{F32Literal, F64Literal, I32Literal, I64Literal},
    operand::{StringOperand, Target},
};
use vihaco_abi::{Raise, Syntax, word};
use vihaco_module::raising::{FunctionNames, LabelNames, StringTable, decode_canonical};
use vihaco_parser::{Ident, QuotedString};

fn symbol(name: &str) -> Target {
    Target::Symbol(Ident(name.to_owned()))
}

fn label<C: LabelNames + ?Sized>(address: u32, context: &C) -> eyre::Result<Target> {
    context.label_name(address).map(symbol)
}

impl<C: LabelNames + ?Sized> Raise<C> for cpu::Branch {
    type Syntax = cpu::Branch<Syntax>;
    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax> {
        Ok(cpu::Branch(label(self.0, context)?))
    }
}

impl<C: LabelNames + ?Sized> Raise<C> for cpu::ConditionalBranch {
    type Syntax = cpu::ConditionalBranch<Syntax>;
    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax> {
        // Look both targets up before failing, so that every address without
        // a symbol is reported to the context.
        let (then, otherwise) = (label(self.0, context), label(self.1, context));
        Ok(cpu::ConditionalBranch(then?, otherwise?))
    }
}

impl<C: FunctionNames + ?Sized> Raise<C> for cpu::Call {
    type Syntax = cpu::Call<Syntax>;
    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax> {
        Ok(cpu::Call(self.0, symbol(context.function_at(self.1)?)))
    }
}

impl<C: FunctionNames + ?Sized> Raise<C> for cpu::ConstFunctionRef {
    type Syntax = cpu::ConstFunctionRef<Syntax>;
    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax> {
        let index = decode_canonical(
            self.0,
            word::decode_function_ref,
            word::encode_function_ref,
            "function reference",
        )?;
        Ok(cpu::ConstFunctionRef(symbol(context.function_name(index)?)))
    }
}

impl<C: StringTable + ?Sized> Raise<C> for cpu::ConstString {
    type Syntax = cpu::ConstString<Syntax>;
    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax> {
        let index = decode_canonical(
            self.0,
            word::decode_string_id,
            word::encode_string_id,
            "string",
        )?;
        let text = context.string(index)?.to_owned();
        Ok(cpu::ConstString(StringOperand::Literal(QuotedString(text))))
    }
}

// Literal decoding is instruction-owned raising and needs no context.
macro_rules! raise_literal {
    ($($instruction:ident => $decode:path, $encode:path, $what:literal, $wrap:expr;)*) => {$(
        impl<C: ?Sized> Raise<C> for cpu::$instruction {
            type Syntax = cpu::$instruction<Syntax>;
            fn raise(&self, _: &C) -> eyre::Result<Self::Syntax> {
                let value = decode_canonical(self.0, $decode, $encode, $what)?;
                Ok(cpu::$instruction(($wrap)(value)))
            }
        }
    )*};
}

raise_literal! {
    ConstI32 => word::decode_i32, word::encode_i32, "i32", I32Literal;
    ConstI64 => word::decode_i64, word::encode_i64, "i64", I64Literal;
    ConstF32 => word::decode_f32, word::encode_f32, "f32", F32Literal;
    ConstF64 => word::decode_f64, word::encode_f64, "f64", F64Literal;
    ConstU32 => word::decode_u32, word::encode_u32, "u32", std::convert::identity;
    ConstBool => word::decode_bool, word::encode_bool, "bool", std::convert::identity;
    ConstHeapRef => word::decode_heap_ref, word::encode_heap_ref, "heap reference",
        std::convert::identity;
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_abi::Lower;
    use vihaco_module::loader::MetadataLoweringContext;
    use vihaco_module::module::{
        FunctionDecl, FunctionInfo, LocalModule, NoInfo, ProgramMetadata, Signature, Symbol,
    };
    use vihaco_module::raising::RaiseContext;
    use vihaco_parser::Print;

    fn metadata() -> ProgramMetadata {
        let function = |name| FunctionDecl {
            name,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            file: 0,
        };
        ProgramMetadata {
            strings: vec!["helper".into(), "main".into(), "hello".into()],
            functions: vec![function(0), function(1)],
            constants: vec![],
            main_function: Some(1),
            source_symbols: vec![],
            extra: NoInfo,
        }
    }

    fn symbols() -> Vec<Symbol> {
        [("main", 0), ("done", 4), ("helper", 5)]
            .map(|(name, address)| Symbol {
                name: name.into(),
                address,
            })
            .to_vec()
    }

    fn module() -> LocalModule<(), vihaco_abi::Value, vihaco_abi::Type> {
        let function = |name, start_address, end_address| FunctionInfo {
            name,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            start_address,
            end_address,
            file: 0,
        };
        LocalModule {
            code: vec![(); 6],
            functions: vec![function(0, 5, 6), function(1, 0, 5)],
            symbols: symbols(),
            constants: vec![],
            strings: metadata().strings,
            main_function: Some(1),
            source_symbols: vec![],
            extra: NoInfo,
        }
    }

    /// Raise `instruction`, check its source spelling, and lower it back.
    fn round_trip<I>(instruction: I, spelled: &str) -> eyre::Result<()>
    where
        I: Raise<RaiseContext> + PartialEq + std::fmt::Debug,
        I::Syntax: Lower<MetadataLoweringContext, Runtime = I> + Print,
    {
        let raised = instruction.raise(&RaiseContext::new(&module())?)?;
        assert_eq!(raised.to_source().unwrap(), spelled);
        let metadata = metadata();
        let mut context = MetadataLoweringContext::new(&metadata, &symbols())?;
        assert_eq!(raised.lower(&mut context)?, instruction);
        Ok(())
    }

    #[test]
    fn raising_names_what_lowering_resolved() -> eyre::Result<()> {
        round_trip(cpu::Branch(4), "br @done")?;
        round_trip(cpu::ConditionalBranch(0, 4), "cond_br @main, @done")?;
        round_trip(cpu::Call(2, 5), "call 2, @helper")?;
        round_trip(
            cpu::ConstFunctionRef(word::encode_function_ref(1)),
            "const_fn_ref @main",
        )?;
        round_trip(
            cpu::ConstString(word::encode_string_id(2)),
            "const_string \"hello\"",
        )?;
        round_trip(cpu::ConstI32(word::encode_i32(-7)), "const_i32 -7")?;
        round_trip(cpu::ConstI64(word::encode_i64(-7)), "const_i64 -7")?;
        round_trip(cpu::ConstF32(word::encode_f32(0.5)), "const_f32 0.5")?;
        round_trip(cpu::ConstF64(word::encode_f64(-2.5)), "const_f64 -2.5")?;
        round_trip(cpu::ConstU32(word::encode_u32(9)), "const_u32 9")?;
        round_trip(cpu::ConstBool(word::encode_bool(true)), "const_bool true")?;
        round_trip(
            cpu::ConstHeapRef(word::encode_heap_ref(3)),
            "const_heap_ref 3",
        )
    }

    #[test]
    fn raising_reports_what_has_no_name_or_spelling() -> eyre::Result<()> {
        let module = module();
        let context = RaiseContext::new(&module)?;
        // Both missing branch targets are recorded for label synthesis.
        assert!(cpu::ConditionalBranch(1, 2).raise(&context).is_err());
        assert_eq!(context.unlabeled(), [1, 2]);
        let error = cpu::Call(0, 4).raise(&context).unwrap_err();
        assert_eq!(error.to_string(), "no function starts at address 0x0004");
        assert!(
            cpu::ConstString(word::encode_string_id(3))
                .raise(&context)
                .is_err()
        );
        assert!(
            cpu::ConstFunctionRef(word::encode_function_ref(2))
                .raise(&context)
                .is_err()
        );
        let error = cpu::ConstBool(5).raise(&context).unwrap_err();
        assert_eq!(
            error.to_string(),
            "word 0x5 is not the encoding of any bool literal"
        );
        assert!(cpu::ConstI32(u64::MAX).raise(&context).is_err());
        assert!(cpu::ConstString(u64::MAX).raise(&context).is_err());
        Ok(())
    }
}

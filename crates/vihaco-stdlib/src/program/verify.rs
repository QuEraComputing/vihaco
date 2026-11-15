// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load-time typing rules ([`Verify`]) for every program instruction.
//!
//! These match the rules for the same instructions in the `typed-declarations`
//! CPU dialect, through the shared helpers in `vihaco_verifier`. Each rule
//! bounds its context only on the capabilities it uses.

use super::program;
use vihaco_abi::word::{decode_function_ref, decode_string_id};
use vihaco_abi::{Flow, Type, Value, Verify};
use vihaco_verifier::{
    Constants, Locals, OperandStack, Signatures, TypedSlot, direct_call_effect, function_constant,
    indirect_call_effect, load_local, return_effect, stack_effect, store_local, string_constant,
};

impl<C: ?Sized> Verify<C> for program::Branch {
    fn flow(&self) -> Flow {
        Flow::jump(self.0)
    }

    fn verify(&self, _: &mut C) -> eyre::Result<()> {
        Ok(())
    }
}

/// Pops the `bool` condition.
impl<C> Verify<C> for program::ConditionalBranch
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn flow(&self) -> Flow {
        Flow::jump_to([self.0, self.1])
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [Type::Bool], [])
    }
}

impl<C: ?Sized> Verify<C> for program::Halt {
    fn flow(&self) -> Flow {
        Flow::Halt
    }

    fn verify(&self, _: &mut C) -> eyre::Result<()> {
        Ok(())
    }
}

impl<C> Verify<C> for program::Return
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn flow(&self) -> Flow {
        Flow::Return
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        return_effect(cx, self.0 as usize)
    }
}

impl<C> Verify<C> for program::Call
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        direct_call_effect(cx, self.0, self.1)
    }
}

impl<C> Verify<C> for program::IndirectCall
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        indirect_call_effect(cx, &self.0)
    }
}

/// Pushes a copy of the top slot.
impl<C> Verify<C> for program::Dup
where
    C: OperandStack + ?Sized,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let top = cx.peek()?.clone();
        cx.push(top);
        Ok(())
    }
}

/// Typed loads and stores: the local must be declared with exactly the
/// mnemonic's type. `None` for the untyped `load` / `store`.
macro_rules! locals {
    ($($load:ident, $store:ident => $ty:expr;)*) => {$(
        impl<C> Verify<C> for program::$load
        where
            C: Locals + ?Sized,
            C::Slot: TypedSlot,
        {
            fn verify(&self, cx: &mut C) -> eyre::Result<()> {
                load_local(cx, self.0, $ty.as_ref())
            }
        }

        impl<C> Verify<C> for program::$store
        where
            C: Locals + ?Sized,
            C::Slot: TypedSlot,
        {
            fn verify(&self, cx: &mut C) -> eyre::Result<()> {
                store_local(cx, self.0, $ty.as_ref())
            }
        }
    )*};
}

locals! {
    LoadI32, StoreI32 => Some(Type::I32);
    LoadI64, StoreI64 => Some(Type::I64);
    LoadU32, StoreU32 => Some(Type::U32);
    LoadU64, StoreU64 => Some(Type::U64);
    LoadF32, StoreF32 => Some(Type::F32);
    LoadF64, StoreF64 => Some(Type::F64);
    LoadBool, StoreBool => Some(Type::Bool);
    Load, Store => None::<Type>;
}

/// Literal constants push their mnemonic's type. (There is no reference
/// literal: typed programs cannot write a heap reference.)
macro_rules! literals {
    ($($instruction:ident => $ty:expr;)*) => {$(
        impl<C> Verify<C> for program::$instruction
        where
            C: OperandStack + ?Sized,
            C::Slot: TypedSlot,
        {
            fn verify(&self, cx: &mut C) -> eyre::Result<()> {
                stack_effect(cx, [], [$ty])
            }
        }
    )*};
}

literals! {
    ConstI32 => Type::I32;
    ConstI64 => Type::I64;
    ConstU32 => Type::U32;
    ConstU64 => Type::U64;
    ConstF32 => Type::F32;
    ConstF64 => Type::F64;
    ConstBool => Type::Bool;
}

/// Pushes `str`; the index must be in the string table.
impl<C> Verify<C> for program::ConstString
where
    C: OperandStack + Constants<Value, Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        string_constant(cx, decode_string_id(self.0))
    }
}

/// Pushes `fn`; the index must be in the function table.
impl<C> Verify<C> for program::ConstFunctionRef
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        function_constant(cx, decode_function_ref(self.0))
    }
}

#[cfg(test)]
mod tests {
    //! Rules checked one instruction at a time against a [`ModuleVerifier`]
    //! for a module with functions `main() -> u64` (locals `[u64, array f64]`)
    //! and `helper(f64) -> bool`.

    use super::*;
    use vihaco_abi::FunctionType;
    use vihaco_abi::word::{encode_function_ref, encode_string_id};
    use vihaco_module::module::{FunctionInfo, LocalModule, Parameter, Signature};
    use vihaco_verifier::{
        AbstractState, DiagnosticKind, Frame, ModuleVerifier, TypeSlot, VerifyError,
    };

    use Type::{Bool, F64, I64, U64};

    fn module() -> LocalModule<(), Value, Type> {
        let function = |name, params: Vec<Type>, ret, locals, start| FunctionInfo {
            name,
            signature: Signature {
                params: params
                    .into_iter()
                    .map(|ty| Parameter { name: 0, ty })
                    .collect(),
                ret,
            },
            locals,
            start_address: start,
            end_address: start + 1,
            file: 0,
        };
        LocalModule {
            code: vec![(), ()],
            functions: vec![
                function(0, vec![], vec![U64], vec![U64, Type::array(F64)], 0),
                function(1, vec![F64], vec![Bool], vec![F64], 1),
            ],
            strings: vec!["main".into(), "helper".into()],
            ..LocalModule::default()
        }
    }

    fn check<I>(instruction: &I, stack: &[Type]) -> Result<Vec<String>, DiagnosticKind>
    where
        I: for<'m> Verify<ModuleVerifier<'m>>,
    {
        let module = module();
        let mut cx = ModuleVerifier::new(&module);
        let locals = cx.entry_frame(0).unwrap().locals().to_vec();
        let operands = stack.iter().cloned().map(TypeSlot::Known).collect();
        cx.restore(&Frame::with_operands(locals, operands));
        cx.set_address(0, 0);
        match instruction.verify(&mut cx) {
            Ok(()) => Ok(cx
                .snapshot()
                .operands()
                .iter()
                .map(ToString::to_string)
                .collect()),
            Err(report) => Err(match report.downcast::<VerifyError>() {
                Ok(error) => error.kind().clone(),
                Err(report) => DiagnosticKind::Rule(report.to_string()),
            }),
        }
    }

    fn flow<I: for<'m> Verify<ModuleVerifier<'m>>>(instruction: &I) -> Flow {
        <I as Verify<ModuleVerifier<'_>>>::flow(instruction)
    }

    fn ok(types: &[&str]) -> Result<Vec<String>, DiagnosticKind> {
        Ok(types.iter().map(|ty| ty.to_string()).collect())
    }

    fn mismatch(expected: &str, found: &str) -> Result<Vec<String>, DiagnosticKind> {
        Err(DiagnosticKind::TypeMismatch {
            expected: expected.into(),
            found: found.into(),
        })
    }

    #[test]
    fn control_flow_is_stateless_and_typed() {
        assert_eq!(flow(&program::Branch(4)), Flow::jump(4));
        assert_eq!(check(&program::Branch(4), &[]), ok(&[]));
        assert_eq!(
            flow(&program::ConditionalBranch(1, 2)),
            Flow::jump_to([1, 2])
        );
        assert_eq!(check(&program::ConditionalBranch(1, 2), &[Bool]), ok(&[]));
        assert_eq!(
            check(&program::ConditionalBranch(1, 2), &[U64]),
            mismatch("bool", "u64")
        );
        assert_eq!(flow(&program::Halt), Flow::Halt);
        assert_eq!(check(&program::Halt, &[]), ok(&[]));
        assert_eq!(flow(&program::Return(1)), Flow::Return);
        assert_eq!(check(&program::Return(1), &[U64]), ok(&[]));
        assert_eq!(
            check(&program::Return(2), &[U64, U64]),
            Err(DiagnosticKind::ReturnCountMismatch {
                declared: 1,
                found: 2
            })
        );
        assert_eq!(flow(&program::Call(1, 1)), Flow::Next);
    }

    #[test]
    fn calls_are_checked_against_signatures() {
        assert_eq!(check(&program::Call(1, 1), &[F64]), ok(&["bool"]));
        assert_eq!(
            check(&program::Call(0, 1), &[]),
            Err(DiagnosticKind::CallArityMismatch {
                callee: 1,
                declared: 1,
                found: 0
            })
        );
        let call = program::IndirectCall(FunctionType::new([F64], [Bool]));
        assert_eq!(check(&call, &[F64, Type::FunctionRef]), ok(&["bool"]));
        assert_eq!(
            check(&call, &[U64, Type::FunctionRef]),
            mismatch("f64", "u64")
        );
        assert_eq!(check(&call, &[F64, U64]), mismatch("fn", "u64"));
    }

    #[test]
    fn locals_and_constants_follow_declared_types() {
        assert_eq!(check(&program::LoadU64(0), &[]), ok(&["u64"]));
        assert_eq!(check(&program::LoadF64(0), &[]), mismatch("f64", "u64"));
        assert_eq!(check(&program::StoreI64(0), &[I64]), mismatch("i64", "u64"));
        assert_eq!(check(&program::Load(1), &[]), ok(&["array f64"]));
        assert_eq!(
            check(&program::Store(1), &[Type::array(U64)]),
            mismatch("array f64", "array u64")
        );
        assert_eq!(
            check(&program::Store(2), &[U64]),
            Err(DiagnosticKind::LocalIndexOutOfRange { index: 2, count: 2 })
        );
        assert_eq!(check(&program::Dup, &[F64]), ok(&["f64", "f64"]));
        assert_eq!(check(&program::ConstF64(0), &[]), ok(&["f64"]));
        assert_eq!(
            check(&program::ConstString(encode_string_id(1)), &[]),
            ok(&["str"])
        );
        assert!(check(&program::ConstString(encode_string_id(2)), &[]).is_err());
        assert_eq!(
            check(&program::ConstFunctionRef(encode_function_ref(0)), &[]),
            ok(&["fn"])
        );
        assert!(check(&program::ConstFunctionRef(encode_function_ref(2)), &[]).is_err());
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load-time typing rules ([`Verify`]) for every CPU instruction.
//!
//! Both typed builds name their dialect `cpu`, so one set of rules serves
//! both: computation, heap, and named-type instructions are common, and the
//! control-flow, local, and constant instructions that only the
//! `typed-declarations` dialect has are gated on its build. (With
//! `program-image-execution` those belong to `vihaco_stdlib::program`, whose
//! rules use the same helpers.) Each rule bounds its context only on the
//! capabilities it uses.
//!
//! Operand types follow what the CPU does at runtime: shift and rotate amounts
//! are `u32`, comparisons push `bool`, the logic instructions take `bool`, and
//! array indices and capacities are `u64`.

use crate::cpu;
use vihaco_abi::{Type, Verify};
use vihaco_verifier::{NamedTypes, OperandStack, StructTypes, TypedSlot, pop_array, stack_effect};

/// Fixed-signature rules: `Instruction: [inputs] -> [outputs]`, inputs bottom
/// first.
macro_rules! fixed {
    ($($instruction:ident: [$($input:expr),*] -> [$($output:expr),*];)*) => {$(
        impl<C> Verify<C> for cpu::$instruction
        where
            C: OperandStack + ?Sized,
            C::Slot: TypedSlot,
        {
            fn verify(&self, cx: &mut C) -> eyre::Result<()> {
                stack_effect(cx, [$($input),*], [$($output),*])
            }
        }
    )*};
}

/// `[T, T] -> [T]` for each listed instruction.
macro_rules! binary {
    ($ty:ident: $($instruction:ident),* $(,)?) => {
        fixed! { $($instruction: [Type::$ty, Type::$ty] -> [Type::$ty];)* }
    };
}

/// `[T, T] -> [bool]` for each listed instruction.
macro_rules! compare {
    ($ty:ident: $($instruction:ident),* $(,)?) => {
        fixed! { $($instruction: [Type::$ty, Type::$ty] -> [Type::Bool];)* }
    };
}

/// `[T, u32] -> [T]`: shifts and rotates take a `u32` amount.
macro_rules! shift {
    ($ty:ident: $($instruction:ident),* $(,)?) => {
        fixed! { $($instruction: [Type::$ty, Type::U32] -> [Type::$ty];)* }
    };
}

binary!(I32: AddI32, SubI32, MulI32, DivI32, RemI32, BitAndI32, BitOrI32, BitXorI32);
binary!(I64: AddI64, SubI64, MulI64, DivI64, RemI64, BitAndI64, BitOrI64, BitXorI64);
binary!(U32: AddU32, SubU32, MulU32, DivU32, RemU32, BitAndU32, BitOrU32, BitXorU32);
binary!(U64: AddU64, SubU64, MulU64, DivU64, RemU64, BitAndU64, BitOrU64, BitXorU64);
binary!(F32: AddF32, SubF32, MulF32, DivF32, RemF32);
binary!(F64: AddF64, SubF64, MulF64, DivF64, RemF64);

fixed! {
    NegI32: [Type::I32] -> [Type::I32];
    NegI64: [Type::I64] -> [Type::I64];
    NegF32: [Type::F32] -> [Type::F32];
    NegF64: [Type::F64] -> [Type::F64];
    Not: [Type::Bool] -> [Type::Bool];
    And: [Type::Bool, Type::Bool] -> [Type::Bool];
    Or: [Type::Bool, Type::Bool] -> [Type::Bool];
    Xor: [Type::Bool, Type::Bool] -> [Type::Bool];
    // Any heap object: `struct N` and `array T` satisfy `heap`.
    HeapDealloc: [Type::HeapRef] -> [];
}

shift!(I32: ShlI32, ShrI32, RolI32, RorI32);
shift!(I64: ShlI64, ShrI64, RolI64, RorI64);
shift!(U32: ShlU32, ShrU32, RolU32, RorU32);
shift!(U64: ShlU64, ShrU64, RolU64, RorU64);

compare!(I32: EqI32, NeI32, LtI32, GtI32, LeI32, GeI32);
compare!(I64: EqI64, NeI64, LtI64, GtI64, LeI64, GeI64);
compare!(U32: EqU32, NeU32, LtU32, GtU32, LeU32, GeU32);
compare!(U64: EqU64, NeU64, LtU64, GtU64, LeU64, GeU64);
compare!(F32: EqF32, NeF32, LtF32, GtF32, LeF32, GeF32);
compare!(F64: EqF64, NeF64, LtF64, GtF64, LeF64, GeF64);

// ---------------------------------------------------------------- arrays

/// `[T × N] -> [array T]`.
impl<C> Verify<C> for cpu::ArrayNew
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let cpu::ArrayNew(elem, count) = self;
        // Bounded: one underflow, however large the count.
        for value in cx.pop_n(*count as usize)? {
            value.expect(elem)?;
        }
        cx.push(C::Slot::of_type(Type::array(elem.clone())));
        Ok(())
    }
}

/// `[u64] -> [array T]` (the capacity).
impl<C> Verify<C> for cpu::ArrayReserve
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [Type::U64], [Type::array(self.0.clone())])
    }
}

/// `[array T, T] -> [array T]`.
impl<C> Verify<C> for cpu::ArrayPush
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let array = Type::array(self.0.clone());
        stack_effect(cx, [array.clone(), self.0.clone()], [array])
    }
}

/// `[array T, u64] -> [T]`.
impl<C> Verify<C> for cpu::ArrayGet
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let array = Type::array(self.0.clone());
        stack_effect(cx, [array, Type::U64], [self.0.clone()])
    }
}

/// `[array T, u64, T] -> []`.
impl<C> Verify<C> for cpu::ArraySet
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let array = Type::array(self.0.clone());
        stack_effect(cx, [array, Type::U64, self.0.clone()], [])
    }
}

/// `[array _] -> [u64]`: any element type.
impl<C> Verify<C> for cpu::ArrayLen
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        pop_array(cx)?;
        cx.push(C::Slot::of_type(Type::U64));
        Ok(())
    }
}

// ---------------------------------------------------------------- structs

/// `[field 0, …, field n-1] -> [struct N]`, fields typed from the struct table.
impl<C> Verify<C> for cpu::StructNew
where
    C: OperandStack + StructTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let struct_id = self.0.struct_id;
        let count = cx.field_count(struct_id)?;
        // The CPU pops the layout's count; lowering takes it from the table,
        // but a module built directly could disagree.
        if self.0.field_count as usize != count {
            return Err(vihaco_verifier::VerifyError::rule(format!(
                "struct_new pops {} fields, but struct {struct_id} declares {count}",
                self.0.field_count
            ))
            .into());
        }
        let fields = (0..count as u32)
            .map(|slot| cx.field_type(struct_id, slot).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        for (value, ty) in cx.pop_n(count)?.iter().zip(&fields) {
            value.expect(ty)?;
        }
        cx.push(C::Slot::of_type(Type::Struct(struct_id)));
        Ok(())
    }
}

/// `[struct N] -> [field type]`.
impl<C> Verify<C> for cpu::GetField
where
    C: OperandStack + StructTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let crate::FieldSlot { struct_id, slot } = self.0;
        let field = cx.field_type(struct_id, slot)?.clone();
        stack_effect(cx, [Type::Struct(struct_id)], [field])
    }
}

/// `[struct N, field type] -> []`.
impl<C> Verify<C> for cpu::SetField
where
    C: OperandStack + StructTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let crate::FieldSlot { struct_id, slot } = self.0;
        let field = cx.field_type(struct_id, slot)?.clone();
        stack_effect(cx, [Type::Struct(struct_id), field], [])
    }
}

// ---------------------------------------------------------------- named types

/// `[repr] -> [type N]`.
impl<C> Verify<C> for cpu::Wrap
where
    C: OperandStack + NamedTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let repr = cx.named_type_repr(self.0)?.clone();
        stack_effect(cx, [repr], [Type::Named(self.0)])
    }
}

/// `[type N] -> [repr]`.
impl<C> Verify<C> for cpu::Unwrap
where
    C: OperandStack + NamedTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let repr = cx.named_type_repr(self.0)?.clone();
        stack_effect(cx, [Type::Named(self.0)], [repr])
    }
}

#[cfg(test)]
mod tests {
    //! Rules checked one instruction at a time against a [`ModuleVerifier`]
    //! for a small module: `Point { x: i64, y: f64 }`, named types `location`
    //! and `lane` (both `u64`), and functions `main() -> u64` with locals
    //! `[u64, struct 0]` and `helper(f64) -> (bool)`.

    use super::*;
    use crate::{FieldSlot, StructLayout};
    use vihaco_abi::{Flow, Value};
    use vihaco_module::module::{
        FieldInfo, FunctionInfo, LocalModule, NamedTypeInfo, Parameter, Signature, StructInfo,
    };
    use vihaco_verifier::{
        AbstractState, DiagnosticKind, Frame, ModuleVerifier, TypeSlot, VerifyError,
    };

    pub(super) fn module() -> LocalModule<(), Value, Type> {
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
                function(
                    0,
                    vec![],
                    vec![Type::U64],
                    vec![Type::U64, Type::Struct(0)],
                    0,
                ),
                function(1, vec![Type::F64], vec![Type::Bool], vec![Type::F64], 1),
            ],
            structs: vec![StructInfo {
                name: 2,
                fields: vec![
                    FieldInfo {
                        name: 3,
                        ty: Type::I64,
                    },
                    FieldInfo {
                        name: 4,
                        ty: Type::F64,
                    },
                ],
            }],
            types: vec![
                NamedTypeInfo {
                    name: 5,
                    repr: Type::U64,
                },
                NamedTypeInfo {
                    name: 6,
                    repr: Type::U64,
                },
            ],
            strings: ["main", "helper", "Point", "x", "y", "location", "lane"]
                .map(String::from)
                .to_vec(),
            ..LocalModule::default()
        }
    }

    /// Verify `instruction` in `main` with `stack` as its operands (bottom
    /// first), returning the resulting operands or the diagnostic.
    pub(super) fn check<I>(instruction: &I, stack: &[Type]) -> Result<Vec<String>, DiagnosticKind>
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

    pub(super) fn ok(types: &[&str]) -> Result<Vec<String>, DiagnosticKind> {
        Ok(types.iter().map(|ty| ty.to_string()).collect())
    }

    pub(super) fn mismatch(expected: &str, found: &str) -> Result<Vec<String>, DiagnosticKind> {
        Err(DiagnosticKind::TypeMismatch {
            expected: expected.into(),
            found: found.into(),
        })
    }

    pub(super) fn flow<I: for<'m> Verify<ModuleVerifier<'m>>>(instruction: &I) -> Flow {
        <I as Verify<ModuleVerifier<'_>>>::flow(instruction)
    }

    use Type::{Bool, F32, F64, HeapRef, I32, I64, U32, U64};

    #[test]
    fn fixed_signature_families_follow_the_runtime() {
        // Arithmetic and bitwise: [T, T] -> [T].
        assert_eq!(check(&cpu::AddI64, &[I64, I64]), ok(&["i64"]));
        assert_eq!(check(&cpu::RemF32, &[F32, F32]), ok(&["f32"]));
        assert_eq!(check(&cpu::BitXorU32, &[U32, U32]), ok(&["u32"]));
        assert_eq!(check(&cpu::DivU64, &[U64, I64]), mismatch("u64", "i64"));
        assert_eq!(check(&cpu::NegF64, &[F64]), ok(&["f64"]));
        // Shifts and rotates take a u32 amount.
        assert_eq!(check(&cpu::ShlU64, &[U64, U32]), ok(&["u64"]));
        assert_eq!(check(&cpu::RorI32, &[I32, U32]), ok(&["i32"]));
        assert_eq!(check(&cpu::ShrI64, &[I64, I64]), mismatch("u32", "i64"));
        // Comparisons push bool.
        assert_eq!(check(&cpu::LtF64, &[F64, F64]), ok(&["bool"]));
        assert_eq!(check(&cpu::EqI32, &[I32, I32]), ok(&["bool"]));
        assert_eq!(check(&cpu::GeU32, &[U64, U32]), mismatch("u32", "u64"));
        // Logic takes bool.
        assert_eq!(check(&cpu::And, &[Bool, Bool]), ok(&["bool"]));
        assert_eq!(check(&cpu::Not, &[U64]), mismatch("bool", "u64"));
        // Underflow is reported once, before anything is popped.
        assert_eq!(
            check(&cpu::AddU64, &[U64]),
            Err(DiagnosticKind::StackUnderflow)
        );
    }

    #[test]
    fn array_rules_are_typed_by_their_element() {
        let array_f64 = Type::array(F64);
        assert_eq!(
            check(&cpu::ArrayNew(F64, 2), &[F64, F64]),
            ok(&["array f64"])
        );
        assert_eq!(check(&cpu::ArrayNew(F64, 0), &[]), ok(&["array f64"]));
        assert_eq!(
            check(&cpu::ArrayNew(F64, 2), &[U64, F64]),
            mismatch("f64", "u64")
        );
        assert_eq!(
            check(&cpu::ArrayNew(F64, u32::MAX), &[F64]),
            Err(DiagnosticKind::StackUnderflow)
        );
        assert_eq!(
            check(&cpu::ArrayReserve(Type::Struct(0)), &[U64]),
            ok(&["array struct 0"])
        );
        assert_eq!(
            check(&cpu::ArrayPush(F64), &[array_f64.clone(), F64]),
            ok(&["array f64"])
        );
        assert_eq!(
            check(&cpu::ArrayGet(F64), &[array_f64.clone(), U64]),
            ok(&["f64"])
        );
        assert_eq!(
            check(&cpu::ArrayGet(U64), &[array_f64.clone(), U64]),
            mismatch("array u64", "array f64")
        );
        assert_eq!(
            check(&cpu::ArrayGet(F64), &[array_f64.clone(), I64]),
            mismatch("u64", "i64")
        );
        assert_eq!(
            check(&cpu::ArraySet(F64), &[array_f64.clone(), U64, F64]),
            ok(&[])
        );
        assert_eq!(
            check(&cpu::ArrayLen, &[Type::array(Type::Struct(0))]),
            ok(&["u64"])
        );
        assert_eq!(
            check(&cpu::ArrayLen, &[HeapRef]),
            mismatch("array _", "heap")
        );
        // `heap_dealloc` takes any heap object.
        for object in [HeapRef, Type::Struct(0), array_f64] {
            assert_eq!(check(&cpu::HeapDealloc, &[object]), ok(&[]));
        }
        assert_eq!(check(&cpu::HeapDealloc, &[U64]), mismatch("heap", "u64"));
    }

    #[test]
    fn struct_rules_use_the_struct_table() {
        let point = StructLayout {
            struct_id: 0,
            field_count: 2,
        };
        let field = |slot| FieldSlot { struct_id: 0, slot };
        let point_ty = Type::Struct(0);
        assert_eq!(
            check(&cpu::StructNew(point), &[I64, F64]),
            ok(&["struct 0"])
        );
        assert_eq!(
            check(&cpu::StructNew(point), &[F64, I64]),
            mismatch("i64", "f64")
        );
        let short = StructLayout {
            struct_id: 0,
            field_count: 1,
        };
        assert!(matches!(
            check(&cpu::StructNew(short), &[I64]),
            Err(DiagnosticKind::Rule(message)) if message.contains("declares 2")
        ));
        assert_eq!(
            check(&cpu::GetField(field(1)), std::slice::from_ref(&point_ty)),
            ok(&["f64"])
        );
        assert_eq!(
            check(&cpu::GetField(field(0)), &[HeapRef]),
            mismatch("struct 0", "heap")
        );
        assert_eq!(
            check(&cpu::SetField(field(0)), &[point_ty.clone(), I64]),
            ok(&[])
        );
        assert_eq!(
            check(&cpu::SetField(field(0)), &[point_ty, F64]),
            mismatch("i64", "f64")
        );
        assert!(matches!(
            check(
                &cpu::GetField(FieldSlot {
                    struct_id: 3,
                    slot: 0
                }),
                &[]
            ),
            Err(DiagnosticKind::Rule(_))
        ));
    }

    #[test]
    fn wrap_and_unwrap_convert_between_a_named_type_and_its_representation() {
        assert_eq!(check(&cpu::Wrap(0), &[U64]), ok(&["type 0"]));
        assert_eq!(check(&cpu::Unwrap(1), &[Type::Named(1)]), ok(&["u64"]));
        assert_eq!(check(&cpu::Wrap(0), &[I64]), mismatch("u64", "i64"));
        assert_eq!(
            check(&cpu::Unwrap(0), &[Type::Named(1)]),
            mismatch("type 0", "type 1")
        );
        assert_eq!(check(&cpu::Unwrap(0), &[U64]), mismatch("type 0", "u64"));
        assert!(matches!(
            check(&cpu::Wrap(2), &[U64]),
            Err(DiagnosticKind::Rule(_))
        ));
    }

    #[test]
    fn computation_instructions_continue_to_the_next_address() {
        assert_eq!(flow(&cpu::AddI64), Flow::Next);
        assert_eq!(flow(&cpu::ArrayLen), Flow::Next);
        assert_eq!(flow(&cpu::Wrap(0)), Flow::Next);
    }
}

#[cfg(not(feature = "program-image-execution"))]
mod program {
    //! Control flow, calls, locals, and constants: in the
    //! `typed-declarations` dialect only.

    use crate::cpu;
    use crate::word::{decode_function_ref, decode_string_id};
    use vihaco_abi::{Flow, Type, Verify};
    use vihaco_verifier::{
        Constants, Locals, OperandStack, Signatures, TypedSlot, direct_call_effect,
        function_constant, indirect_call_effect, load_local, return_effect, stack_effect,
        store_local, string_constant,
    };

    /// Instructions with no stack effect that continue to the next address.
    macro_rules! no_op {
        ($($instruction:ident),*) => {$(
            impl<C: ?Sized> Verify<C> for cpu::$instruction {
                fn verify(&self, _: &mut C) -> eyre::Result<()> {
                    Ok(())
                }
            }
        )*};
    }

    no_op!(Span, Breakpoint);

    impl<C: ?Sized> Verify<C> for cpu::Branch {
        fn flow(&self) -> Flow {
            Flow::jump(self.0)
        }

        fn verify(&self, _: &mut C) -> eyre::Result<()> {
            Ok(())
        }
    }

    /// Pops the `bool` condition.
    impl<C> Verify<C> for cpu::ConditionalBranch
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

    impl<C: ?Sized> Verify<C> for cpu::Halt {
        fn flow(&self) -> Flow {
            Flow::Halt
        }

        fn verify(&self, _: &mut C) -> eyre::Result<()> {
            Ok(())
        }
    }

    impl<C> Verify<C> for cpu::Return
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

    impl<C> Verify<C> for cpu::Call
    where
        C: OperandStack + Signatures<Type> + ?Sized,
        C::Slot: TypedSlot,
    {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            direct_call_effect(cx, self.0, self.1)
        }
    }

    impl<C> Verify<C> for cpu::IndirectCall
    where
        C: OperandStack + ?Sized,
        C::Slot: TypedSlot,
    {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            indirect_call_effect(cx, &self.0)
        }
    }

    /// `[str] -> []`: the composite resolves the string the CPU pops.
    impl<C> Verify<C> for cpu::Print
    where
        C: OperandStack + ?Sized,
        C::Slot: TypedSlot,
    {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            stack_effect(cx, [Type::String], [])
        }
    }

    /// Pushes a copy of the top slot.
    impl<C> Verify<C> for cpu::Dup
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
            impl<C> Verify<C> for cpu::$load
            where
                C: Locals + ?Sized,
                C::Slot: TypedSlot,
            {
                fn verify(&self, cx: &mut C) -> eyre::Result<()> {
                    load_local(cx, self.0, $ty.as_ref())
                }
            }

            impl<C> Verify<C> for cpu::$store
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
            impl<C> Verify<C> for cpu::$instruction
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
    impl<C> Verify<C> for cpu::ConstString
    where
        C: OperandStack + Constants<vihaco_abi::Value, Type> + ?Sized,
        C::Slot: TypedSlot,
    {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            string_constant(cx, decode_string_id(self.0))
        }
    }

    /// Pushes `fn`; the index must be in the function table.
    impl<C> Verify<C> for cpu::ConstFunctionRef
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
        use super::super::tests::{check, flow, mismatch, ok};
        use crate::cpu;
        use crate::word::{encode_function_ref, encode_string_id};
        use vihaco_abi::{Flow, FunctionType, Type};
        use vihaco_verifier::DiagnosticKind;

        use Type::{Bool, F64, I64, U64};

        #[test]
        fn control_flow_is_stateless_and_typed() {
            assert_eq!(flow(&cpu::Branch(7)), Flow::jump(7));
            assert_eq!(check(&cpu::Branch(7), &[U64]), ok(&["u64"]));
            assert_eq!(flow(&cpu::ConditionalBranch(3, 9)), Flow::jump_to([3, 9]));
            assert_eq!(check(&cpu::ConditionalBranch(3, 9), &[Bool]), ok(&[]));
            assert_eq!(
                check(&cpu::ConditionalBranch(3, 9), &[U64]),
                mismatch("bool", "u64")
            );
            assert_eq!(flow(&cpu::Halt), Flow::Halt);
            assert_eq!(check(&cpu::Halt, &[U64]), ok(&["u64"]));
            assert_eq!(flow(&cpu::Return(1)), Flow::Return);
            // `main` returns one u64; anything below it is dropped.
            assert_eq!(check(&cpu::Return(1), &[F64, U64]), ok(&["f64"]));
            assert_eq!(check(&cpu::Return(1), &[F64]), mismatch("u64", "f64"));
            assert_eq!(
                check(&cpu::Return(0), &[]),
                Err(DiagnosticKind::ReturnCountMismatch {
                    declared: 1,
                    found: 0
                })
            );
            for no_op in [flow(&cpu::Span(0, 1, 2)), flow(&cpu::Breakpoint)] {
                assert_eq!(no_op, Flow::Next);
            }
            assert_eq!(check(&cpu::Span(0, 1, 2), &[]), ok(&[]));
            assert_eq!(check(&cpu::Breakpoint, &[I64]), ok(&["i64"]));
            assert_eq!(check(&cpu::Print, &[Type::String]), ok(&[]));
            assert_eq!(check(&cpu::Print, &[U64]), mismatch("str", "u64"));
        }

        #[test]
        fn calls_are_checked_against_signatures() {
            // `helper` (function 1) starts at address 1: (f64) -> bool.
            assert_eq!(flow(&cpu::Call(1, 1)), Flow::Next);
            assert_eq!(check(&cpu::Call(1, 1), &[F64]), ok(&["bool"]));
            assert_eq!(check(&cpu::Call(1, 1), &[U64]), mismatch("f64", "u64"));
            assert_eq!(
                check(&cpu::Call(2, 1), &[F64, F64]),
                Err(DiagnosticKind::CallArityMismatch {
                    callee: 1,
                    declared: 1,
                    found: 2
                })
            );
            assert!(matches!(
                check(&cpu::Call(0, 5), &[]),
                Err(DiagnosticKind::Rule(message)) if message.contains("address 5")
            ));
            // An indirect call is typed by its operand.
            let unary = cpu::IndirectCall(FunctionType::new([U64], [Bool, F64]));
            assert_eq!(
                check(&unary, &[U64, Type::FunctionRef]),
                ok(&["bool", "f64"])
            );
            assert_eq!(
                check(&unary, &[Type::FunctionRef, U64]),
                mismatch("fn", "u64")
            );
            assert_eq!(
                check(&unary, &[I64, Type::FunctionRef]),
                mismatch("u64", "i64")
            );
            assert_eq!(
                check(&unary, &[Type::FunctionRef]),
                Err(DiagnosticKind::StackUnderflow)
            );
        }

        #[test]
        fn locals_are_used_at_their_declared_types() {
            // `main` declares locals [u64, struct 0].
            assert_eq!(check(&cpu::LoadU64(0), &[]), ok(&["u64"]));
            assert_eq!(check(&cpu::LoadI64(0), &[]), mismatch("i64", "u64"));
            assert_eq!(check(&cpu::StoreU64(0), &[U64]), ok(&[]));
            assert_eq!(check(&cpu::StoreU64(0), &[I64]), mismatch("u64", "i64"));
            assert_eq!(check(&cpu::StoreF64(0), &[F64]), mismatch("f64", "u64"));
            assert_eq!(check(&cpu::Load(1), &[]), ok(&["struct 0"]));
            assert_eq!(check(&cpu::Store(1), &[Type::Struct(0)]), ok(&[]));
            assert_eq!(
                check(&cpu::Store(1), &[Type::HeapRef]),
                mismatch("struct 0", "heap")
            );
            assert_eq!(
                check(&cpu::Load(2), &[]),
                Err(DiagnosticKind::LocalIndexOutOfRange { index: 2, count: 2 })
            );
            assert_eq!(
                check(&cpu::StoreBool(9), &[Bool]),
                Err(DiagnosticKind::LocalIndexOutOfRange { index: 9, count: 2 })
            );
            assert_eq!(check(&cpu::Dup, &[I64, F64]), ok(&["i64", "f64", "f64"]));
            assert_eq!(check(&cpu::Dup, &[]), Err(DiagnosticKind::StackUnderflow));
        }

        #[test]
        fn constants_push_their_types() {
            assert_eq!(check(&cpu::ConstI64(0), &[]), ok(&["i64"]));
            assert_eq!(check(&cpu::ConstBool(1), &[]), ok(&["bool"]));
            assert_eq!(
                check(&cpu::ConstString(encode_string_id(6)), &[]),
                ok(&["str"])
            );
            assert!(matches!(
                check(&cpu::ConstString(encode_string_id(7)), &[]),
                Err(DiagnosticKind::Rule(message)) if message.contains("string 7")
            ));
            assert_eq!(
                check(&cpu::ConstFunctionRef(encode_function_ref(1)), &[]),
                ok(&["fn"])
            );
            assert!(matches!(
                check(&cpu::ConstFunctionRef(encode_function_ref(2)), &[]),
                Err(DiagnosticKind::Rule(_))
            ));
        }
    }
}

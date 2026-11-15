// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Shared building blocks for standard instruction rules.
//!
//! Dialects that implement the same kind of instruction (the CPU and the
//! stdlib program device both have locals, constants, and calls) use these so
//! the rules agree. Each helper bounds its context only on the capabilities it
//! needs.

use vihaco_abi::{FunctionType, Type};

use crate::capability::{Constants, Locals, Signatures};
use crate::diagnostic::VerifyError;
use crate::slot::TypedSlot;
use crate::stack::OperandStack;

/// Pop a reference to an array of any element type, returning the element
/// type; for instructions such as `array_len` that accept every `array T`.
pub fn pop_array<C>(cx: &mut C) -> Result<Type, VerifyError>
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    let slot = cx.pop()?;
    match slot.known_type() {
        Some(Type::Array(elem)) => Ok((**elem).clone()),
        _ => Err(VerifyError::type_mismatch("array _", &slot)),
    }
}

/// Apply the stack effect of a direct call by address, `call arity, address`:
/// resolve the callee with [`Signatures::function_at`] as the runtime does,
/// then apply [`call_effect`](crate::call_effect).
pub fn direct_call_effect<C>(cx: &mut C, arity: u32, address: u32) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let callee = cx.function_at(address)?;
    crate::call_effect(cx, callee, arity as usize)
}

/// Apply the stack effect of an indirect call with the expected signature
/// `ty`: pop the function reference (`fn`, on top), then one argument per
/// parameter, checking each against its type, then push the results and
/// apply [`Signatures::after_call`] with no known callee. The runtime checks
/// that the referenced function has exactly this signature before entering
/// it.
pub fn indirect_call_effect<C>(cx: &mut C, ty: &FunctionType) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    if cx.depth() <= ty.params.len() {
        return Err(VerifyError::stack_underflow().into());
    }
    cx.pop_expect(&Type::FunctionRef)?;
    let arguments = cx.pop_n(ty.params.len())?;
    for (argument, parameter) in arguments.iter().zip(&ty.params) {
        argument.expect(parameter)?;
    }
    for result in &ty.results {
        cx.push(C::Slot::of_type(result.clone()));
    }
    cx.after_call(None);
    Ok(())
}

/// Push local `index`. A typed load (`load_u64 i`) passes the type its
/// mnemonic names, which the local's declared type must be exactly; an untyped
/// load (`load i`) passes `None` and pushes the local's declared type.
pub fn load_local<C>(cx: &mut C, index: u32, expected: Option<&Type>) -> eyre::Result<()>
where
    C: Locals + ?Sized,
    C::Slot: TypedSlot,
{
    let local = cx.local(index)?.clone();
    if let Some(expected) = expected {
        expect_local_type(&local, expected)?;
    }
    cx.push(local);
    Ok(())
}

/// Pop the top operand into local `index`. A typed store (`store_u64 i`)
/// passes the type its mnemonic names, which the local's declared type must be
/// exactly; an untyped store (`store i`) passes `None`. Either way the operand
/// must satisfy the local's declared type, and the local keeps that type: a
/// store never retypes a local.
pub fn store_local<C>(cx: &mut C, index: u32, expected: Option<&Type>) -> eyre::Result<()>
where
    C: Locals + ?Sized,
    C::Slot: TypedSlot,
{
    let local = cx.local(index)?;
    let declared = match (expected, local.known_type()) {
        (Some(expected), _) => {
            expect_local_type(local, expected)?;
            expected.clone()
        }
        (None, Some(declared)) => declared.clone(),
        (None, None) => return Err(VerifyError::type_mismatch("a declared type", local).into()),
    };
    cx.pop_expect(&declared)?;
    Ok(())
}

/// A local accessed by a typed load or store must be declared with exactly
/// the mnemonic's type, not merely a type that satisfies it.
fn expect_local_type<S: TypedSlot>(local: &S, expected: &Type) -> Result<(), VerifyError> {
    match local.known_type() {
        Some(declared) if declared == expected => Ok(()),
        _ => Err(VerifyError::type_mismatch(expected, local)),
    }
}

/// Push a string reference (`str`) to entry `index` of the string table, for
/// `const_string`. An index outside the table is a rule error.
pub fn string_constant<C, V>(cx: &mut C, index: u32) -> eyre::Result<()>
where
    C: OperandStack + Constants<V, Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let count = cx.string_count();
    if index as usize >= count {
        return Err(VerifyError::rule(format!(
            "string {index} is outside the string table of length {count}"
        ))
        .into());
    }
    cx.push(C::Slot::of_type(Type::String));
    Ok(())
}

/// Push a function reference (`fn`) to entry `index` of the function table,
/// for `const_fn_ref`. An index outside the table is a rule error.
pub fn function_constant<C>(cx: &mut C, index: u32) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    cx.signature(index)?;
    cx.push(C::Slot::of_type(Type::FunctionRef));
    Ok(())
}

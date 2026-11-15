// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Shared building blocks for standard instruction rules.
//!
//! Dialects that implement the same kind of instruction (the CPU and the
//! stdlib program device both have locals, constants, and calls) use these so
//! the rules agree. Each helper bounds its context only on the capabilities it
//! needs.

use vihaco_abi::{FunctionType, Type};

use crate::capability::{Constants, Locals, Signatures, StructTypes};
use crate::diagnostic::{DiagnosticKind, Expected, Role, Table, VerifyError};
use crate::slot::TypedSlot;
use crate::stack::OperandStack;

/// Pop a reference to an array of any element type, returning the element
/// type; for instructions such as `array_len` that accept every `array T`.
/// The array is reported as operand 0 if it is not one.
pub fn pop_array<C>(cx: &mut C) -> Result<Type, VerifyError>
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    let slot = cx.pop()?;
    match slot.known_type() {
        Some(Type::Array(elem)) => Ok((**elem).clone()),
        _ => Err(
            VerifyError::type_mismatch(Expected::AnyArray, slot.to_type_slot())
                .with_role(Role::Operand(0)),
        ),
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
/// `ty`: pop the function reference (`fn`, on top, so the last operand), then
/// one argument per parameter, checking each against its type
/// ([`Role::Argument`] with no callee), then push the results and apply
/// [`Signatures::after_call`] with no known callee. The runtime checks that
/// the referenced function has exactly this signature before entering it.
pub fn indirect_call_effect<C>(cx: &mut C, ty: &FunctionType) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let needed = ty.params.len() + 1;
    if cx.depth() < needed {
        return Err(VerifyError::stack_underflow(needed, cx.depth()).into());
    }
    cx.pop_typed(&Type::FunctionRef)
        .map_err(|error| error.with_role(Role::Operand(ty.params.len() as u32)))?;
    let arguments = cx.pop_n(ty.params.len())?;
    for (index, (argument, parameter)) in arguments.iter().zip(&ty.params).enumerate() {
        argument.check(parameter).map_err(|error| {
            error.with_role(Role::Argument {
                callee: None,
                index: index as u32,
            })
        })?;
    }
    for result in &ty.results {
        cx.push(C::Slot::of_type(result.clone()));
    }
    cx.after_call(None);
    Ok(())
}

/// Push local `index`. A typed load (`load_u64 i`) passes the type its
/// mnemonic names, which the local's declared type must be exactly
/// ([`Role::Local`] otherwise); an untyped load (`load i`) passes `None` and
/// pushes the local's declared type.
pub fn load_local<C>(cx: &mut C, index: u32, expected: Option<&Type>) -> eyre::Result<()>
where
    C: Locals + ?Sized,
    C::Slot: TypedSlot,
{
    let local = cx.local(index)?.clone();
    if let Some(expected) = expected {
        local
            .check_exact(expected)
            .map_err(|error| error.with_role(Role::Local(index)))?;
    }
    cx.push(local);
    Ok(())
}

/// Pop the top operand into local `index`. A typed store (`store_u64 i`)
/// passes the type its mnemonic names, which the local's declared type must be
/// exactly ([`Role::Local`] otherwise); an untyped store (`store i`) passes
/// `None`. Either way the operand must satisfy the local's declared type
/// ([`Role::StoredValue`] otherwise), and the local keeps that type: a store
/// never retypes a local.
pub fn store_local<C>(cx: &mut C, index: u32, expected: Option<&Type>) -> eyre::Result<()>
where
    C: Locals + ?Sized,
    C::Slot: TypedSlot,
{
    let local = cx.local(index)?;
    let declared = match (expected, local.known_type()) {
        (Some(expected), _) => {
            local
                .check_exact(expected)
                .map_err(|error| error.with_role(Role::Local(index)))?;
            expected.clone()
        }
        (None, Some(declared)) => declared.clone(),
        (None, None) => {
            return Err(
                VerifyError::type_mismatch(Expected::AnyKnown, local.to_type_slot())
                    .with_role(Role::Local(index))
                    .into(),
            );
        }
    };
    cx.pop_typed(&declared)
        .map_err(|error| error.with_role(Role::StoredValue(index)))?;
    Ok(())
}

/// Push a string reference (`str`) to entry `index` of the string table, for
/// `const_string`. An index outside the table is a
/// [`DiagnosticKind::UnknownEntry`].
pub fn string_constant<C, V>(cx: &mut C, index: u32) -> eyre::Result<()>
where
    C: OperandStack + Constants<V, Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let count = cx.string_count();
    if index as usize >= count {
        return Err(VerifyError::unknown_entry(Table::Strings, index, count).into());
    }
    cx.push(C::Slot::of_type(Type::String));
    Ok(())
}

/// Push a function reference (`fn`) to entry `index` of the function table,
/// for `const_fn_ref`. An index outside the table is a
/// [`DiagnosticKind::UnknownEntry`].
pub fn function_constant<C>(cx: &mut C, index: u32) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    cx.signature(index)?;
    cx.push(C::Slot::of_type(Type::FunctionRef));
    Ok(())
}

/// Apply `struct_new`: pop one value per field of struct `struct_id`, checking
/// each against the field's declared type ([`Role::Field`]), then push
/// `struct N`.
///
/// `field_count` is the count the instruction carries (the runtime pops that
/// many); it must equal the struct table's, or the rule reports
/// [`DiagnosticKind::FieldCountMismatch`].
pub fn struct_new_effect<C>(cx: &mut C, struct_id: u32, field_count: u32) -> eyre::Result<()>
where
    C: OperandStack + StructTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let declared = cx.field_count(struct_id)?;
    if field_count as usize != declared {
        return Err(VerifyError::new(DiagnosticKind::FieldCountMismatch {
            struct_id,
            declared,
            found: field_count as usize,
        })
        .into());
    }
    let fields = (0..declared as u32)
        .map(|slot| cx.field_type(struct_id, slot).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    for (field, (value, ty)) in cx.pop_n(declared)?.iter().zip(&fields).enumerate() {
        value.check(ty).map_err(|error| {
            error.with_role(Role::Field {
                struct_id,
                field: field as u32,
            })
        })?;
    }
    cx.push(C::Slot::of_type(Type::Struct(struct_id)));
    Ok(())
}

/// Apply `set_field`: pop the new value, checked against the field's declared
/// type ([`Role::Field`]), and the `struct N` below it (operand 0).
pub fn set_field_effect<C>(cx: &mut C, struct_id: u32, field: u32) -> eyre::Result<()>
where
    C: OperandStack + StructTypes<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let ty = cx.field_type(struct_id, field)?.clone();
    if cx.depth() < 2 {
        return Err(VerifyError::stack_underflow(2, cx.depth()).into());
    }
    cx.pop_typed(&ty)
        .map_err(|error| error.with_role(Role::Field { struct_id, field }))?;
    cx.pop_typed(&Type::Struct(struct_id))
        .map_err(|error| error.with_role(Role::Operand(0)))?;
    Ok(())
}

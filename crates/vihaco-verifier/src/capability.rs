// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard verifier capabilities beyond the operand stack.
//!
//! Each capability is a small trait, so a rule bounds its context only on
//! what it uses. [`ModuleVerifier`](crate::ModuleVerifier) provides all of
//! them from a module's metadata; a machine context that wraps it delegates
//! them (see [`WithState`](crate::WithState)).

use vihaco_abi::Type;
use vihaco_module::module::Signature;

use crate::diagnostic::{Role, VerifyError};
use crate::slot::TypedSlot;
use crate::stack::OperandStack;

/// Capability: the abstract locals of the function being verified.
///
/// Locals share the operand stack's [`Slot`](OperandStack::Slot) type, so a
/// rule can move slots between the two (`load_*` pushes a local, `store_*`
/// pops into one). Every index is bounds-checked against the function's
/// declared locals; an out-of-range index is a
/// [`DiagnosticKind::LocalIndexOutOfRange`](crate::DiagnosticKind::LocalIndexOutOfRange).
pub trait Locals: OperandStack {
    /// The number of locals the function declares, parameters included.
    fn local_count(&self) -> u32;

    /// Local `index`.
    fn local(&self, index: u32) -> Result<&Self::Slot, VerifyError>;

    /// Overwrite local `index` with `slot`.
    fn set_local(&mut self, index: u32, slot: Self::Slot) -> Result<(), VerifyError>;
}

/// Capability: function signatures, for calls, returns and function
/// references.
///
/// Function indices are positions in the module's function table. Lookups of
/// an unknown function report [`DiagnosticKind::UnknownEntry`], and of an
/// address where no function starts [`DiagnosticKind::NoFunctionAt`].
///
/// [`DiagnosticKind::UnknownEntry`]: crate::DiagnosticKind::UnknownEntry
/// [`DiagnosticKind::NoFunctionAt`]: crate::DiagnosticKind::NoFunctionAt
pub trait Signatures<Ty> {
    /// The number of functions in the function table.
    fn function_count(&self) -> usize;

    /// The signature of function `function`.
    fn signature(&self, function: u32) -> Result<&Signature<Ty>, VerifyError>;

    /// The index of the function whose first instruction is at `address`, as
    /// a direct `call` by address resolves it at runtime (the first such entry
    /// in table order).
    fn function_at(&self, address: u32) -> Result<u32, VerifyError>;

    /// The index of the function being verified.
    fn current_function(&self) -> u32;

    /// The signature of the function being verified, for `ret`.
    fn current_signature(&self) -> Result<&Signature<Ty>, VerifyError> {
        self.signature(self.current_function())
    }

    /// Apply what a call does to state beyond the operand stack, after its
    /// operand effect. `callee` is the called function for a direct call and
    /// `None` for an indirect one. [`call_effect`] and
    /// [`indirect_call_effect`](crate::indirect_call_effect) call this.
    ///
    /// The callee is verified separately, so the caller knows nothing about
    /// what it did: a context with component state must forget whatever the
    /// callee could have changed ([`WithState`](crate::WithState) applies
    /// [`ComponentState::after_call`](crate::ComponentState::after_call)). A
    /// context whose only state is the frame has nothing to do. There is no
    /// default, so a context cannot skip this by omission.
    fn after_call(&mut self, callee: Option<u32>);
}

/// Capability: the struct table, for `struct_new`, `get_field` and
/// `set_field`.
///
/// Struct indices are positions in the struct table (`struct N`); field slots
/// are positions in a struct's field list. An unknown struct reports
/// [`DiagnosticKind::UnknownEntry`], an unknown field
/// [`DiagnosticKind::UnknownField`].
///
/// [`DiagnosticKind::UnknownEntry`]: crate::DiagnosticKind::UnknownEntry
/// [`DiagnosticKind::UnknownField`]: crate::DiagnosticKind::UnknownField
pub trait StructTypes<Ty> {
    /// The number of structs in the struct table.
    fn struct_count(&self) -> usize;

    /// The number of fields struct `struct_id` declares.
    fn field_count(&self, struct_id: u32) -> Result<usize, VerifyError>;

    /// The declared type of field `slot` of struct `struct_id`.
    fn field_type(&self, struct_id: u32, slot: u32) -> Result<&Ty, VerifyError>;
}

/// Capability: the named-type table, for `wrap`, `unwrap`, and component
/// rules whose operands have named types.
///
/// Named-type indices are positions in the table (`type N`). A device rule
/// names its operand types by name (`named_type_index("location")`), so it
/// works in any module that declares them. An unknown name reports
/// [`DiagnosticKind::UndefinedNamedType`], an unknown index
/// [`DiagnosticKind::UnknownEntry`].
///
/// [`DiagnosticKind::UnknownEntry`]: crate::DiagnosticKind::UnknownEntry
/// [`DiagnosticKind::UndefinedNamedType`]: crate::DiagnosticKind::UndefinedNamedType
pub trait NamedTypes<Ty> {
    /// The number of entries in the named-type table.
    fn named_type_count(&self) -> usize;

    /// The index of the named type called `name`.
    fn named_type_index(&self, name: &str) -> Result<u32, VerifyError>;

    /// The scalar representation of named type `index`.
    fn named_type_repr(&self, index: u32) -> Result<&Ty, VerifyError>;
}

/// Capability: the constant and string tables, for constant loads and
/// `const_string`.
///
/// Constant indices are positions in the constant table, string indices
/// positions in the string table. Unknown constants report
/// [`DiagnosticKind::UnknownEntry`].
///
/// [`DiagnosticKind::UnknownEntry`]: crate::DiagnosticKind::UnknownEntry
pub trait Constants<V, Ty> {
    /// The number of constants in the constant table.
    fn constant_count(&self) -> usize;

    /// Constant `index`.
    fn constant(&self, index: u32) -> Result<&V, VerifyError>;

    /// The type of constant `index`, as pushed by a constant load.
    fn constant_type(&self, index: u32) -> Result<Ty, VerifyError>;

    /// The number of entries in the string table.
    fn string_count(&self) -> usize;
}

/// Apply the stack effect of a direct call to function `callee` passing
/// `arguments` operands: check the count against the callee's declared
/// parameters ([`DiagnosticKind::CallArityMismatch`]), pop the arguments and
/// check each against its parameter type ([`Role::Argument`]), then push the
/// declared results, and finally apply [`Signatures::after_call`].
///
/// [`DiagnosticKind::CallArityMismatch`]: crate::DiagnosticKind::CallArityMismatch
pub fn call_effect<C>(cx: &mut C, callee: u32, arguments: usize) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let signature = cx.signature(callee)?.clone();
    let declared = signature.params.len();
    if arguments != declared {
        return Err(VerifyError::call_arity_mismatch(callee, declared, arguments).into());
    }
    let popped = cx.pop_n(declared)?;
    for (index, (argument, parameter)) in popped.iter().zip(&signature.params).enumerate() {
        argument.check(&parameter.ty).map_err(|error| {
            error.with_role(Role::Argument {
                callee: Some(callee),
                index: index as u32,
            })
        })?;
    }
    for ty in signature.ret {
        cx.push(C::Slot::of_type(ty));
    }
    cx.after_call(Some(callee));
    Ok(())
}

/// Check a return keeping the top `count` operands against the current
/// function's declared results: `count` must equal the declared result count
/// ([`DiagnosticKind::ReturnCountMismatch`]) and each kept operand must match
/// its result type. The kept operands are popped; anything below them is
/// dropped at runtime and needs no check.
///
/// [`DiagnosticKind::ReturnCountMismatch`]: crate::DiagnosticKind::ReturnCountMismatch
pub fn return_effect<C>(cx: &mut C, count: usize) -> eyre::Result<()>
where
    C: OperandStack + Signatures<Type> + ?Sized,
    C::Slot: TypedSlot,
{
    let results = cx.current_signature()?.ret.clone();
    if count != results.len() {
        return Err(VerifyError::return_count_mismatch(results.len(), count).into());
    }
    let kept = cx.pop_n(count)?;
    for (index, (value, ty)) in kept.iter().zip(&results).enumerate() {
        value
            .check(ty)
            .map_err(|error| error.with_role(Role::Return(index as u32)))?;
    }
    Ok(())
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load-time verification for vihaco programs.
//!
//! Components own their typing rules: each instruction implements
//! [`Verify<C>`](Verify), bounding `C` on the capabilities it needs (such as
//! [`OperandStack`]). This crate owns the generic parts: finding basic blocks
//! and reachable code from [`Verify::flow`], walking each function to a
//! fixpoint with strict-change joins at leaders, structural checks, two-phase
//! reporting, and structured [`Diagnostic`]s. See `design/verifier.md`.
//!
//! The building blocks:
//!
//! - [`Lattice`], [`TypedSlot`] and the standard [`TypeSlot`]: abstract values.
//! - [`OperandStack`]: the operand-stack capability rules are written
//!   against, with [`stack_effect`] for fixed-signature instructions.
//! - [`AbstractState`]: how the engine snapshots, restores and joins a
//!   context's state. [`FrameState`] implements it (and [`OperandStack`]) for
//!   a standard [`Frame`] of locals and operands; a machine-specific context
//!   can wrap it and join its own state alongside.
//! - [`verify_functions`]: the engine, returning a [`Verification`] (with
//!   any warnings) or a [`Rejection`] listing every diagnostic. Either carries
//!   [`TypeMaps`] when [`VerifyOptions::type_maps`] asks for them.
//! - [`Diagnostic`] and [`DiagnosticKind`]: structured findings. Rules raise
//!   them through [`VerifyError`]; [`VerifyError::unverifiable`] reports an
//!   instruction whose operand count comes from a runtime value
//!   ([`DiagnosticKind::Unverifiable`]), since device instructions must carry
//!   counts as immediates to be checked.
//! - [`Locals`], [`Signatures`], [`StructTypes`], [`NamedTypes`] and
//!   [`Constants`]: the standard capabilities beyond the operand stack, with
//!   [`call_effect`] and [`return_effect`] for calls and returns checked
//!   against signatures.
//! - Shared rule helpers for instructions several dialects implement:
//!   [`load_local`] and [`store_local`], [`direct_call_effect`] and
//!   [`indirect_call_effect`], [`string_constant`] and [`function_constant`],
//!   and [`pop_array`].
//! - [`ModuleVerifier`]: the standard context for a [`LocalModule`] using
//!   vihaco's [`Type`] and [`Value`], providing every standard capability
//!   from the module's metadata. [`verify_module`] verifies a whole module
//!   with it. [`WithState`] wraps it with a component's own
//!   [`ComponentState`], for machines whose components need more.
//! - [`Verified`]: a module that passed verification, built by
//!   [`Verified::new`], [`Verified::verify`] or
//!   [`Verified::verify_with_state`], or explicitly by
//!   [`Verified::assume_verified`]. The stdlib `Image::new` accepts only a
//!   `Verified` module, so programs are verified as part of loading.
//!
//! [`LocalModule`]: vihaco_module::module::LocalModule
//!
//! # Embedding
//!
//! A host verifies a lowered module once, before building its image:
//!
//! ```ignore
//! let (module, verification) = Verified::verify(module, VerifyOptions::default())?;
//! for warning in &verification.warnings {
//!     eprintln!("{warning}"); // for example, unreachable code
//! }
//! let image = vihaco_stdlib::Image::new(module);
//! ```
//!
//! A [`Rejection`] lists every error, sorted by function and address, and
//! converts into an [`eyre::Report`]. With
//! [`VerifyOptions::with_type_maps`], both outcomes carry the converged state
//! at each reached block leader, for debuggers and editors.
//!
//! # Writing rules
//!
//! A rule implements [`Verify<C>`](Verify) for an instruction, bounding `C`
//! only on the capabilities it uses, so it works in any machine whose context
//! provides them; the `#[composite]` macro aggregates the rules of every
//! device. Rules that only need the operand stack use [`stack_effect`];
//! control-flow instructions also override [`Verify::flow`].
//!
//! Facade users reach this crate as `vihaco::verifier` (the `verifier`
//! feature, implied by `program-image-execution`).
//!
//! # Example
//!
//! A tiny instruction set with typed rules, verified with the bare engine and
//! the standard frame (a module is usually verified with [`verify_module`]
//! instead, which builds the entry states from its metadata):
//!
//! ```
//! use vihaco_verifier::{
//!     DiagnosticKind, Flow, Frame, FrameState, FunctionSpan, OperandStack, Role, Type, TypeSlot,
//!     TypedSlot, Verify, VerifyOptions, stack_effect, verify_functions,
//! };
//!
//! enum Inst {
//!     ConstI64,
//!     ConstF64,
//!     AddI64,
//!     Ret,
//! }
//!
//! impl<C> Verify<C> for Inst
//! where
//!     C: OperandStack + ?Sized,
//!     C::Slot: TypedSlot,
//! {
//!     fn flow(&self) -> Flow {
//!         match self {
//!             Inst::Ret => Flow::Return,
//!             _ => Flow::Next,
//!         }
//!     }
//!
//!     fn verify(&self, cx: &mut C) -> eyre::Result<()> {
//!         match self {
//!             Inst::ConstI64 => stack_effect(cx, [], [Type::I64]),
//!             Inst::ConstF64 => stack_effect(cx, [], [Type::F64]),
//!             Inst::AddI64 => stack_effect(cx, [Type::I64, Type::I64], [Type::I64]),
//!             Inst::Ret => Ok(()),
//!         }
//!     }
//! }
//!
//! let code = [Inst::ConstI64, Inst::ConstF64, Inst::AddI64, Inst::Ret];
//! let entry = Frame::<TypeSlot>::new(vec![]);
//! let function = FunctionSpan { function: 0, start: 0, end: 4, entry };
//! let mut cx = FrameState::new(Frame::new(vec![]));
//!
//! let rejection =
//!     verify_functions(&code, &[function], &mut cx, VerifyOptions::default()).unwrap_err();
//! let [diagnostic] = rejection.diagnostics.as_slice() else { panic!() };
//! assert_eq!(diagnostic.address, 2);
//! assert_eq!(
//!     diagnostic.kind,
//!     DiagnosticKind::TypeMismatch {
//!         role: Some(Role::Operand(1)),
//!         expected: Type::I64.into(),
//!         found: TypeSlot::Known(Type::F64),
//!     }
//! );
//! ```

mod capability;
mod diagnostic;
mod engine;
mod module;
mod rules;
mod slot;
mod stack;
mod state;
mod verified;

pub use capability::{
    Constants, Locals, NamedTypes, Signatures, StructTypes, call_effect, return_effect,
};
pub use diagnostic::{
    Diagnostic, DiagnosticKind, DiagnosticNames, Expected, Predecessor, Role, Severity, Table,
    VerifyError,
};
pub use engine::{
    FunctionSpan, Rejection, TypeMaps, Verification, VerifyOptions, verify_functions,
};
pub use module::{ComponentState, ModuleVerifier, WithState, verify_module};
pub use rules::{
    direct_call_effect, function_constant, indirect_call_effect, load_local, pop_array,
    set_field_effect, store_local, string_constant, struct_new_effect,
};
pub use slot::{Lattice, TypeSlot, TypedSlot, type_satisfies};
pub use stack::{Frame, FrameState, OperandStack, stack_effect};
pub use state::{AbstractState, JoinPoint};
pub use verified::Verified;

pub use vihaco_abi::{Flow, FunctionType, Type, Value, Verify};

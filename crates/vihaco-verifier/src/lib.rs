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
//! - [`verify_functions`]: the engine, returning a [`Verification`] (possibly
//!   with warnings and [`TypeMaps`]) or a [`Rejection`] listing every
//!   diagnostic.
//! - [`Locals`], [`Signatures`], [`StructTypes`], [`NamedTypes`] and
//!   [`Constants`]: the standard capabilities beyond the operand stack, with
//!   [`call_effect`] and [`return_effect`] for calls and returns checked
//!   against signatures.
//! - [`ModuleVerifier`]: the standard context for a [`LocalModule`] using
//!   vihaco's [`Type`] and [`Value`], providing every standard capability
//!   from the module's metadata. [`verify_module`] verifies a whole module
//!   with it. [`WithState`] wraps it with a component's own
//!   [`ComponentState`], for machines whose components need more.
//!
//! [`LocalModule`]: vihaco_module::module::LocalModule
//!
//! # Example
//!
//! A tiny instruction set with typed rules, verified with the bare engine and
//! the standard frame (a module is usually verified with [`verify_module`]
//! instead, which builds the entry states from its metadata):
//!
//! ```
//! use vihaco_verifier::{
//!     DiagnosticKind, Flow, Frame, FrameState, FunctionSpan, OperandStack, Type, TypeSlot,
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
//!     DiagnosticKind::TypeMismatch { expected: "i64".into(), found: "f64".into() }
//! );
//! ```

mod capability;
mod diagnostic;
mod engine;
mod module;
mod slot;
mod stack;
mod state;

pub use capability::{
    Constants, Locals, NamedTypes, Signatures, StructTypes, call_effect, return_effect,
};
pub use diagnostic::{Diagnostic, DiagnosticKind, Severity, VerifyError};
pub use engine::{
    FunctionSpan, Rejection, TypeMaps, Verification, VerifyOptions, verify_functions,
};
pub use module::{ComponentState, ModuleVerifier, WithState, verify_module};
pub use slot::{Lattice, TypeSlot, TypedSlot, type_satisfies};
pub use stack::{Frame, FrameState, OperandStack, stack_effect};
pub use state::AbstractState;

pub use vihaco_abi::{Flow, Type, Value, Verify};

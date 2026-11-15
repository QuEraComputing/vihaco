// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! [`Verified<M>`]: proof that a module passed verification.

use std::ops::Deref;

use vihaco_abi::{Type, Value, Verify};
use vihaco_module::module::LocalModule;

use crate::engine::{Rejection, Verification, VerifyOptions};
use crate::module::{ComponentState, ModuleVerifier, WithState};
use crate::slot::TypeSlot;
use crate::stack::Frame;

/// A module that passed load-time verification.
///
/// The only ways to build one are verifying a module ([`Verified::new`],
/// [`Verified::verify`], [`Verified::verify_with_state`]) and the explicit
/// escape hatch [`Verified::assume_verified`]. Executable images take a
/// `Verified` module (the stdlib `Image::new`, and through it `Loadable`), so
/// a program cannot run without either passing verification or a greppable
/// `assume_verified` at its construction site.
///
/// `Verified` gives shared access to the module through [`Deref`] and
/// [`get`](Verified::get), but no mutable access: changing the module could
/// invalidate the verification. [`into_inner`](Verified::into_inner) gives
/// the module back, unverified.
///
/// ```
/// use vihaco_module::module::{FunctionInfo, LocalModule, Signature};
/// use vihaco_verifier::{
///     Flow, OperandStack, Type, TypedSlot, Value, Verified, Verify, stack_effect,
/// };
///
/// #[derive(Debug)]
/// enum Inst {
///     ConstI64,
///     Halt,
/// }
///
/// impl<C> Verify<C> for Inst
/// where
///     C: OperandStack + ?Sized,
///     C::Slot: TypedSlot,
/// {
///     fn flow(&self) -> Flow {
///         match self {
///             Inst::Halt => Flow::Halt,
///             Inst::ConstI64 => Flow::Next,
///         }
///     }
///     fn verify(&self, cx: &mut C) -> eyre::Result<()> {
///         match self {
///             Inst::ConstI64 => stack_effect(cx, [], [Type::I64]),
///             Inst::Halt => Ok(()),
///         }
///     }
/// }
///
/// let main = FunctionInfo {
///     name: 0,
///     signature: Signature { params: vec![], ret: vec![] },
///     locals: vec![],
///     start_address: 0,
///     end_address: 2,
///     file: 0,
/// };
/// let module: LocalModule<Inst, Value, Type> = LocalModule {
///     code: vec![Inst::ConstI64, Inst::Halt],
///     functions: vec![main],
///     main_function: Some(0),
///     ..Default::default()
/// };
/// let verified = Verified::new(module)?;
/// assert_eq!(verified.code.len(), 2);
/// # Ok::<(), eyre::Report>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Verified<M> {
    module: M,
}

impl<M> Verified<M> {
    /// Treat `module` as verified without checking it.
    ///
    /// This is the escape hatch for tests that exercise runtime checks on
    /// deliberately ill-typed programs, for modules produced by a trusted
    /// toolchain that already verified them, and for module models the
    /// standard verifier does not cover (other value or type models, or
    /// machines verified through a hand-written context). Running an
    /// unverified module is memory-safe, since the runtime still checks heap
    /// tags, bounds and signatures, but errors the verifier would reject at
    /// load time surface during execution instead.
    pub fn assume_verified(module: M) -> Self {
        Self { module }
    }

    /// The verified module.
    pub fn get(&self) -> &M {
        &self.module
    }

    /// Give the module back. It is no longer marked verified.
    pub fn into_inner(self) -> M {
        self.module
    }
}

impl<M> Deref for Verified<M> {
    type Target = M;

    fn deref(&self) -> &M {
        &self.module
    }
}

impl<M> AsRef<M> for Verified<M> {
    fn as_ref(&self) -> &M {
        &self.module
    }
}

impl<I, Info> Verified<LocalModule<I, Value, Type, Info>> {
    /// Verify `module` with the standard [`ModuleVerifier`] and default
    /// options, discarding warnings.
    ///
    /// # Errors
    ///
    /// The [`Rejection`] listing every diagnostic, if any is an error. The
    /// module is dropped; verify a borrowed module with
    /// [`verify_module`](crate::verify_module) first to keep it.
    pub fn new(
        module: LocalModule<I, Value, Type, Info>,
    ) -> Result<Self, Rejection<Frame<TypeSlot>>>
    where
        I: for<'m> Verify<ModuleVerifier<'m>>,
    {
        Self::verify(module, VerifyOptions::default()).map(|(verified, _)| verified)
    }

    /// Verify `module` with the standard [`ModuleVerifier`], returning the
    /// verified module with its [`Verification`] (warnings and, if
    /// requested, type maps).
    ///
    /// # Errors
    ///
    /// The [`Rejection`] listing every diagnostic (and type maps, if
    /// requested), if any diagnostic is an error.
    pub fn verify(
        module: LocalModule<I, Value, Type, Info>,
        options: VerifyOptions,
    ) -> Result<(Self, Verification<Frame<TypeSlot>>), Rejection<Frame<TypeSlot>>>
    where
        I: for<'m> Verify<ModuleVerifier<'m>>,
    {
        let verification = ModuleVerifier::new(&module).verify(&module.code, options)?;
        Ok((Self { module }, verification))
    }

    /// Verify `module` with a [`WithState`] context, for machines whose
    /// components add their own [`ComponentState`]; every function starts
    /// with component state `entry`.
    ///
    /// # Errors
    ///
    /// The [`Rejection`] listing every diagnostic, if any is an error.
    #[allow(clippy::type_complexity)]
    pub fn verify_with_state<S: ComponentState>(
        module: LocalModule<I, Value, Type, Info>,
        entry: S,
        options: VerifyOptions,
    ) -> Result<(Self, Verification<(Frame<TypeSlot>, S)>), Rejection<(Frame<TypeSlot>, S)>>
    where
        I: for<'m> Verify<WithState<'m, S>>,
    {
        let verification = ModuleVerifier::new(&module)
            .with_state(entry)
            .verify(&module.code, options)?;
        Ok((Self { module }, verification))
    }
}

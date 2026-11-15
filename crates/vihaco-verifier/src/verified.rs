// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! [`Verified<M>`]: proof that a module passed verification, and
//! [`VerifyFailed`], which gives a rejected module back.

use std::fmt;
use std::ops::Deref;

use vihaco_abi::{Type, Value, Verify};
use vihaco_module::module::LocalModule;

use crate::diagnostic::DiagnosticNames;
use crate::engine::{Rejection, Verification, VerifyOptions};
use crate::module::{ComponentState, ModuleVerifier, WithState};
use crate::slot::TypeSlot;
use crate::stack::Frame;

/// A module that passed load-time verification.
///
/// The ways to build one are verifying a module ([`Verified::new`],
/// [`Verified::verify`], [`Verified::verify_with_state`], or
/// [`Verified::verify_with`] for a hand-written context) and the explicit
/// escape hatch [`Verified::assume_verified`]. Executable images take a
/// `Verified` module (the stdlib `Image::new`, and through it `Loadable`), so
/// a program cannot run **through an `Image` or `Loadable`** without either
/// passing verification or a greppable `assume_verified` at its construction
/// site. The stdlib's low-level runner (`execution_state::execute` and
/// `resume`) takes a plain module and does not verify it.
///
/// `Verified` gives shared access to the module through [`Deref`] and
/// [`get`](Verified::get), but no mutable access: changing the module could
/// invalidate the verification. [`into_inner`](Verified::into_inner) gives
/// the module back, unverified. A rejected module comes back too, inside the
/// [`VerifyFailed`] error.
///
/// ```
/// use vihaco_module::module::{FunctionInfo, LocalModule, Signature};
/// use vihaco_verifier::{Flow, Type, TypedStack, Value, Verified, Verify, stack_effect};
///
/// #[derive(Debug)]
/// enum Inst {
///     ConstI64,
///     Halt,
/// }
///
/// // Diagnostics show the instruction at each location. (Composites
/// // generate this from their patterns.)
/// impl vihaco_abi::names::InstructionText for Inst {
///     fn write_text(
///         &self,
///         _: &dyn vihaco_abi::names::SymbolNames,
///         f: &mut std::fmt::Formatter<'_>,
///     ) -> std::fmt::Result {
///         f.write_str(match self {
///             Inst::ConstI64 => "const_i64",
///             Inst::Halt => "halt",
///         })
///     }
/// }
///
/// impl<C: TypedStack + ?Sized> Verify<C> for Inst {
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
///     strings: vec!["main".into()],
///     main_function: Some(0),
///     ..Default::default()
/// };
/// let verified = Verified::new(module)?;
/// assert_eq!(verified.code.len(), 2);
///
/// // A rejection gives the module back, with every diagnostic.
/// let mut module = verified.into_inner();
/// module.code.pop();
/// module.functions[0].end_address = 1;
/// let failed = Verified::new(module).unwrap_err();
/// assert_eq!(
///     failed.to_string(),
///     "verification failed with 1 error\n\
///      error: control falls off the end of @main without a return or halt\n  \
///      --> @main+0 (address 0): const_i64"
/// );
/// let module = failed.into_module();
/// assert_eq!(module.code.len(), 1);
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
    /// deliberately ill-typed programs, and for modules produced by a trusted
    /// toolchain that already verified them. (Hand-written verifier contexts
    /// and other module models use [`verify_with`](Verified::verify_with)
    /// instead.) Running an unverified module is memory-safe, since the
    /// runtime still checks heap tags, bounds and signatures, but errors the
    /// verifier would reject at load time surface during execution instead.
    pub fn assume_verified(module: M) -> Self {
        Self { module }
    }

    /// Verify `module` with a verification of your own: `verify` builds a
    /// context from the module, typically a hand-written one wrapping
    /// [`ModuleVerifier`] or a context for another module model, and runs it
    /// (for example with [`verify_functions`](crate::verify_functions)).
    ///
    /// The module is marked verified exactly when `verify` returns a
    /// [`Verification`]; the guarantee is as strong as the context `verify`
    /// uses. The standard constructors ([`new`](Verified::new),
    /// [`verify`](Verified::verify),
    /// [`verify_with_state`](Verified::verify_with_state)) are this with the
    /// standard contexts. Snapshots must not borrow the module, since it is
    /// moved into the result.
    ///
    /// ```
    /// use vihaco_module::module::LocalModule;
    /// use vihaco_verifier::{
    ///     Frame, FrameState, FunctionSpan, TypeSlot, Verified, VerifyOptions, verify_functions,
    /// };
    ///
    /// /// A module model the standard verifier does not cover: bare code.
    /// struct Bare(Vec<Halt>);
    /// struct Halt;
    /// impl<C: ?Sized> vihaco_verifier::Verify<C> for Halt {
    ///     fn flow(&self) -> vihaco_verifier::Flow {
    ///         vihaco_verifier::Flow::Halt
    ///     }
    ///     fn verify(&self, _: &mut C) -> eyre::Result<()> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let (verified, verification) = Verified::verify_with(Bare(vec![Halt]), |bare| {
    ///     // The context: one function over all of the code, a standard frame.
    ///     let entry = Frame::<TypeSlot>::new(vec![]);
    ///     let span = FunctionSpan { function: 0, start: 0, end: bare.0.len() as u32, entry };
    ///     let mut cx = FrameState::new(Frame::new(vec![]));
    ///     verify_functions(&bare.0, &[span], &mut cx, VerifyOptions::default())
    /// })
    /// .map_err(|failed| failed.rejection)?;
    /// assert!(verification.warnings.is_empty());
    /// assert_eq!(verified.0.len(), 1);
    /// # Ok::<(), vihaco_verifier::Rejection<Frame<TypeSlot>>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`VerifyFailed`] with the module and the [`Rejection`] `verify`
    /// returned.
    #[allow(clippy::type_complexity)]
    pub fn verify_with<S>(
        module: M,
        verify: impl FnOnce(&M) -> Result<Verification<S>, Rejection<S>>,
    ) -> Result<(Self, Verification<S>), VerifyFailed<M, S>> {
        match verify(&module) {
            Ok(verification) => Ok((Self { module }, verification)),
            Err(rejection) => Err(VerifyFailed {
                module: Box::new(module),
                rejection,
            }),
        }
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
    /// [`VerifyFailed`], holding the module and the [`Rejection`] listing
    /// every diagnostic, if any is an error. It displays the diagnostics with
    /// the module's names and converts into an [`eyre::Report`].
    pub fn new(
        module: LocalModule<I, Value, Type, Info>,
    ) -> Result<Self, VerifyFailed<LocalModule<I, Value, Type, Info>>>
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
    /// [`VerifyFailed`] with the module and the [`Rejection`] (and type maps,
    /// if requested), if any diagnostic is an error.
    #[allow(clippy::type_complexity)]
    pub fn verify(
        module: LocalModule<I, Value, Type, Info>,
        options: VerifyOptions,
    ) -> Result<
        (Self, Verification<Frame<TypeSlot>>),
        VerifyFailed<LocalModule<I, Value, Type, Info>>,
    >
    where
        I: for<'m> Verify<ModuleVerifier<'m>>,
    {
        Self::verify_with(module, |module| {
            ModuleVerifier::new(module).verify(&module.code, options)
        })
    }

    /// Verify `module` with a [`WithState`] context, for machines whose
    /// components add their own [`ComponentState`]; every function starts
    /// with component state `entry`.
    ///
    /// # Errors
    ///
    /// [`VerifyFailed`] with the module and the [`Rejection`], if any
    /// diagnostic is an error.
    #[allow(clippy::type_complexity)]
    pub fn verify_with_state<S: ComponentState>(
        module: LocalModule<I, Value, Type, Info>,
        entry: S,
        options: VerifyOptions,
    ) -> Result<
        (Self, Verification<(Frame<TypeSlot>, S)>),
        VerifyFailed<LocalModule<I, Value, Type, Info>, (Frame<TypeSlot>, S)>,
    >
    where
        I: for<'m> Verify<WithState<'m, S>>,
    {
        Self::verify_with(module, |module| {
            ModuleVerifier::new(module)
                .with_state(entry)
                .verify(&module.code, options)
        })
    }
}

/// A module that failed verification, given back with the [`Rejection`].
///
/// Displays every diagnostic with the module's names (see
/// [`Rejection::display_with`]) when the module provides them, which every
/// [`LocalModule`] whose instructions implement
/// [`InstructionText`](vihaco_abi::names::InstructionText) does (every
/// `#[composite]` instruction set), and then converts into an
/// [`eyre::Report`] with `?`.
pub struct VerifyFailed<M, S = Frame<TypeSlot>> {
    /// The module, unchanged (boxed, so the error stays small).
    pub module: Box<M>,
    /// Why it was rejected.
    pub rejection: Rejection<S>,
}

impl<M, S> VerifyFailed<M, S> {
    /// Give the module back, dropping the rejection.
    pub fn into_module(self) -> M {
        *self.module
    }

    /// The module and the rejection.
    pub fn into_parts(self) -> (M, Rejection<S>) {
        (*self.module, self.rejection)
    }
}

/// Shows the rejection; the module is omitted.
impl<M, S: fmt::Debug> fmt::Debug for VerifyFailed<M, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifyFailed")
            .field("rejection", &self.rejection)
            .finish_non_exhaustive()
    }
}

impl<M: DiagnosticNames, S> fmt::Display for VerifyFailed<M, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.rejection.display_with(&*self.module).fmt(f)
    }
}

impl<M: DiagnosticNames, S: fmt::Debug> std::error::Error for VerifyFailed<M, S> {}

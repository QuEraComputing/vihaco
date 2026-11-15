// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Program data owned beside a dispatchable machine.
//!
//! Construct a module explicitly by parsing and lowering SST or decoding bytecode,
//! verify it ([`Verified::new`]), then wrap it with [`Image::new`].
//! [`crate::Loadable`] pairs the image with a composite in separate fields so
//! execution borrows code and metadata while independently mutating the machine.
//!
//! An image only accepts a [`Verified`] module, so every program a
//! [`crate::Loadable`] runs has passed load-time verification, or was marked
//! verified explicitly with [`Verified::assume_verified`] (for tests and
//! trusted toolchains).
//!
//! Keep the image outside mutable Dispatch and unchanged during an invocation.
//! `Loadable` rejects replacement while an invocation is active and resets
//! execution state after successful replacement. Resetting execution does not
//! reset devices or other independently loaded programs. Image construction is
//! outside the owner; failures do not roll back caller mutations.

use vihaco_abi::{Type, Value};
use vihaco_module::module::{LocalModule, NoInfo};
use vihaco_verifier::Verified;

/// Verified code and metadata for one independently loadable unit.
///
/// The outer owner controls replacement. This type contains no PC, stack,
/// invocation state, or instruction inventory. `ProgramImage` remains the
/// separate legacy loader/host API.
#[derive(Debug, Clone)]
pub struct Image<I, V = Value, Ty = Type, Info = NoInfo> {
    module: Verified<LocalModule<I, V, Ty, Info>>,
}

impl<I, V, Ty, Info> Image<I, V, Ty, Info> {
    /// Wrap a verified module.
    ///
    /// Verify a module with [`Verified::new`] (or [`Verified::verify`] for
    /// warnings and type maps); [`Verified::assume_verified`] skips
    /// verification explicitly.
    pub fn new(module: Verified<LocalModule<I, V, Ty, Info>>) -> Self {
        Self { module }
    }

    /// The image's module. It is read-only: changing a verified module could
    /// invalidate its verification.
    pub fn module(&self) -> &LocalModule<I, V, Ty, Info> {
        self.module.get()
    }

    /// Consume the image, returning its verified module.
    pub fn into_module(self) -> Verified<LocalModule<I, V, Ty, Info>> {
        self.module
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Program data owned beside a dispatchable machine.
//!
//! An authored loadable owner stores an `Image` and its composite in separate
//! fields. Loading returns a module from the composite's own-section hook; the
//! owner installs that module here after generated child loading succeeds.
//! Execution borrows `image.module` and the machine independently. Nested
//! loadable owners repeat the same arrangement with their own instruction types.
//!
//! Keep the image outside mutable Dispatch and unchanged during an invocation.
//! Reject replacement while execution is active, or explicitly reset that
//! invocation first. After successful replacement, reset its ExecutionState
//! before entering the new program. Resetting execution need not reset devices
//! or independently loaded children. Resume always uses the paired, unchanged
//! image. Loading failures retain earlier loader/child mutations; they do not
//! imply rollback of an entire loadable tree.

use vihaco_abi::{Type, Value};
use vihaco_module::module::{LocalModule, NoInfo};

/// Code and metadata for one independently loadable unit.
///
/// The outer owner controls replacement. This type contains no PC, stack,
/// invocation state, or instruction inventory. `ProgramImage` remains the
/// separate legacy loader/host API.
#[derive(Debug, Clone)]
pub struct Image<I, V = Value, Ty = Type, Info = NoInfo> {
    pub module: LocalModule<I, V, Ty, Info>,
}

impl<I, V, Ty, Info> Image<I, V, Ty, Info> {
    pub fn new(module: LocalModule<I, V, Ty, Info>) -> Self {
        Self { module }
    }
}

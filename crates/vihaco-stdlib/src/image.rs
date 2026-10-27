// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Program data owned beside a dispatchable machine.
//!
//! Construct a module explicitly by parsing and lowering SST or decoding bytecode,
//! then wrap it with [`Image::new`]. [`crate::Loadable`] pairs the image with a
//! composite in separate fields so execution borrows code and metadata while
//! independently mutating the machine.
//!
//! Keep the image outside mutable Dispatch and unchanged during an invocation.
//! `Loadable` rejects replacement while an invocation is active and resets
//! execution state after successful replacement. Resetting execution does not
//! reset devices or other independently loaded programs. Image construction is
//! outside the owner; failures do not roll back caller mutations.

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

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::program::ProgramContext;

/// The global context for a given bytecode file.
///
/// This should include all context needed for an entire section tree.
/// Anything that should be shared across machines should be in a
/// [`BytecodeContext`].
pub trait BytecodeContext: Sized {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self>;

    fn from_text(text: &str) -> eyre::Result<Self>;

    fn section_name(&self, index: u32) -> Option<&str>;
}

/// The public handle for a bytecode context.
///
/// To avoid needing explicit lifetimes permeating throughout
/// machine definitions, we wrap the context in an [`Arc`] to drop it
/// automatically.
#[derive(Debug)]
pub struct ContextHandle<C = ProgramContext>(Arc<C>);

impl<C> Clone for ContextHandle<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C> ContextHandle<C> {
    pub fn new(context: C) -> Self {
        Self(Arc::new(context))
    }

    pub fn get(&self) -> &C {
        &self.0
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<C> std::ops::Deref for ContextHandle<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

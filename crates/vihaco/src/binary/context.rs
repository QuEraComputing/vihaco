// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::program::ProgramContext;

/// Marker context for SST files that do not carry global metadata.
///
/// `NoHeader` accepts only an empty `.global:` section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoHeader;

/// Resolve section-name indexes stored in bytecode files.
///
/// Bytecode child section entries store section names indirectly, so parsing a
/// bytecode file needs a context object that can resolve those indexes.
pub trait SectionNameResolver {
    fn section_name(&self, index: u32) -> Option<&str>;
}

/// A global context that can be decoded from bytecode.
pub trait BytecodeGlobalContext: Sized + SectionNameResolver {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self>;
}

/// A global context that can be parsed from SST text.
pub trait SstGlobalContext: Sized {
    fn from_text(text: &str) -> eyre::Result<Self>;
}

/// A global context that supports both bytecode and SST representations.
pub trait GlobalContext: BytecodeGlobalContext + SstGlobalContext {}

impl<T> GlobalContext for T where T: BytecodeGlobalContext + SstGlobalContext {}

impl SstGlobalContext for NoHeader {
    fn from_text(text: &str) -> eyre::Result<Self> {
        if text.trim().is_empty() {
            Ok(Self)
        } else {
            Err(eyre::eyre!("NoHeader accepts only an empty global section"))
        }
    }
}

/// The public handle for a global context.
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

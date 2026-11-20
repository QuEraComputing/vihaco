// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

pub use vihaco_module::module::Symbol;

/// One parsed SST section: its header and its flat `.text` stream.
///
/// `I` is the consumer's instruction syntax type and `H` its section-header
/// type. `.text` is not structured: it is one instruction stream in which
/// `@name:` lines bind symbols to addresses without occupying a slot. What a
/// symbol *is* (for example a function's signature) lives in the header.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, H> {
    pub header: H,
    /// Instructions in address order.
    pub code: Vec<I>,
    /// Symbol definitions in source order. Each address is the index of the
    /// next instruction in [`code`](Self::code), possibly `code.len()`.
    pub symbols: Vec<Symbol>,
}

impl<I, H> ParsedModule<I, H> {
    /// Check function declarations and the symbol table against supplied
    /// metadata without changing either.
    pub fn validate_metadata<V, Ty: Clone, Info>(
        &self,
        metadata: &vihaco_module::module::ProgramMetadata<V, Ty, Info>,
    ) -> eyre::Result<()> {
        metadata.validate(&self.symbols, self.code.len())
    }
}

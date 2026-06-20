// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The `Resolve` trait — bridge between [`super::ParsedModule`] and the
//! runtime [`crate::module::Module`]. Each consumer implements this for its
//! own instruction set + header type, holding whatever state is needed
//! (label table, string interner, sugar expansion rules).

use crate::syntax::{BodyItem, ParsedModule};

/// Lower a parsed module to its resolved runtime form.
///
/// Implementations own the symbol table / string interner / sugar expansion
/// rules. Default behavior for any `BodyItem::Raw` is to error; consumers
/// override [`Resolve::resolve_body`] to handle their sugar/symbolic forms.
pub trait Resolve<I, H> {
    /// Resolved module type — concrete to the consumer (typically
    /// `crate::module::Module<I, Value, Type, Info>` with consumer-specific
    /// `Info`).
    type Module;

    fn resolve_module(&mut self, parsed: ParsedModule<I, H>) -> eyre::Result<Self::Module>;

    /// Convenience hook: lower a single function body. Default just walks
    /// `Direct` items and errors on `Raw`. Override to handle sugar/symbolic
    /// forms.
    fn resolve_body(&mut self, items: Vec<BodyItem<I>>) -> eyre::Result<Vec<I>> {
        items
            .into_iter()
            .map(|item| match item {
                BodyItem::Direct(inst) => Ok(inst),
                BodyItem::Raw(raw) => Err(eyre::eyre!(
                    "unhandled raw form `{}` (consumer's Resolve impl must override `resolve_body`)",
                    raw.mnemonic
                )),
            })
            .collect()
    }
}

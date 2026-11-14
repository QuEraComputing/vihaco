// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-set-independent lowering of a parsed `.text` stream.

use crate::{ParsedModule, Symbol};
use eyre::WrapErr;
use vihaco_abi::{Lower, Value};
use vihaco_module::loader::InternedStrings;
use vihaco_module::module::{LocalModule, ProgramMetadata};

/// Describe an address relative to the nearest preceding symbol, e.g. `@main+3`.
fn locate(symbols: &[Symbol], address: usize) -> String {
    symbols
        .iter()
        .filter(|symbol| symbol.address as usize <= address)
        .max_by_key(|symbol| symbol.address)
        .map(|symbol| match address - symbol.address as usize {
            0 => format!("@{}", symbol.name),
            offset => format!("@{}+{offset}", symbol.name),
        })
        .unwrap_or_else(|| format!("address {address}"))
}

/// Lower a code stream in address order.
///
/// Each syntax instruction lowers to one runtime instruction. The context must
/// already know the whole symbol table, so forward references resolve. Errors
/// name the instruction by address and nearest preceding symbol; no partial
/// code vector is returned. Context mutations before an error are not rolled
/// back.
pub fn lower_code<I, C>(
    code: Vec<I>,
    symbols: &[Symbol],
    context: &mut C,
) -> eyre::Result<Vec<I::Runtime>>
where
    I: Lower<C>,
{
    code.into_iter()
        .enumerate()
        .map(|(address, instruction)| {
            instruction.lower(context).wrap_err_with(|| {
                format!(
                    "lowering instruction {address} ({})",
                    locate(symbols, address)
                )
            })
        })
        .collect()
}

/// Validate supplied metadata and lower a code stream without consuming it.
///
/// Available with `program-image-execution`. The caller prepares the metadata
/// and context independently, for example by processing individual header
/// declarations. The context may borrow prepared configuration or the metadata
/// itself; neither is moved or cloned by this function.
///
/// Function declarations, the symbol table, and table bounds are validated
/// before lowering starts. Lowering may intern string literals in the
/// context; store the context's completed string table in the metadata
/// (`metadata.strings = context.into_strings()` for
/// `MetadataLoweringContext`) and assemble the returned code with
/// [`ProgramMetadata::into_module`], which also checks standard value
/// references.
///
/// # Errors
/// Returns declaration, symbol, table, or instruction-lowering errors. No
/// partial code is returned; earlier context mutations are not rolled back.
#[cfg(feature = "program-image-execution")]
pub fn lower_code_with_metadata<I, Ty, V, Info, C>(
    code: Vec<I>,
    symbols: &[Symbol],
    metadata: &ProgramMetadata<V, Ty, Info>,
    context: &mut C,
) -> eyre::Result<Vec<I::Runtime>>
where
    Ty: Clone,
    I: Lower<C>,
{
    metadata.validate(symbols, code.len())?;
    lower_code(code, symbols, context)
}

impl<I, H> ParsedModule<I, H> {
    /// Validate declarations, symbols, and tables, lower instructions with the
    /// supplied context, and assemble a module without inventing or reordering
    /// metadata. The module's string table is the context's: the metadata's
    /// strings followed by those interned during lowering.
    ///
    /// The consumer extracts the metadata from its chosen header schema and
    /// prepares section-wide context capabilities (for example
    /// `MetadataLoweringContext::new(&metadata, &parsed.symbols)`) before
    /// calling this method. The header itself is not interpreted here.
    ///
    /// # Errors
    /// Returns declaration, symbol, table, or lowering errors. Context
    /// mutations before an error are retained; no partial module is returned.
    pub fn lower_with_metadata<C, Ty: Clone, Info>(
        self,
        metadata: ProgramMetadata<Value, Ty, Info>,
        context: &mut C,
    ) -> eyre::Result<LocalModule<I::Runtime, Value, Ty, Info>>
    where
        I: Lower<C>,
        C: InternedStrings,
    {
        metadata.validate(&self.symbols, self.code.len())?;
        let code = lower_code(self.code, &self.symbols, context)?;
        let mut metadata = metadata;
        metadata.strings = context.take_strings();
        metadata.into_module(code, self.symbols)
    }
}

#[cfg(test)]
mod tests;

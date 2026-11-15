// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-set-independent lowering of parsed function bodies.

use crate::{ParsedFunction, ParsedModule};
use eyre::WrapErr;
use vihaco_abi::{Lower, Value};
use vihaco_module::loader::FunctionScope;
use vihaco_module::module::{LocalModule, ProgramMetadata, ResolveType, StructTypeRef};

/// Lower functions in source order, preparing each function's complete label
/// scope before lowering its first instruction.
///
/// Each syntax instruction lowers to one runtime instruction. Errors include
/// function and instruction context; no partial code vector is returned.
/// Context mutations before an error are not rolled back.
pub fn lower_functions<I, Ty, C>(
    functions: Vec<ParsedFunction<I, Ty>>,
    context: &mut C,
) -> eyre::Result<Vec<I::Runtime>>
where
    I: Lower<C>,
    C: FunctionScope,
{
    let mut code = Vec::new();
    for function in functions {
        let name = function.name.as_str();
        #[cfg(feature = "typed-declarations")]
        let labels = function
            .labels
            .iter()
            .map(|label| (label.name.as_str(), label.instruction_index));
        #[cfg(not(feature = "typed-declarations"))]
        let labels = std::iter::empty();
        context.enter_function(name, labels)?;
        for (offset, instruction) in function.body.into_iter().enumerate() {
            let address = code.len();
            // The same `@name+offset (address N)` form verifier diagnostics use.
            code.push(
                instruction
                    .lower(context)
                    .wrap_err_with(|| format!("lowering @{name}+{offset} (address {address})"))?,
            );
        }
    }
    Ok(code)
}

fn validate_program_functions<I, S, Ty, V, Info>(
    functions: &[ParsedFunction<I, S>],
    metadata: &ProgramMetadata<V, Ty, Info>,
) -> eyre::Result<()>
where
    S: ResolveType<Ty>,
    Ty: PartialEq + Clone + std::fmt::Display,
{
    crate::types::validate_functions(functions, metadata)?;
    metadata.validate(functions.iter().map(|function| function.body.len()).sum())
}

/// Validate supplied metadata and lower function bodies without consuming it.
///
/// Available with `program-image-execution`. The caller prepares the metadata
/// and context independently, for example by processing individual header
/// declarations. The context may borrow prepared configuration or the metadata
/// itself; neither is moved or cloned by this function.
///
/// Function declarations and table bounds are validated before lowering starts.
/// [`lower_functions`] prepares label scopes and preserves instruction error
/// context. The returned code can be assembled with
/// [`ProgramMetadata::into_module`], which also checks standard value references.
///
/// Function headers may be parsed with a source type such as
/// [`SourceType`](vihaco_module::module::SourceType), naming structs and named
/// types (`fn @f(p: struct @Point)`); they are resolved against the metadata's
/// tables before comparison (see [`ResolveType`]).
///
/// # Errors
/// Returns declaration, table, or instruction-lowering errors. No partial code
/// is returned; earlier context mutations are not rolled back.
#[cfg(feature = "program-image-execution")]
pub fn lower_functions_with_metadata<I, S, Ty, V, Info, C>(
    functions: Vec<ParsedFunction<I, S>>,
    metadata: &ProgramMetadata<V, Ty, Info>,
    context: &mut C,
) -> eyre::Result<Vec<I::Runtime>>
where
    S: ResolveType<Ty>,
    Ty: PartialEq + Clone + std::fmt::Display,
    I: Lower<C>,
    C: FunctionScope,
{
    validate_program_functions(&functions, metadata)?;
    lower_functions(functions, context)
}

impl<I, Ty: PartialEq + Clone + std::fmt::Display + StructTypeRef, H> ParsedModule<I, Ty, H> {
    /// Validate declarations and tables, lower instructions with the supplied
    /// context, and assemble a module without inventing or reordering metadata.
    ///
    /// The consumer extracts the metadata from its chosen header schema and
    /// prepares section-wide context capabilities before calling this method.
    /// [`lower_functions`] prepares function-local scopes. Standard CPU symbol
    /// capabilities are provided by `MetadataLoweringContext` in `vihaco-module`.
    /// Additional capabilities can be supplied by a consumer-owned context.
    /// The header itself is not interpreted by this method.
    ///
    /// # Errors
    /// Returns declaration, table, or lowering errors. Context mutations before
    /// an error are retained; no partial module is returned.
    pub fn lower_with_metadata<C, Info>(
        self,
        metadata: ProgramMetadata<Value, Ty, Info>,
        context: &mut C,
    ) -> eyre::Result<LocalModule<I::Runtime, Value, Ty, Info>>
    where
        I: Lower<C>,
        C: FunctionScope,
    {
        validate_program_functions(&self.functions, &metadata)?;
        let code = lower_functions(self.functions, context)?;
        metadata.into_module(code)
    }
}

#[cfg(test)]
mod tests;

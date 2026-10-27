// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-set-independent lowering of parsed function bodies.

use crate::{ParsedFunction, ParsedModule};
use eyre::WrapErr;
use vihaco_abi::{Lower, Value};
use vihaco_module::loader::FunctionScope;
use vihaco_module::module::{LocalModule, ProgramMetadata};

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
        for (index, instruction) in function.body.into_iter().enumerate() {
            code.push(
                instruction
                    .lower(context)
                    .wrap_err_with(|| format!("lowering function @{name}, instruction {index}"))?,
            );
        }
    }
    Ok(code)
}

fn validate_program_functions<I, Ty: PartialEq, V, Info>(
    functions: &[ParsedFunction<I, Ty>],
    metadata: &ProgramMetadata<V, Ty, Info>,
) -> eyre::Result<()> {
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
/// # Errors
/// Returns declaration, table, or instruction-lowering errors. No partial code
/// is returned; earlier context mutations are not rolled back.
#[cfg(feature = "program-image-execution")]
pub fn lower_functions_with_metadata<I, Ty, V, Info, C>(
    functions: Vec<ParsedFunction<I, Ty>>,
    metadata: &ProgramMetadata<V, Ty, Info>,
    context: &mut C,
) -> eyre::Result<Vec<I::Runtime>>
where
    Ty: PartialEq,
    I: Lower<C>,
    C: FunctionScope,
{
    validate_program_functions(&functions, metadata)?;
    lower_functions(functions, context)
}

impl<I, Ty: PartialEq, H> ParsedModule<I, Ty, H> {
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

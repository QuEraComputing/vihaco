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
        self.validate_metadata(&metadata)?;
        metadata.validate(
            self.functions
                .iter()
                .map(|function| function.body.len())
                .sum(),
        )?;
        let code = lower_functions(self.functions, context)?;
        metadata.into_module(code)
    }
}

#[cfg(test)]
mod tests;

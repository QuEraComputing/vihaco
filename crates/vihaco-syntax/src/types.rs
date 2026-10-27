// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

use vihaco_parser::Ident;

/// Parsed SST module containing consumer instruction values.
///
/// `I` is the consumer's instruction syntax type, `Ty` is its source type
/// syntax, and `H` is its section-header type.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, Ty, H> {
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I, Ty> {
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: Ident,
    /// Function parameters parsed from `name: type` declarations.
    pub params: Vec<Param<Ty>>,
    /// Return types parsed with the consumer-provided `Ty` syntax.
    pub return_types: Vec<Ty>,
    pub body: Vec<I>,
    /// Labels and their positions in the executable body, for loader preparation.
    #[cfg(feature = "typed-declarations")]
    pub labels: Vec<ParsedLabel>,
}

/// A function-local label that occupies no executable instruction slot.
#[cfg(feature = "typed-declarations")]
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLabel {
    pub name: Ident,
    /// Index of the next instruction in this function, possibly `body.len()`.
    pub instruction_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param<Ty> {
    pub name: Ident,
    pub ty: Ty,
}

impl<I, Ty: PartialEq, H> ParsedModule<I, Ty, H> {
    /// Check `fn` declarations against supplied metadata without changing either.
    /// Table bounds and runtime constant references are checked during assembly.
    pub fn validate_metadata<V, Info>(
        &self,
        metadata: &vihaco_module::module::ProgramMetadata<V, Ty, Info>,
    ) -> eyre::Result<()> {
        eyre::ensure!(
            self.functions.len() == metadata.functions.len(),
            "function declarations and metadata must contain the same functions"
        );
        let mut seen = std::collections::BTreeSet::new();
        let mut address = 0usize;
        for function in &self.functions {
            let name = function.name.as_str();
            eyre::ensure!(
                seen.insert(name),
                "duplicate function declaration `@{name}`"
            );
            let mut matches = metadata.functions.iter().filter(|info| {
                metadata
                    .strings
                    .get(info.name as usize)
                    .is_some_and(|value| value == name)
            });
            let info = matches
                .next()
                .ok_or_else(|| eyre::eyre!("function `@{name}` has no metadata"))?;
            eyre::ensure!(
                matches.next().is_none(),
                "function `@{name}` has duplicate metadata"
            );
            let end = address
                .checked_add(function.body.len())
                .ok_or_else(|| eyre::eyre!("instruction count overflow"))?;
            eyre::ensure!(
                info.start_address as usize == address && info.end_address as usize == end,
                "function `@{name}`: supplied range {}..{} disagrees with body range {address}..{end}",
                info.start_address,
                info.end_address
            );
            eyre::ensure!(
                function.params.len() == info.signature.params.len(),
                "function `@{name}`: parameter count disagrees with metadata"
            );
            for (parameter, declared) in function.params.iter().zip(&info.signature.params) {
                eyre::ensure!(
                    metadata
                        .strings
                        .get(declared.name as usize)
                        .is_some_and(|value| value == parameter.name.as_str())
                        && parameter.ty == declared.ty,
                    "function `@{name}`: parameter `{}` disagrees with metadata",
                    parameter.name
                );
            }
            eyre::ensure!(
                function.return_types == info.signature.ret,
                "function `@{name}`: return types disagree with metadata"
            );
            #[cfg(feature = "typed-declarations")]
            {
                let mut labels = std::collections::BTreeSet::new();
                for label in &function.labels {
                    eyre::ensure!(
                        labels.insert(label.name.as_str()),
                        "function `@{name}`: duplicate label `@{}`",
                        label.name
                    );
                    eyre::ensure!(
                        label.instruction_index <= function.body.len(),
                        "function `@{name}`: label `@{}` is outside its body",
                        label.name
                    );
                    let label_address = address + label.instruction_index;
                    eyre::ensure!(
                        metadata.labels.iter().any(|info| {
                            metadata
                                .strings
                                .get(info.name as usize)
                                .is_some_and(|value| value == label.name.as_str())
                                && info.address as usize == label_address
                        }),
                        "function `@{name}`: label `@{}` at {label_address} requires matching metadata",
                        label.name
                    );
                }
            }
            address = end;
        }
        Ok(())
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Section headers as collections of consumer-defined declarations.

use chumsky::prelude::*;
use vihaco_abi::traits::FromText;
use vihaco_bytecode::SstHeader;
use vihaco_parser::Parse;

/// Semicolon-terminated declarations parsed from one section header.
///
/// Available with `program-image-execution`. `D` selects the declarations that
/// the application accepts, such as device directives and program metadata.
/// Whitespace and line comments may separate declarations; empty headers are
/// valid. Consumers apply each declaration to their own preparation context
/// and decide which declarations are required and when to resolve dependencies.
/// This collection imposes no lowering or image-construction policy.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderDeclarations<D>(pub Vec<D>);

impl<D> FromText for HeaderDeclarations<D>
where
    for<'src> D: Parse<'src>,
{
    fn from_text(text: &str) -> eyre::Result<Self> {
        D::parser()
            .padded_by(crate::skip())
            .then_ignore(just(';').padded_by(crate::skip()))
            .repeated()
            .collect::<Vec<_>>()
            .then_ignore(crate::skip())
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map(Self)
            .map_err(|errors| eyre::eyre!("invalid header declarations: {errors:?}"))
    }
}

impl<D> SstHeader for HeaderDeclarations<D> where for<'src> D: Parse<'src> {}

impl<D> IntoIterator for HeaderDeclarations<D> {
    type Item = D;
    type IntoIter = std::vec::IntoIter<D>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests;

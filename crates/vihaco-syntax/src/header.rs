// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Section headers as collections of consumer-defined declarations.

use vihaco_abi::traits::FromText;
use vihaco_bytecode::SSTHeader;
use vihaco_parser::Parse;

/// Declarations parsed from one section header, one per line.
///
/// Available with `program-image-execution`. `D` selects the declarations that
/// the application accepts, such as device directives and program metadata.
/// Each declaration ends at the end of its line and continues onto the next
/// line only inside an open `(`, `[`, or `{`; there is no terminator. Blank
/// lines and `;` comments may separate declarations; empty headers are valid.
/// Consumers apply each declaration to their own preparation context
/// and decide which declarations are required and when to resolve dependencies.
/// This collection imposes no lowering or image-construction policy.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderDeclarations<D>(pub Vec<D>);

impl<D> FromText for HeaderDeclarations<D>
where
    for<'src> D: Parse<'src>,
{
    fn from_text(text: &str) -> eyre::Result<Self> {
        let parsed = crate::spanned::parse_declarations::<D>(text);
        if let Some(first) = parsed.errors.first() {
            let (line, column) = crate::spanned::line_column(text, first.span.start);
            eyre::bail!(
                "invalid header declarations: header line {line}, column {column}: {first}"
            );
        }
        Ok(Self(
            parsed
                .declarations
                .into_iter()
                .map(|declaration| declaration.value)
                .collect(),
        ))
    }
}

impl<D> SSTHeader for HeaderDeclarations<D> where for<'src> D: Parse<'src> {}

impl<D> IntoIterator for HeaderDeclarations<D> {
    type Item = D;
    type IntoIter = std::vec::IntoIter<D>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests;

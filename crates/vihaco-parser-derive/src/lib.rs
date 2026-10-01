// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate proc_macro;
mod attr;
mod codegen;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Parse, attributes(syntax_class, pattern))]
pub fn derive_parse(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    codegen::expand(input).unwrap_or_else(|error| error.into_compile_error().into())
}

/// Internal parser emitter used by typed dialect declarations.
#[doc(hidden)]
#[proc_macro]
pub fn __dialect_parsers(input: TokenStream) -> TokenStream {
    codegen::dialect_parsers(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

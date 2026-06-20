// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate proc_macro;
mod attr;
mod codegen;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Parse, attributes(head, token, delimiters, delegate, parse_with))]
pub fn derive_parse(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    codegen::expand(input).unwrap_or_else(|e| e.to_compile_error().into())
}

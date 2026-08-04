// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::common::resolve_root;

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let root = match resolve_root(&input.attrs) {
        Ok(root) => root,
        Err(err) => return err.into_compile_error().into(),
    };
    let ident = input.ident;
    quote! {
        impl #root::runtime::Message for #ident {}
    }
    .into()
}

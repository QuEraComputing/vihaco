// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Item, ItemStruct, Result};

use crate::common::{resolve_root, strip_vihaco_attrs};

mod coverage;
mod selector;

use coverage::expand_coverage_check;
use selector::instructions;

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let result = expand_inner(attr.into(), item.into());
    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand_inner(attr: TokenStream2, item: TokenStream2) -> Result<TokenStream2> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "`component` takes no arguments",
        ));
    }
    let parsed: Item = syn::parse2(item)?;
    let mut component: ItemStruct = match parsed {
        Item::Struct(item) => item,
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "`component` requires a struct",
            ));
        }
    };
    let root = resolve_root(&component.attrs)?;
    let selectors = instructions(&mut component.attrs)?;
    strip_vihaco_attrs(&mut component.attrs);

    let ident = &component.ident;
    let generics = &component.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let coverage_check = expand_coverage_check(&component, &root, &selectors);

    Ok(quote! {
        #component
        impl #impl_generics #root::Component for #ident #ty_generics #where_clause {}
        #coverage_check
    })
}

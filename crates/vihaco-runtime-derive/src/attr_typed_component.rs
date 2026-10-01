// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Whole-dialect support, coverage, and ordered component descriptions.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::{
    Attribute, ItemStruct, MacroDelimiter, Meta, Path, Result, Token, punctuated::Punctuated,
};

use crate::common::{resolve_root, strip_attrs, validate_module_path};

struct Component {
    item: ItemStruct,
    root: Path,
    dialects: Vec<Path>,
}

pub(crate) fn expand(options: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !options.is_empty() {
        return Err(syn::Error::new_spanned(
            options,
            "#[component] takes no arguments",
        ));
    }
    let declaration = parse(syn::parse2(item)?)?;
    Ok(emit(declaration))
}

fn parse(mut item: ItemStruct) -> Result<Component> {
    let root = resolve_root(&item.attrs)?;
    let mut helpers = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("instructions"));
    let helper = helpers.next().ok_or_else(|| {
        syn::Error::new(
            item.ident.span(),
            "#[component] requires #[instructions { dialects, ... }]",
        )
    })?;
    if let Some(repeated) = helpers.next() {
        return Err(syn::Error::new_spanned(
            repeated,
            "repeated #[instructions] helper",
        ));
    }
    let Meta::List(list) = &helper.meta else {
        return Err(syn::Error::new_spanned(
            helper,
            "expected #[instructions { dialects, ... }]",
        ));
    };
    if !matches!(list.delimiter, MacroDelimiter::Brace(_)) {
        return Err(syn::Error::new_spanned(
            helper,
            "expected #[instructions { dialects, ... }]",
        ));
    }
    let dialects = helper.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
    if dialects.is_empty() {
        return Err(syn::Error::new_spanned(
            helper,
            "component dialect list must not be empty",
        ));
    }
    let mut paths = HashSet::new();
    for path in &dialects {
        validate_module_path(path)?;
        if !paths.insert(path.to_token_stream().to_string()) {
            return Err(syn::Error::new_spanned(
                path,
                "duplicate component dialect path",
            ));
        }
    }
    let dialects = dialects.into_iter().collect();
    strip_attrs(&mut item.attrs, &["instructions", "vihaco"]);
    Ok(Component {
        item,
        root,
        dialects,
    })
}

// Only conditional item existence propagates to generated items. An authored
// cfg_attr may also contain derive/repr attributes that cannot apply to a const.
fn gates(attrs: &[Attribute]) -> Vec<Meta> {
    fn gate(meta: &Meta) -> Option<Meta> {
        if meta.path().is_ident("cfg") {
            return Some(meta.clone());
        }
        if let Meta::List(list) = meta
            && list.path.is_ident("cfg_attr")
        {
            let args = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()?;
            let condition = args.first()?;
            let nested: Vec<_> = args.iter().skip(1).filter_map(gate).collect();
            if !nested.is_empty() {
                return Some(syn::parse_quote!(cfg_attr(#condition, #(#nested),*)));
            }
        }
        None
    }
    attrs.iter().filter_map(|attr| gate(&attr.meta)).collect()
}

fn emit(component: Component) -> TokenStream {
    let Component {
        item,
        root,
        dialects,
    } = component;
    let name = &item.ident;
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    let gates = gates(&item.attrs);
    let identities: Vec<_> = dialects
        .iter()
        .map(|path| {
            // Name-resolution errors point at the generated marker. Give it
            // the authored module's span, including for qualified paths.
            let span = path
                .segments
                .last()
                .expect("nonempty dialect path")
                .ident
                .span();
            quote_spanned!(span=> #path::__VihacoDialect)
        })
        .collect();
    let support = identities.iter().map(|identity| {
        quote! {
            #(#[#gates])*
            impl #impl_generics #root::__private::Supports<#identity>
                for #name #type_generics #where_clause {}
        }
    });
    let description = quote! {
        #root::__private::Description::Component(&[
            #(<#identities as #root::__private::DialectSpec>::LEAVES),*
        ])
    };
    quote! {
        #item
        #(#[#gates])*
        const _: () = {
            // The function body is checked for every original generic parameter;
            // adding Coverage/Execute bounds here would conceal missing impls.
            #[allow(dead_code, clippy::multiple_bound_locations)]
            fn __vihaco_check_coverage #impl_generics () #where_clause {
                #(#root::__private::require_coverage::<#name #type_generics, #identities>();)*
            }
            #root::__private::validate(&#description);
        };
        #(#support)*
        #(#[#gates])*
        impl #impl_generics #root::Component for #name #type_generics #where_clause {}
        #(#[#gates])*
        impl #impl_generics #root::__private::Capabilities for #name #type_generics #where_clause {
            const DESCRIPTION: &'static #root::__private::Description = &#description;
        }
    }
}

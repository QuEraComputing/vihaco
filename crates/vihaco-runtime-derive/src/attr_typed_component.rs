// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Whole-dialect support, coverage, and ordered component descriptions.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::{ItemStruct, MacroDelimiter, Meta, Path, Result, Token, punctuated::Punctuated};

use crate::common::{gates, resolve_root, strip_attrs, validate_module_path};

struct Component {
    item: ItemStruct,
    root: Path,
    dialects: Vec<Path>,
}

/// Validate a typed component declaration and emit its support and inventory machinery.
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

/// Read the complete dialect list and consume the component-owned helper attributes.
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

/// Generate dialect support, eager coverage checks, capability metadata, and inventory queries.
fn emit(component: Component) -> TokenStream {
    let Component {
        item,
        root,
        dialects,
    } = component;
    let namespace_root = crate::common::root_in_child_module(&root);
    let name = &item.ident;
    let fields_module = crate::field::module_ident(name);
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
    let requests = dialects
        .iter()
        .map(|path| crate::inventory::Request {
            callback: syn::parse_quote!(#path::__vihaco_inventory::__enumerate),
            provider: syn::parse_quote!(#path::__VihacoDialect),
            prefix: vec![],
            field: None,
            codes: vec![],
        })
        .collect();
    let inventory = crate::inventory::start("component", &root, &item, requests);
    quote! {
        #item
        #(#[#gates])*
        #inventory
        #(#[#gates])*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #fields_module {
            #[allow(unused_imports)]
            use super::*;

            #[doc(hidden)]
            #[allow(unused_imports)]
            pub use #namespace_root::__private::FieldPath as __FieldPath;

            #[doc(hidden)]
            #[allow(unused_imports)]
            pub use #namespace_root::__private::CheckedRoute as __CheckedRoute;

            pub mod __markers {}

            pub mod __children {}
        }
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

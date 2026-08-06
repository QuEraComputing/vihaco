// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::{Field, GenericParam, Generics, Ident, Token};

/// Parse a comma-separated sequence of named fields.
pub fn parse_named_fields(input: ParseStream<'_>) -> syn::Result<Punctuated<Field, Token![,]>> {
    Punctuated::parse_terminated_with(input, Field::parse_named)
}

/// Retain only the generic parameters referenced by the supplied token streams.
pub fn retain_generics(generics: &Generics, references: &[TokenStream2]) -> Generics {
    let mut result = generics.clone();
    result.params = generics
        .params
        .iter()
        .filter(|param| {
            references
                .iter()
                .any(|tokens| generic_param_is_referenced(param, tokens))
        })
        .cloned()
        .collect();

    if let Some(where_clause) = &mut result.where_clause {
        where_clause.predicates = where_clause
            .predicates
            .iter()
            .filter(|predicate| {
                let tokens = quote!(#predicate);
                result
                    .params
                    .iter()
                    .any(|param| generic_param_is_referenced(param, &tokens))
            })
            .cloned()
            .collect();
        if where_clause.predicates.is_empty() {
            result.where_clause = None;
        }
    }
    result
}

fn generic_param_is_referenced(param: &GenericParam, tokens: &TokenStream2) -> bool {
    // TODO: improve: inspect the syntax tree instead of matching token strings.
    let tokens = tokens.to_string();
    match param {
        GenericParam::Type(param) => tokens.contains(&param.ident.to_string()),
        GenericParam::Const(param) => tokens.contains(&param.ident.to_string()),
        GenericParam::Lifetime(param) => tokens.contains(&param.lifetime.to_string()),
    }
}

/// Remove attributes consumed by this proc-macro crate before re-emitting an
/// item. Attribute macros cannot register helper attributes, so leaving one on
/// the output causes rustc to reject the expanded item.
pub fn strip_vihaco_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| !attr.path().is_ident("vihaco"));
}

/// Resolve the crate root that generated code should be rooted at.
///
/// A downstream crate might depend on the `vihaco` facade *or* directly on
/// `vihaco-runtime`; either way the generated `::<root>::…` paths must resolve.
/// We honour an explicit `#[vihaco(crate = ::some::path)]` override first, then
/// probe (via `proc-macro-crate`) for the facade and then the runtime crate,
/// emitting `crate` when the derive is used inside the runtime crate itself.
pub fn resolve_root(attrs: &[syn::Attribute]) -> syn::Result<TokenStream2> {
    if let Some(path) = crate_override(attrs)? {
        return Ok(quote! { #path });
    }

    for name in ["vihaco", "vihaco-runtime"] {
        if let Ok(found) = crate_name(name) {
            return Ok(match found {
                FoundCrate::Itself => quote! { crate },
                FoundCrate::Name(name) => {
                    let ident = Ident::new(&name, Span::call_site());
                    quote! { ::#ident }
                }
            });
        }
    }

    // Neither the facade nor the runtime crate is a visible dependency; fall
    // back to the runtime crate's canonical extern name so a clear resolution
    // error is produced rather than an opaque one.
    Ok(quote! { ::vihaco_runtime })
}

/// Parse an optional `#[vihaco(crate = ::path)]` override on the annotated item.
fn crate_override(attrs: &[syn::Attribute]) -> syn::Result<Option<syn::Path>> {
    let mut found = None;
    for attr in attrs {
        if !attr.path().is_ident("vihaco") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                found = Some(value.parse::<syn::Path>()?);
                Ok(())
            } else {
                Err(meta.error("unsupported vihaco attribute; expected `crate = <path>`"))
            }
        })?;
    }
    Ok(found)
}

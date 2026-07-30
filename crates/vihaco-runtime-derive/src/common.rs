// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::Ident;

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

    for name in ["vihaco-runtime", "vihaco"] {
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

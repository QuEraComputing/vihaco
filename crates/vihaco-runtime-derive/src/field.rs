// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Shared marker naming and declaration-path lookup.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{Ident, Path, Result};

/// Derive the reproducible companion namespace used to look up field markers.
pub(crate) fn module_ident(composite: &Ident) -> Ident {
    format_ident!(
        "__vihaco_fields_{}",
        composite.unraw(),
        span = composite.span()
    )
}

/// Resolve a field route into checked marker projections, including nested children.
pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let path: Path = syn::parse2(input)?;
    if path.leading_colon.is_some() || path.segments.len() < 2 {
        return Err(syn::Error::new_spanned(
            path,
            "expected Root::field followed by optional nested fields",
        ));
    }
    let mut segments = path.segments.iter();
    let parent = segments.next().expect("root segment");
    if parent.ident == "crate"
        || parent.ident == "self"
        || parent.ident == "super"
        || parent.ident == "Self"
    {
        return Err(syn::Error::new_spanned(
            parent,
            "the first identifier must name the composite declaration",
        ));
    }
    if matches!(parent.arguments, syn::PathArguments::Parenthesized(_)) {
        return Err(syn::Error::new_spanned(
            &parent.arguments,
            "expected a root type with optional generic arguments",
        ));
    }
    let module = module_ident(&parent.ident);
    let mut namespace = quote!(#module);
    let mut markers = Vec::new();
    for segment in segments {
        if !segment.arguments.is_none() {
            return Err(syn::Error::new_spanned(
                &segment.arguments,
                "field names cannot have generic arguments",
            ));
        }
        let field = &segment.ident;
        markers.push(quote!(#namespace::__markers::#field));
        namespace = quote!(#namespace::__children::#field);
    }
    let mut route = markers.pop().expect("at least one field");
    while let Some(head) = markers.pop() {
        route = quote!(#module::__FieldPath<#head, #route>);
    }
    Ok(quote!(<#parent as #module::__CheckedRoute<#route>>::Marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_marker_paths_without_name_encoding() {
        let checker = quote!(__vihaco_fields_Machine::__CheckedRoute);
        let marker = quote!(__vihaco_fields_Machine::__markers::r#type);
        assert_eq!(
            expand(quote!(Machine::r#type)).unwrap().to_string(),
            quote!(<Machine as #checker<#marker>>::Marker).to_string()
        );
        let nested = expand(quote!(Machine<T>::controller::cpu::a::b::c)).unwrap();
        let text = nested.to_string();
        assert_eq!(text.matches("FieldPath").count(), 4);
        assert!(text.contains("Machine < T > as __vihaco_fields_Machine :: __CheckedRoute"));
        assert!(text.contains("__children :: controller :: __children :: cpu"));
        syn::parse2::<syn::Type>(nested).unwrap();
        assert!(
            expand(quote!(r#type::Δ))
                .unwrap()
                .to_string()
                .contains("__vihaco_fields_type :: __markers :: Δ")
        );
    }
    #[test]
    fn rejects_invalid_roots_and_field_segments() {
        for input in [
            quote!(cpu),
            quote!(::Machine::cpu),
            quote!(self::cpu),
            quote!(Self::cpu),
            quote!(Machine::cpu<u8>),
            quote!(<T as Trait>::Child::cpu),
            quote!(Machine::cpu, other),
        ] {
            assert!(expand(input).is_err());
        }
    }
}

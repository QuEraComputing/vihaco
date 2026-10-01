// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use syn::{Ident, Path, parse_quote};

/// Remove attributes consumed by this proc-macro crate before re-emitting an
/// item. Attribute macros cannot register helper attributes, so leaving one on
/// the output causes rustc to reject the expanded item.
pub fn strip_vihaco_attrs(attrs: &mut Vec<syn::Attribute>) {
    strip_attrs(attrs, &["vihaco"]);
}

/// Strip only helpers owned by the calling macro, preserving qualified attrs.
pub(crate) fn strip_attrs(attrs: &mut Vec<syn::Attribute>, names: &[&str]) {
    attrs.retain(|attr| !names.iter().any(|name| attr.path().is_ident(name)));
}

/// Resolve the crate root that generated code should be rooted at.
///
/// A downstream crate might depend on the `vihaco` facade *or* directly on
/// `vihaco-runtime`; either way the generated `::<root>::…` paths must resolve.
/// We honour an explicit `#[vihaco(crate = ::some::path)]` override first, then
/// probe (via `proc-macro-crate`) for the facade and then the runtime crate,
/// emitting `crate` when the derive is used inside the runtime crate itself.
pub fn resolve_root(attrs: &[syn::Attribute]) -> syn::Result<Path> {
    if let Some(path) = crate_override(attrs)? {
        return Ok(path);
    }
    Ok(discover_root(|name| crate_name(name).ok()))
}

fn discover_root(mut lookup: impl FnMut(&str) -> Option<FoundCrate>) -> Path {
    for name in ["vihaco", "vihaco-runtime"] {
        if let Some(found) = lookup(name) {
            return match found {
                FoundCrate::Itself => parse_quote!(crate),
                FoundCrate::Name(name) => {
                    let ident = Ident::new(&name, Span::call_site());
                    parse_quote!(::#ident)
                }
            };
        }
    }
    // Retain the canonical runtime fallback for a clear resolution error.
    parse_quote!(::vihaco_runtime)
}

/// A framework root names a module, not a generic type or associated namespace.
pub(crate) fn validate_module_path(path: &Path) -> syn::Result<()> {
    let mut in_super_prefix = path.leading_colon.is_none();
    for (index, segment) in path.segments.iter().enumerate() {
        if !segment.arguments.is_none() {
            return Err(syn::Error::new_spanned(
                &segment.arguments,
                "module paths cannot have generic arguments",
            ));
        }
        let ident = &segment.ident;
        let contextual = ident == "crate" || ident == "self";
        if ident == "Self"
            || (contextual && (index != 0 || path.leading_colon.is_some()))
            || (ident == "super" && !in_super_prefix)
        {
            return Err(syn::Error::new(ident.span(), "invalid module path root"));
        }
        in_super_prefix &= ident == "super";
    }
    Ok(())
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
                if found.is_some() {
                    return Err(meta.error("duplicate `crate` override"));
                }
                let path = meta.value()?.parse::<Path>()?;
                validate_module_path(&path)?;
                found = Some(path);
                Ok(())
            } else {
                Err(meta.error("unsupported vihaco attribute; expected `crate = <path>`"))
            }
        })?;
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_paths_and_unrelated_attributes_keep_their_scope() {
        for path in [
            "self::custom",
            "super::super::custom",
            "crate::custom",
            "::renamed",
            "r#type",
        ] {
            let source = format!("#[vihaco(crate = {path})]");
            let attrs =
                syn::parse::Parser::parse_str(syn::Attribute::parse_outer, &source).unwrap();
            let actual = resolve_root(&attrs).unwrap();
            let expected: Path = syn::parse_str(path).unwrap();
            assert_eq!(
                actual.leading_colon.is_some(),
                expected.leading_colon.is_some()
            );
            assert_eq!(actual.segments.len(), expected.segments.len());
            for (actual, expected) in actual.segments.iter().zip(&expected.segments) {
                assert_eq!(actual.ident, expected.ident);
            }
        }
        let mut attrs = vec![
            parse_quote!(#[doc = "keep"]),
            parse_quote!(#[vihaco(crate = crate::custom)]),
            parse_quote!(#[instructions { cpu }]),
            parse_quote!(#[cfg(feature = "example")]),
            parse_quote!(#[other::vihaco(crate = ignored)]),
        ];
        strip_attrs(&mut attrs, &["vihaco", "instructions"]);
        assert_eq!(attrs.len(), 3);
        assert!(attrs[0].path().is_ident("doc"));
        assert!(attrs[1].path().is_ident("cfg"));
        assert_eq!(attrs[2].path().segments.len(), 2);
    }

    #[test]
    fn root_discovery_prefers_facade_and_handles_runtime_renames_and_self() {
        let mut looked_up = Vec::new();
        let root = discover_root(|name| {
            looked_up.push(name.to_owned());
            Some(FoundCrate::Name("renamed_facade".into()))
        });
        assert_eq!(looked_up, ["vihaco"]);
        assert!(root.leading_colon.is_some());
        assert_eq!(root.segments[0].ident, "renamed_facade");
        let root = discover_root(|name| {
            (name == "vihaco-runtime").then(|| FoundCrate::Name("renamed_runtime".into()))
        });
        assert_eq!(root.segments[0].ident, "renamed_runtime");
        assert!(discover_root(|_| Some(FoundCrate::Itself)).is_ident("crate"));
        let root = discover_root(|_| None);
        assert!(root.leading_colon.is_some());
        assert_eq!(root.segments[0].ident, "vihaco_runtime");
    }

    #[test]
    fn duplicate_and_non_module_roots_are_rejected() {
        for source in [
            "#[vihaco(crate = first, crate = second)]",
            "#[vihaco(crate = first)] #[vihaco(crate = second)]",
        ] {
            let attrs = syn::parse::Parser::parse_str(syn::Attribute::parse_outer, source).unwrap();
            let Err(error) = crate_override(&attrs) else {
                panic!("duplicate root was accepted");
            };
            assert_eq!(error.to_string(), "duplicate `crate` override");
        }
        for attr in [
            parse_quote!(#[vihaco(crate = root<T>)]),
            parse_quote!(#[vihaco(crate = root<>)]),
            parse_quote!(#[vihaco(crate = Self)]),
            parse_quote!(#[vihaco(crate = root::super::other)]),
            parse_quote!(#[vihaco(crate = ::self::other)]),
            parse_quote!(#[vihaco(crate = root::crate)]),
            parse_quote!(#[vihaco(inventory_id = unused)]),
        ] {
            assert!(crate_override(&[attr]).is_err());
        }
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed composite fields, capabilities, and section loaders.

use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::visit::Visit;
use syn::{Fields, Ident, ItemStruct, LitStr, Meta, Path, Result, Token, Type};

use crate::common::{gates, resolve_root, strip_attrs};
use crate::composite_loader::{self, SectionLoadField};

struct DeviceArgs {
    code: Option<u8>,
    aliases: Vec<LitStr>,
}

impl Parse for DeviceArgs {
    /// Read a device code and source aliases for subsequent declaration validation.
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let code = if input.peek(syn::LitInt) {
            let code = input.parse::<syn::LitInt>()?.base10_parse()?;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
            Some(code)
        } else {
            None
        };
        let mut aliases = Vec::new();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            if key != "alias" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected alias = \"source segment\"",
                ));
            }
            input.parse::<Token![=]>()?;
            aliases.push(input.parse()?);
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { code, aliases })
    }
}

struct Field {
    ident: Ident,
    ty: Type,
    device: Option<DeviceArgs>,
    source: String,
    section: Option<String>,
    eligible: bool,
}

struct Composite {
    item: ItemStruct,
    root: Path,
    fields: Vec<Field>,
}

/// Validate a field name or alias as one unambiguous source instruction segment.
fn segment(value: &str, span: Span) -> Result<()> {
    if value.is_empty()
        || value.contains("::")
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(syn::Error::new(
            span,
            "source segment must be nonempty and contain no whitespace, control characters, or `::`",
        ));
    }
    Ok(())
}

/// Record a declaration-local identity, reporting duplicates at the authored span.
fn insert<T: Eq + std::hash::Hash>(
    seen: &mut HashSet<T>,
    value: T,
    span: Span,
    message: &str,
) -> Result<()> {
    if !seen.insert(value) {
        return Err(syn::Error::new(span, message));
    }
    Ok(())
}

/// Validate device codes, aliases, and loadable boundaries while collecting device fields.
fn parse(mut item: ItemStruct) -> Result<Composite> {
    let root = resolve_root(&item.attrs)?;
    strip_attrs(&mut item.attrs, &["vihaco"]);
    let Fields::Named(named) = &mut item.fields else {
        return Err(syn::Error::new_spanned(
            &item,
            "typed composites require named fields",
        ));
    };
    let mut fields = Vec::with_capacity(named.named.len());
    let mut codes = HashSet::new();
    let mut sources = HashSet::new();
    let mut sections = HashSet::new();
    for field in &mut named.named {
        let ident = field.ident.clone().expect("named field");
        let source = ident.unraw().to_string();
        let mut device = None;
        let mut section = None;
        for attr in &field.attrs {
            if attr.path().is_ident("device") {
                if device.is_some() {
                    return Err(syn::Error::new_spanned(attr, "duplicate #[device] helper"));
                }
                device = Some(match &attr.meta {
                    Meta::Path(_) => DeviceArgs {
                        code: None,
                        aliases: Vec::new(),
                    },
                    Meta::List(_) => attr.parse_args()?,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "expected #[device] or #[device(...)]",
                        ));
                    }
                });
            } else if attr.path().is_ident("loadable") {
                if section.is_some() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "duplicate #[loadable] helper",
                    ));
                }
                let (name, span) = match &attr.meta {
                    Meta::Path(_) => (source.clone(), ident.span()),
                    Meta::List(list) if list.tokens.is_empty() => (source.clone(), ident.span()),
                    Meta::List(_) => {
                        let name: LitStr = attr.parse_args()?;
                        (name.value(), name.span())
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "expected #[loadable] or #[loadable(\"section\")]",
                        ));
                    }
                };
                if name.is_empty() || name.contains('/') {
                    return Err(syn::Error::new(
                        span,
                        "loadable section name must be nonempty and contain no `/`",
                    ));
                }
                insert(
                    &mut sections,
                    name.clone(),
                    span,
                    "duplicate loadable section name",
                )?;
                section = Some(name);
            }
        }
        if section.is_some() && device.is_none() {
            return Err(syn::Error::new(
                ident.span(),
                "#[loadable] requires #[device]",
            ));
        }
        if let Some(args) = &device {
            if !gates(&field.attrs).is_empty() {
                return Err(syn::Error::new(
                    ident.span(),
                    "conditional device fields are unsupported; gate the composite",
                ));
            }
            segment(&source, ident.span())?;
            insert(
                &mut sources,
                source.clone(),
                ident.span(),
                "duplicate device source segment",
            )?;
            if args.code.is_none() {
                return Err(syn::Error::new(
                    ident.span(),
                    "every #[device] requires an explicit u8 code",
                ));
            }
            if let Some(code) = args.code {
                insert(&mut codes, code, ident.span(), "duplicate device code")?;
            }
            for alias in &args.aliases {
                segment(&alias.value(), alias.span())?;
                insert(
                    &mut sources,
                    alias.value(),
                    alias.span(),
                    "duplicate device source segment",
                )?;
            }
        }
        let eligible = device.is_some() && section.is_none();
        strip_attrs(&mut field.attrs, &["device", "loadable"]);
        fields.push(Field {
            ident,
            ty: field.ty.clone(),
            device,
            source,
            section,
            eligible,
        });
    }
    Ok(Composite { item, root, fields })
}

/// Determine whether validating a child type requires the declaration's generic scope.
// Only fixed child types can be validated outside the owning generic impl.
// Rust still checks all projected child obligations under the original bounds.
fn depends_on_generics(ty: &Type, generics: &syn::Generics) -> bool {
    struct Dependencies {
        names: HashSet<String>,
        found: bool,
    }

    impl<'ast> Visit<'ast> for Dependencies {
        fn visit_path(&mut self, path: &'ast Path) {
            // A qualified module's later segment may happen to share a generic
            // parameter's name without referring to that parameter.
            if path.leading_colon.is_none() {
                self.found |= path
                    .segments
                    .first()
                    .is_some_and(|s| self.names.contains(&s.ident.to_string()));
            }
            for segment in &path.segments {
                self.visit_path_arguments(&segment.arguments);
            }
        }

        fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
            self.found |= self.names.contains(&lifetime.ident.to_string());
        }

        fn visit_macro(&mut self, _: &'ast syn::Macro) {
            // Type/const macros may refer to parameters inside opaque tokens.
            self.found = true;
        }
    }
    let mut dependencies = Dependencies {
        names: generics
            .params
            .iter()
            .map(|p| match p {
                syn::GenericParam::Type(p) => p.ident.to_string(),
                syn::GenericParam::Const(p) => p.ident.to_string(),
                syn::GenericParam::Lifetime(p) => p.lifetime.ident.to_string(),
            })
            .chain(["Self".to_owned()])
            .collect(),
        found: false,
    };
    dependencies.visit_type(ty);
    dependencies.found
}

/// Find a named child's field companion namespace for nested route lookup.
// Relative authored paths are evaluated in a generated module two levels
// below the owning declaration. Bare names are imported from its original scope.
fn child_namespace(ty: &Type, generics: &syn::Generics) -> Option<Path> {
    let Type::Path(ty) = ty else {
        return None;
    };
    if ty.qself.is_some() {
        return None;
    }
    let first = &ty.path.segments.first()?.ident;
    if generics.type_params().any(|param| param.ident == *first) {
        return None;
    }
    let mut path = ty.path.clone();
    let last = path.segments.last_mut()?;
    last.ident = crate::field::module_ident(&last.ident);
    last.arguments = syn::PathArguments::None;
    Some(rebase_namespace(path))
}

/// Preserve relative child paths inside the generated nested companion modules.
fn rebase_namespace(mut path: Path) -> Path {
    if path.leading_colon.is_some() {
        return path;
    }
    let first = &path.segments.first().expect("nonempty namespace").ident;
    if first == "self" || first == "super" {
        let prefix: Path = syn::parse_quote!(super::super);
        let skip = usize::from(first == "self");
        path.segments = prefix
            .segments
            .into_iter()
            .chain(path.segments.into_iter().skip(skip))
            .collect();
    }
    if path.segments.len() == 1 {
        let prefix: Path = syn::parse_quote!(super::super);
        path.segments = prefix.segments.into_iter().chain(path.segments).collect();
    }
    path
}

/// Generate typed field routes, capabilities, loaders, and flat instruction inventory queries.
pub(crate) fn expand(options: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !options.is_empty() {
        return Err(syn::Error::new_spanned(
            options,
            "#[composite] takes no arguments",
        ));
    }
    let Composite { item, root, fields } = parse(syn::parse2(item)?)?;
    let namespace_root = crate::common::root_in_child_module(&root);
    let name = &item.ident;
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    let gates = gates(&item.attrs);
    let marker_module = crate::field::module_ident(name);
    let eligible: Vec<_> = fields.iter().filter(|f| f.eligible).collect();
    let mut requests = Vec::new();
    let mut inventory_checks = Vec::new();
    for field in &eligible {
        let Type::Path(child) = &field.ty else {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "instruction inventory requires a named child declaration path",
            ));
        };
        let first = &child
            .path
            .segments
            .first()
            .expect("nonempty type path")
            .ident;
        if child.qself.is_some()
            || first == "Self"
            || item
                .generics
                .type_params()
                .any(|param| param.ident == *first)
        {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "instruction inventory requires a named child declaration path; generic and associated child declarations cannot be inferred",
            ));
        }
        let mut companion = child.path.clone();
        for segment in &mut companion.segments {
            segment.arguments = syn::PathArguments::None;
        }
        let last = companion.segments.last_mut().expect("nonempty type path");
        last.ident = crate::inventory::module_ident(&last.ident);
        let child_type = &field.ty;
        inventory_checks.push(
            quote! { #root::__private::require_inventory::<#child_type, #companion::Provider>(); },
        );
        let marker = &field.ident;
        let args = field.device.as_ref().expect("eligible device");
        let code = args.code.expect("validated code");
        let mut source = vec![LitStr::new(&field.source, marker.span())];
        source.extend(args.aliases.clone());
        requests.push(crate::inventory::Request {
            callback: syn::parse_quote!(#companion::__enumerate),
            provider: syn::parse_quote!(#companion::Provider),
            prefix: vec![crate::inventory::Segment {
                name: marker.clone(),
                source,
            }],
            field: Some(syn::parse_quote!(#marker_module::__markers::#marker)),
            codes: vec![syn::LitInt::new(&code.to_string(), marker.span())],
        });
    }
    let inventory = crate::inventory::start("composite", &root, &item, requests);
    let relations = eligible.iter().map(|field| {
        let marker = &field.ident;
        let ty = &field.ty;
        let source = &field.source;
        let aliases = &field.device.as_ref().expect("eligible device").aliases;
        quote! {
            #(#[#gates])*
            impl #impl_generics #root::__private::DirectField<#marker_module::__markers::#marker> for #name #type_generics #where_clause {}
            #(#[#gates])*
            impl #impl_generics #root::Field<#marker_module::__markers::#marker> for #name #type_generics #where_clause {
                type Child = #ty;
                const SOURCE: &'static [&'static str] = &[#source, #(#aliases),*];
            }
        }
    });
    let markers = eligible.iter().map(|field| {
        let marker = &field.ident;
        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            pub struct #marker;
        }
    });
    let children: Vec<_> = eligible
        .iter()
        .filter_map(|field| {
            let namespace = child_namespace(&field.ty, &item.generics);
            namespace.map(|namespace| {
                let field = &field.ident;
                quote! { #[allow(unused_imports)] pub use #namespace as #field; }
            })
        })
        .collect();
    let devices = eligible.iter().map(|field| {
        let marker = &field.ident;
        let source = &field.source;
        quote! { #root::__private::Device {
            name: #source,
            child: <<Self as #root::Field<#marker_module::__markers::#marker>>::Child as #root::__private::Capabilities>::DESCRIPTION,
        } }
    });
    let metadata = fields.iter().filter_map(|field| {
        field.device.as_ref().map(|args| {
            let source = &field.source;
            let aliases = &args.aliases;
            let code = args.code.expect("validated device code");
            let section = match &field.section {
                Some(section) => quote!(::core::option::Option::Some(#section)),
                None => quote!(::core::option::Option::None),
            };
            quote! { #root::__private::CompositeDevice {
                name: #source, code: #code, source: &[#source, #(#aliases),*], section: #section,
            } }
        })
    });
    let fixed_checks: Vec<_> = if item.generics.params.is_empty() {
        vec![
            quote! { #root::__private::validate(<#name as #root::__private::Capabilities>::DESCRIPTION); },
        ]
    } else {
        eligible.iter().filter(|f| !depends_on_generics(&f.ty, &item.generics)).map(|field| {
            let ty = &field.ty;
            quote! { #root::__private::validate(<#ty as #root::__private::Capabilities>::DESCRIPTION); }
        }).collect()
    };
    let loadables: Vec<_> = fields
        .iter()
        .filter_map(|f| {
            f.section.as_ref().map(|section| SectionLoadField {
                field: f.ident.clone(),
                ty: f.ty.clone(),
                section_name: section.clone(),
            })
        })
        .collect();
    let loaders = composite_loader::emit(name, &item.generics, &root, &loadables, &gates)?;
    Ok(quote! {
        #item
        #(#[#gates])*
        #inventory
        #(#[#gates])*
        const _: () = {
            #[allow(dead_code, clippy::multiple_bound_locations)]
            fn __vihaco_check_inventory #impl_generics () #where_clause { #(#inventory_checks)* }
        };
        #(#[#gates])*
        const _: () = { #(#fixed_checks)* };
        #(#relations)*
        #(#[#gates])*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #marker_module {
            #[allow(unused_imports)]
            use super::*;

            #[doc(hidden)]
            #[allow(unused_imports)]
            pub use #namespace_root::__private::FieldPath as __FieldPath;

            #[doc(hidden)]
            #[allow(unused_imports)]
            pub use #namespace_root::__private::CheckedRoute as __CheckedRoute;

            #[doc(hidden)]
            pub mod __markers { #(#markers)* }

            #[doc(hidden)]
            pub mod __children {
                #[allow(unused_imports)]
                use super::super::*;
                #(#children)*
            }
        }
        #(#[#gates])*
        impl #impl_generics #root::Composite for #name #type_generics #where_clause {}
        #(#[#gates])*
        impl #impl_generics #root::__private::Capabilities for #name #type_generics #where_clause {
            const DESCRIPTION: &'static #root::__private::Description =
                &#root::__private::Description::Composite(&[#(#devices),*]);
        }
        #(#[#gates])*
        impl #impl_generics #root::__private::CompositeSpec for #name #type_generics #where_clause {
            const DEVICES: &'static [#root::__private::CompositeDevice] = &[#(#metadata),*];
        }
        #loaders
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_distinguishes_fixed_and_generic_child_types() {
        let generics = syn::parse_quote!(<'a, T, const N: usize>);
        for source in [
            "T",
            "<T as Trait>::Child",
            "Cpu<'a, N>",
            "[Cpu; N]",
            "Cpu<{ N + 1 }>",
            "ty!(T)",
            "Self::Child",
        ] {
            assert!(
                depends_on_generics(&syn::parse_str(source).unwrap(), &generics),
                "{source}"
            );
        }
        for source in [
            "module::T",
            "::module::T",
            "[Cpu; 3]",
            "Cpu<'static, 2>",
            "std::marker::PhantomData<u8>",
        ] {
            assert!(
                !depends_on_generics(&syn::parse_str(source).unwrap(), &generics),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_declaration_local_collisions_and_invalid_helpers() {
        for fields in [
            "#[device] #[device] a: u8",
            "#[device(1)] a: u8, #[device(1)] #[loadable] b: u8",
            "#[device(1, alias = \"b\")] a: u8, #[device(2)] b: u8",
            "#[device(1, alias = \"a\")] a: u8",
            "#[device(1, alias = \"x\", alias = \"x\")] a: u8",
            "#[device(1, alias = \"\")] a: u8",
            "#[device(1, alias = \"a::b\")] a: u8",
            "#[device(1, alias = \"a b\")] a: u8",
            "#[device(1, alias = \"a\\n\")] a: u8",
            "#[device(1, alias = \"a\\u{7f}\")] a: u8",
            "#[device(1, alias = \"a\\u{2003}b\")] a: u8",
            "#[device(code = 2)] a: u8",
            "#[device(256)] a: u8",
            "#[loadable] a: u8",
            "#[device(1)] #[loadable] #[loadable] a: u8",
            "#[device(1)] #[loadable(\"\")] a: u8",
            "#[device(1)] #[loadable(\"a/b\")] a: u8",
            "#[device(1)] #[loadable(\"x\")] a: u8, #[device(1)] #[loadable(\"x\")] b: u8",
            "#[device(1, alias = \"x\")] #[loadable] a: u8, #[device(1, alias = \"x\")] b: u8",
            "#[cfg(any())] #[device(1)] a: u8",
        ] {
            let item: TokenStream =
                format!("#[vihaco(crate = crate)] struct Parent {{ {fields} }}")
                    .parse()
                    .unwrap();
            assert!(expand(TokenStream::new(), item).is_err(), "{fields}");
        }
        for item in [
            quote!(
                struct Tuple(u8);
            ),
            quote!(
                struct Unit;
            ),
            quote!(
                enum Enum {}
            ),
        ] {
            assert!(expand(TokenStream::new(), item).is_err());
        }
        assert!(
            expand(
                quote!(unknown),
                quote!(
                    struct Empty {}
                )
            )
            .is_err()
        );
        assert!(
            expand(
                TokenStream::new(),
                quote!(
                    #[vihaco(crate = crate, crate = crate)]
                    struct Empty {}
                )
            )
            .is_err()
        );
    }
}

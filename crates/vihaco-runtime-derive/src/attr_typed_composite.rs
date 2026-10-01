// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Internal typed composite preparation. Public activation belongs to the
//! complete instruction-set/parser contract, not this emitter in isolation.

use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
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

// Kept unreachable from the proc-macro entry point until Step 7. Unit compiler
// fixtures exercise this exact emitter, rather than a public placeholder macro.
#[allow(dead_code)]
pub(crate) fn expand(options: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !options.is_empty() {
        return Err(syn::Error::new_spanned(
            options,
            "#[composite] takes no arguments",
        ));
    }
    let Composite { item, root, fields } = parse(syn::parse2(item)?)?;
    let name = &item.ident;
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    let gates = gates(&item.attrs);
    let eligible: Vec<_> = fields.iter().filter(|f| f.eligible).collect();
    let relations = eligible.iter().enumerate().map(|(index, field)| {
        let ty = &field.ty;
        let source = &field.source;
        let aliases = &field.device.as_ref().expect("eligible device").aliases;
        quote! {
            #(#[#gates])*
            impl #impl_generics #root::__private::Field<#index> for #name #type_generics #where_clause {
                type Child = #ty;
                const SOURCE: &'static [&'static str] = &[#source, #(#aliases),*];
            }
        }
    });
    let constants = eligible.iter().enumerate().map(|(index, field)| {
        let constant = format_ident!(
            "__vihaco_field_{}",
            field.ident.unraw(),
            span = field.ident.span()
        );
        quote! {
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            pub const #constant: usize = #index;
        }
    });
    let devices = eligible.iter().enumerate().map(|(index, field)| {
        let source = &field.source;
        quote! { #root::__private::Device {
            name: #source,
            child: <<Self as #root::__private::Field<#index>>::Child as #root::__private::Capabilities>::DESCRIPTION,
        } }
    });
    let metadata = fields.iter().filter_map(|field| {
        field.device.as_ref().map(|args| {
            let source = &field.source;
            let aliases = &args.aliases;
            let code = match args.code {
                Some(code) => quote!(::core::option::Option::Some(#code)),
                None => quote!(::core::option::Option::None),
            };
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
        const _: () = { #(#fixed_checks)* };
        #(#relations)*
        #(#[#gates])*
        impl #impl_generics #name #type_generics #where_clause { #(#constants)* }
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
    use quote::ToTokens;
    use std::io::Write;
    use std::path::{Path as FilePath, PathBuf};
    use std::process::{Command, Stdio};
    use syn::visit_mut::{self, VisitMut};

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
            "#[device(alias = \"b\")] a: u8, #[device] b: u8",
            "#[device(alias = \"a\")] a: u8",
            "#[device(alias = \"x\", alias = \"x\")] a: u8",
            "#[device(alias = \"\")] a: u8",
            "#[device(alias = \"a::b\")] a: u8",
            "#[device(alias = \"a b\")] a: u8",
            "#[device(alias = \"a\\n\")] a: u8",
            "#[device(alias = \"a\\u{7f}\")] a: u8",
            "#[device(alias = \"a\\u{2003}b\")] a: u8",
            "#[device(code = 2)] a: u8",
            "#[device(256)] a: u8",
            "#[loadable] a: u8",
            "#[device] #[loadable] #[loadable] a: u8",
            "#[device] #[loadable(\"\")] a: u8",
            "#[device] #[loadable(\"a/b\")] a: u8",
            "#[device] #[loadable(\"x\")] a: u8, #[device] #[loadable(\"x\")] b: u8",
            "#[device(alias = \"x\")] #[loadable] a: u8, #[device(alias = \"x\")] b: u8",
            "#[cfg(any())] #[device] a: u8",
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

    struct Expand;
    impl VisitMut for Expand {
        fn visit_item_mut(&mut self, item: &mut syn::Item) {
            visit_mut::visit_item_mut(self, item);
            if let syn::Item::Struct(declaration) = item
                && declaration
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("composite"))
            {
                strip_attrs(&mut declaration.attrs, &["composite"]);
                *item = syn::Item::Verbatim(
                    expand(TokenStream::new(), declaration.to_token_stream()).unwrap(),
                );
            }
        }
    }

    fn library(dependencies: &FilePath, name: &str) -> PathBuf {
        // Explicitly selected library targets have stable artifact paths, even
        // when dependency hashes from older fixture runs remain in deps/.
        dependencies
            .parent()
            .unwrap()
            .join(format!("lib{name}.rlib"))
    }

    #[test]
    fn static_fixtures_compile_and_run_the_internal_emitter() {
        // Build the real runtime in an isolated graph. No stubbed contracts or
        // generated source files: authored fixtures are expanded to rustc stdin.
        let target =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/typed-composite-fixtures");
        let built = Command::new(env!("CARGO"))
            .args([
                "build",
                "--offline",
                "-p",
                "vihaco-runtime",
                "-p",
                "vihaco-bytecode",
                "-p",
                "vihaco-module",
                "-p",
                "eyre",
                "--features",
                "typed-declarations",
                "--target-dir",
            ])
            .arg(&target)
            .arg("--manifest-path")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vihaco-runtime/Cargo.toml"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let dependencies = target.join("debug/deps");
        for (name, source, error) in [
            (
                "positive",
                include_str!("../tests/fixtures/composites/positive.rs"),
                None,
            ),
            (
                "ordinary-target",
                include_str!("../tests/fixtures/composites/ordinary-target.rs"),
                Some("__vihaco_field_state"),
            ),
            (
                "loadable-target",
                include_str!("../tests/fixtures/composites/loadable-target.rs"),
                Some("__vihaco_field_image"),
            ),
            (
                "fixed-overlap",
                include_str!("../tests/fixtures/composites/fixed-overlap.rs"),
                Some("overlapping canonical instruction selector"),
            ),
            (
                "generic-overlap",
                include_str!("../tests/fixtures/composites/generic-overlap.rs"),
                Some("overlapping canonical instruction selector"),
            ),
            (
                "missing-bound",
                include_str!("../tests/fixtures/composites/missing-bound.rs"),
                Some("not implemented for `T`"),
            ),
            (
                "loaders",
                include_str!("../tests/fixtures/composites/loaders.rs"),
                None,
            ),
        ] {
            let mut file: syn::File = syn::parse_str(source).unwrap();
            Expand.visit_file_mut(&mut file);
            let binary = target.join(name);
            let mut compiler = Command::new("rustc")
                .args([
                    "--edition=2024",
                    "--crate-name",
                    "composite_fixture",
                    "-",
                    "-o",
                ])
                .arg(&binary)
                .arg("-L")
                .arg(format!("dependency={}", dependencies.display()))
                .arg("--extern")
                .arg(format!(
                    "vihaco_runtime={}",
                    library(&dependencies, "vihaco_runtime").display()
                ))
                .arg("--extern")
                .arg(format!("eyre={}", library(&dependencies, "eyre").display()))
                .arg("--extern")
                .arg(format!(
                    "vihaco_bytecode={}",
                    library(&dependencies, "vihaco_bytecode").display()
                ))
                .arg("--extern")
                .arg(format!(
                    "vihaco_module={}",
                    library(&dependencies, "vihaco_module").display()
                ))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            compiler
                .stdin
                .take()
                .unwrap()
                .write_all(
                    prettyplease::unparse(&syn::parse2(file.to_token_stream()).unwrap()).as_bytes(),
                )
                .unwrap();
            let result = compiler.wait_with_output().unwrap();
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            if let Some(error) = error {
                assert!(
                    !result.status.success() && diagnostic.contains(error),
                    "{name}: {diagnostic}"
                );
            } else {
                assert!(result.status.success(), "{name}: {diagnostic}");
                let run = Command::new(&binary).output().unwrap();
                assert!(
                    run.status.success(),
                    "{name}: {}",
                    String::from_utf8_lossy(&run.stderr)
                );
            }
        }
    }
}

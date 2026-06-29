// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, GenericParam, Lifetime, LitStr, Token};

struct DeviceArgs {
    code: u8,
    aliases: Vec<syn::LitStr>,
}

struct SectionLoadArgs {
    section_name: Option<LitStr>,
}

impl Parse for SectionLoadArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let section_name = if input.is_empty() {
            None
        } else {
            let section_name = input.parse::<LitStr>()?;
            if !input.is_empty() {
                return Err(input.error("unexpected tokens in loadable attribute"));
            }
            Some(section_name)
        };
        Ok(SectionLoadArgs { section_name })
    }
}

struct SectionLoadField {
    field: syn::Ident,
    ty: syn::Type,
    section_name: String,
}

impl Parse for DeviceArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let code_lit: syn::LitInt = input.parse()?;
        let code = code_lit.base10_parse::<u8>()?;
        let mut aliases = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() || !input.peek(syn::Ident) {
                break; // trailing comma
            }
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match ident.to_string().as_str() {
                "alias" => {
                    aliases.push(input.parse::<syn::LitStr>()?);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "unsupported device argument",
                    ));
                }
            }
        }
        Ok(Self { code, aliases })
    }
}

fn pascal_case(ident: &syn::Ident) -> syn::Ident {
    let mut out = String::new();
    for part in ident.to_string().split('_') {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    format_ident!("{}", out)
}

fn validate_loadable_name(name: &str, span: proc_macro2::Span) -> syn::Result<()> {
    if name.is_empty() {
        return Err(syn::Error::new(
            span,
            "loadable section name cannot be empty",
        ));
    }
    if name.contains('/') {
        return Err(syn::Error::new(
            span,
            "loadable section name cannot contain `/`",
        ));
    }
    Ok(())
}

fn method_where_clause(predicates: &[TokenStream2]) -> TokenStream2 {
    if predicates.is_empty() {
        quote! {}
    } else {
        quote! {
            where
                #( #predicates ),*
        }
    }
}

fn stream_contains_ident(stream: TokenStream2, ident: &syn::Ident) -> bool {
    stream.into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Ident(found) => found == *ident,
        proc_macro2::TokenTree::Group(group) => stream_contains_ident(group.stream(), ident),
        _ => false,
    })
}

fn stream_contains_lifetime(stream: &TokenStream2, lifetime: &Lifetime) -> bool {
    stream.to_string().contains(&lifetime.to_string())
}

fn generic_param_used(param: &GenericParam, streams: &[TokenStream2]) -> bool {
    match param {
        GenericParam::Lifetime(param) => streams
            .iter()
            .any(|stream| stream_contains_lifetime(stream, &param.lifetime)),
        GenericParam::Type(param) => streams
            .iter()
            .any(|stream| stream_contains_ident(stream.clone(), &param.ident)),
        GenericParam::Const(param) => streams
            .iter()
            .any(|stream| stream_contains_ident(stream.clone(), &param.ident)),
    }
}

fn stream_mentions_any_generic(stream: TokenStream2, params: &[GenericParam]) -> bool {
    params.iter().any(|param| match param {
        GenericParam::Lifetime(param) => stream_contains_lifetime(&stream, &param.lifetime),
        GenericParam::Type(param) => stream_contains_ident(stream.clone(), &param.ident),
        GenericParam::Const(param) => stream_contains_ident(stream.clone(), &param.ident),
    })
}

fn enum_generics_for_device_fields(
    generics: &syn::Generics,
    devices: &[(syn::Ident, syn::Type, DeviceArgs)],
) -> syn::Generics {
    let device_streams: Vec<_> = devices
        .iter()
        .map(|(_, ty, _)| ty.to_token_stream())
        .collect();
    let retained_params: Vec<GenericParam> = generics
        .params
        .iter()
        .filter(|param| generic_param_used(param, &device_streams))
        .cloned()
        .collect();

    let mut enum_generics = generics.clone();
    enum_generics.params = retained_params.iter().cloned().collect();

    if let Some(where_clause) = &mut enum_generics.where_clause {
        where_clause.predicates = where_clause
            .predicates
            .iter()
            .filter(|predicate| {
                stream_mentions_any_generic(predicate.to_token_stream(), &retained_params)
            })
            .cloned()
            .collect();
        if where_clause.predicates.is_empty() {
            enum_generics.where_clause = None;
        }
    }

    enum_generics
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match try_expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn try_expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = input.ident;
    let generics = input.generics;
    let data = match input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring can only be generated for structs",
            ));
        }
    };
    let fields = match data.fields {
        Fields::Named(fields) => fields.named,
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring requires a struct with named fields",
            ));
        }
    };

    let mut devices = Vec::new();
    let mut program_field: Option<(syn::Ident, syn::Type)> = None;
    let mut header_field: Option<(syn::Ident, syn::Type)> = None;
    let mut loadables = Vec::<SectionLoadField>::new();

    for field in fields {
        let field_ident = field.ident.expect("named field");
        let field_ty = field.ty;
        let mut is_device = false;
        let mut is_program = false;
        let mut is_header = false;
        let mut loadable_args = None;
        for attr in &field.attrs {
            let path = attr.path();
            if path.is_ident("program") {
                if let Some((existing, _)) = &program_field {
                    return Err(syn::Error::new(
                        field_ident.span(),
                        format!(
                            "duplicate program field `{}`; `{}` is already marked #[program]",
                            field_ident, existing
                        ),
                    ));
                }
                is_program = true;
                program_field = Some((field_ident.clone(), field_ty.clone()));
            } else if path.is_ident("header") {
                if !matches!(&attr.meta, syn::Meta::Path(_)) {
                    return Err(syn::Error::new(
                        attr.span(),
                        "header attribute does not take arguments",
                    ));
                }
                if let Some((existing, _)) = &header_field {
                    return Err(syn::Error::new(
                        field_ident.span(),
                        format!(
                            "duplicate header field `{}`; `{}` is already marked #[header]",
                            field_ident, existing
                        ),
                    ));
                }
                is_header = true;
                header_field = Some((field_ident.clone(), field_ty.clone()));
            } else if path.is_ident("device") {
                is_device = true;
                let args = attr.parse_args::<DeviceArgs>()?;
                devices.push((field_ident.clone(), field_ty.clone(), args));
            } else if path.is_ident("loadable") {
                if loadable_args.is_some() {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("duplicate loadable attribute on field `{}`", field_ident),
                    ));
                }
                loadable_args = Some(if matches!(&attr.meta, syn::Meta::Path(_)) {
                    SectionLoadArgs { section_name: None }
                } else {
                    attr.parse_args::<SectionLoadArgs>()?
                });
            }
        }
        if is_header && (is_program || is_device || loadable_args.is_some()) {
            return Err(syn::Error::new(
                field_ident.span(),
                format!(
                    "field `{}` marked #[header] cannot also be marked #[program], #[device(...)] or #[loadable]",
                    field_ident
                ),
            ));
        }
        if let Some(args) = loadable_args {
            if !is_device {
                return Err(syn::Error::new(
                    field_ident.span(),
                    format!(
                        "field `{}` marked #[loadable] must also be marked #[device(...)]",
                        field_ident
                    ),
                ));
            }
            if is_program {
                return Err(syn::Error::new(
                    field_ident.span(),
                    format!(
                        "field `{}` cannot be both #[program] and #[loadable]",
                        field_ident
                    ),
                ));
            }
            let section_name = if let Some(lit) = args.section_name {
                let value = lit.value();
                validate_loadable_name(&value, lit.span())?;
                value
            } else {
                let value = field_ident.to_string();
                validate_loadable_name(&value, field_ident.span())?;
                value
            };
            loadables.push(SectionLoadField {
                field: field_ident.clone(),
                ty: field_ty.clone(),
                section_name,
            });
        }
    }

    let mut seen_device_codes = BTreeMap::<u8, syn::Ident>::new();
    for (field, _, args) in &devices {
        if let Some(existing) = seen_device_codes.insert(args.code, field.clone()) {
            return Err(syn::Error::new(
                field.span(),
                format!(
                    "duplicate device code 0x{:02X} for fields `{}` and `{}`",
                    args.code, existing, field
                ),
            ));
        }
    }

    let mut seen_source_symbols = BTreeMap::<String, syn::Ident>::new();
    for (field, _, args) in &devices {
        let field_name = field.to_string();
        if let Some(existing) = seen_source_symbols.insert(field_name.clone(), field.clone()) {
            return Err(syn::Error::new(
                field.span(),
                format!(
                    "duplicate source symbol `{}` for `{}` and `{}`",
                    field_name, existing, field
                ),
            ));
        }

        let mut local_aliases = BTreeSet::new();
        for alias in &args.aliases {
            let alias_name = alias.value();
            if !local_aliases.insert(alias_name.clone()) {
                return Err(syn::Error::new(
                    alias.span(),
                    format!("duplicate alias `{}` on field `{}`", alias_name, field),
                ));
            }
            if let Some(existing) = seen_source_symbols.insert(alias_name.clone(), field.clone()) {
                return Err(syn::Error::new(
                    alias.span(),
                    format!(
                        "duplicate source symbol `{}` for `{}` and `{}`",
                        alias_name, existing, field
                    ),
                ));
            }
        }
    }

    let mut seen_loadable_names = BTreeMap::<String, syn::Ident>::new();
    for loadable in &loadables {
        if let Some(existing) =
            seen_loadable_names.insert(loadable.section_name.clone(), loadable.field.clone())
        {
            return Err(syn::Error::new(
                loadable.field.span(),
                format!(
                    "duplicate loadable section name `{}` for fields `{}` and `{}`",
                    loadable.section_name, existing, loadable.field
                ),
            ));
        }
    }

    let machine_instruction_ident = format_ident!("{}Instruction", ident);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let enum_generics = enum_generics_for_device_fields(&generics, &devices);
    let (_, enum_ty_generics, _) = enum_generics.split_for_impl();

    let machine_instruction_variants: Vec<_> = devices
        .iter()
        .map(|(field, field_ty, _)| {
            let variant_ident = pascal_case(field);
            quote! {
                #variant_ident(<#field_ty as ::vihaco::GeneratedComponent>::Instruction)
            }
        })
        .collect();

    let device_entries: Vec<_> = devices
        .iter()
        .map(|(field, _, args)| {
            let name = field.to_string();
            let code = args.code;
            quote! { ::vihaco::metadata::DeviceMetadata { code: #code, name: #name } }
        })
        .collect();
    let source_symbol_alias_entries: Vec<_> = devices
        .iter()
        .flat_map(|(_, _, args)| {
            let code = args.code;
            args.aliases.iter().map(move |alias| {
                quote! {
                    ::vihaco::metadata::SourceSymbolAliasMetadata {
                        name: #alias,
                        device_code: #code,
                    }
                }
            })
        })
        .collect();

    let program_impl = if let Some((ref field_name, ref field_ty)) = program_field {
        quote! {
            impl #impl_generics ::vihaco::traits::ProgramCounter for #ident #ty_generics #where_clause {
                type Instruction = <#field_ty as ::vihaco::traits::ProgramCounter>::Instruction;

                fn pc(&self) -> u32 {
                    self.#field_name.pc()
                }

                fn pc_mut(&mut self) -> &mut u32 {
                    self.#field_name.pc_mut()
                }

                fn get_instruction(&self, pc: u32) -> eyre::Result<&Self::Instruction> {
                    self.#field_name.get_instruction(pc)
                }
            }
        }
    } else {
        quote! {}
    };

    let bc_lifetime = Lifetime::new("'__vihaco_bc", proc_macro2::Span::call_site());
    let loadable_context_param = format_ident!("__VihacoContext");
    let mut loadable_predicates = Vec::<TokenStream2>::new();
    loadable_predicates.push(quote! { #loadable_context_param: ::vihaco::binary::BytecodeContext });
    if let Some((_, header_ty)) = &header_field {
        loadable_predicates.push(quote! { #header_ty: ::vihaco::binary::CompositeHeader });
    }
    if let Some((_, program_ty)) = &program_field {
        loadable_predicates
            .push(quote! { #program_ty: ::vihaco::loader::LoadSection<::std::vec::Vec<u8>, #loadable_context_param> });
    }
    for loadable in &loadables {
        let ty = &loadable.ty;
        loadable_predicates
            .push(quote! { #ty: ::vihaco::loader::LoadSection<::std::vec::Vec<u8>, #loadable_context_param> });
    }
    let loadable_method_where = method_where_clause(&loadable_predicates);

    let mut loadable_impl_generics = generics.clone();
    loadable_impl_generics
        .params
        .push(syn::parse_quote!(#loadable_context_param));
    if !loadable_predicates.is_empty() {
        let where_clause = loadable_impl_generics.make_where_clause();
        for predicate in &loadable_predicates {
            where_clause
                .predicates
                .push(syn::parse2(predicate.clone())?);
        }
    }
    let (loadable_impl_generics, _, loadable_where_clause) =
        loadable_impl_generics.split_for_impl();

    let program_load = if let Some((field_name, field_ty)) = &program_field {
        quote! {
            <#field_ty as ::vihaco::loader::LoadSection<::std::vec::Vec<u8>, #loadable_context_param>>::load_section(
                &mut self.#field_name,
                input.clone(),
            )?;
        }
    } else {
        quote! {
            if !input.section.bytecode().is_empty() {
                return Err(::eyre::eyre!(
                    "section `{}` has bytecode but `{}` has no #[program] field",
                    input.section.display_path(),
                    stringify!(#ident),
                ));
            }
        }
    };

    let header_load = if let Some((field_name, field_ty)) = &header_field {
        quote! {
            self.#field_name = input.section.parse_header::<#field_ty>()?;
        }
    } else {
        quote! {}
    };

    let loadable_names: Vec<_> = loadables
        .iter()
        .map(|loadable| loadable.section_name.clone())
        .collect();
    let child_loads: Vec<_> = loadables
        .iter()
        .map(|loadable| {
            let field = &loadable.field;
            let ty = &loadable.ty;
            let name = &loadable.section_name;
            quote! {
                if let ::std::option::Option::Some(__vihaco_child) = input.section.child(#name) {
                    <#ty as ::vihaco::loader::LoadSection<::std::vec::Vec<u8>, #loadable_context_param>>::load_section(
                        &mut self.#field,
                        ::vihaco::loader::LoadInput::from(__vihaco_child)
                    )?;
                }
            }
        })
        .collect();

    let loadable_impl = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn load_generated_sections<#bc_lifetime, #loadable_context_param>(
                &mut self,
                input: ::vihaco::loader::LoadInput<#bc_lifetime, ::std::vec::Vec<u8>, #loadable_context_param>,
            ) -> ::eyre::Result<()>
            #loadable_method_where
            {
                #header_load
                #program_load

                let __vihaco_expected_children: &[&str] = &[#(#loadable_names),*];

                for __vihaco_child in input.section.children() {
                    let __vihaco_child_name = __vihaco_child.local_name().ok_or_else(|| {
                        ::eyre::eyre!(
                            "section `{}` yielded a root section as a child",
                            input.section.display_path(),
                        )
                    })?;
                    if !__vihaco_expected_children
                        .iter()
                        .any(|__vihaco_expected| *__vihaco_expected == __vihaco_child_name)
                    {
                        return Err(::eyre::eyre!(
                            "section `{}` has unexpected child section `{}`",
                            input.section.display_path(),
                            __vihaco_child.display_path(),
                        ));
                    }
                }

                #( #child_loads )*
                Ok(())
            }
        }

        impl #loadable_impl_generics ::vihaco::loader::LoadSection<::std::vec::Vec<u8>, #loadable_context_param>
            for #ident #ty_generics
            #loadable_where_clause
        {
            fn load_section<#bc_lifetime>(
                &mut self,
                input: ::vihaco::loader::LoadInput<#bc_lifetime, ::std::vec::Vec<u8>, #loadable_context_param>,
            ) -> ::eyre::Result<()> {
                self.load_generated_sections(input)
            }
        }
    };

    let mut text_loadable_predicates = Vec::<TokenStream2>::new();
    text_loadable_predicates
        .push(quote! { #loadable_context_param: ::vihaco::binary::BytecodeContext });
    if let Some((_, header_ty)) = &header_field {
        text_loadable_predicates.push(quote! { #header_ty: ::std::str::FromStr });
        text_loadable_predicates
            .push(quote! { <#header_ty as ::std::str::FromStr>::Err: ::std::fmt::Display });
    }
    if let Some((_, program_ty)) = &program_field {
        text_loadable_predicates
            .push(quote! { #program_ty: ::vihaco::loader::LoadSection<::std::string::String, #loadable_context_param> });
    }
    for loadable in &loadables {
        let ty = &loadable.ty;
        text_loadable_predicates
            .push(quote! { #ty: ::vihaco::loader::LoadSection<::std::string::String, #loadable_context_param> });
    }
    let text_loadable_method_where = method_where_clause(&text_loadable_predicates);

    let mut text_loadable_impl_generics = generics.clone();
    text_loadable_impl_generics
        .params
        .push(syn::parse_quote!(#loadable_context_param));
    if !text_loadable_predicates.is_empty() {
        let where_clause = text_loadable_impl_generics.make_where_clause();
        for predicate in &text_loadable_predicates {
            where_clause
                .predicates
                .push(syn::parse2(predicate.clone())?);
        }
    }
    let (text_loadable_impl_generics, _, text_loadable_where_clause) =
        text_loadable_impl_generics.split_for_impl();

    let text_program_load = if let Some((field_name, field_ty)) = &program_field {
        quote! {
            <#field_ty as ::vihaco::loader::LoadSection<::std::string::String, #loadable_context_param>>::load_section(
                &mut self.#field_name,
                input.clone(),
            )?;
        }
    } else {
        quote! {
            if !input.section.text().trim().is_empty() {
                return Err(::eyre::eyre!(
                    "section `{}` has bytecode but `{}` has no #[program] field",
                    input.section.display_path(),
                    stringify!(#ident),
                ));
            }
        }
    };

    let text_header_load = if let Some((field_name, field_ty)) = &header_field {
        quote! {
            self.#field_name = input
                .section
                .header_text()
                .trim()
                .parse::<#field_ty>()
                .map_err(|__vihaco_err| {
                    ::eyre::eyre!(
                        "failed to parse section `{}` header for `{}`: {}",
                        input.section.display_path(),
                        stringify!(#field_ty),
                        __vihaco_err,
                    )
                })?;
        }
    } else {
        quote! {}
    };

    let text_child_loads: Vec<_> = loadables
        .iter()
        .map(|loadable| {
            let field = &loadable.field;
            let ty = &loadable.ty;
            let name = &loadable.section_name;
            quote! {
                if let ::std::option::Option::Some(__vihaco_child) = input.section.child(#name) {
                    <#ty as ::vihaco::loader::LoadSection<::std::string::String, #loadable_context_param>>::load_section(
                        &mut self.#field,
                        ::vihaco::loader::LoadInput::from(__vihaco_child)
                    )?;
                }
            }
        })
        .collect();

    let text_loadable_impl = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn load_generated_text_sections<#bc_lifetime, #loadable_context_param>(
                &mut self,
                input: ::vihaco::loader::LoadInput<#bc_lifetime, ::std::string::String, #loadable_context_param>,
            ) -> ::eyre::Result<()>
            #text_loadable_method_where
            {
                #text_header_load
                #text_program_load

                let __vihaco_expected_children: &[&str] = &[#(#loadable_names),*];

                for __vihaco_child in input.section.children() {
                    let __vihaco_child_name = __vihaco_child.local_name().ok_or_else(|| {
                        ::eyre::eyre!(
                            "section `{}` yielded a root section as a child",
                            input.section.display_path(),
                        )
                    })?;
                    if !__vihaco_expected_children
                        .iter()
                        .any(|__vihaco_expected| *__vihaco_expected == __vihaco_child_name)
                    {
                        return Err(::eyre::eyre!(
                            "section `{}` has unexpected child section `{}`",
                            input.section.display_path(),
                            __vihaco_child.display_path(),
                        ));
                    }
                }

                #( #text_child_loads )*
                Ok(())
            }
        }

        impl #text_loadable_impl_generics ::vihaco::loader::LoadSection<::std::string::String, #loadable_context_param>
            for #ident #ty_generics
            #text_loadable_where_clause
        {
            fn load_section<#bc_lifetime>(
                &mut self,
                input: ::vihaco::loader::LoadInput<#bc_lifetime, ::std::string::String, #loadable_context_param>,
            ) -> ::eyre::Result<()> {
                self.load_generated_text_sections(input)
            }
        }
    };

    Ok(quote! {
        #[derive(Debug, Clone, ::vihaco::Instruction)]
        pub enum #machine_instruction_ident #enum_generics {
            #( #machine_instruction_variants ),*
        }

        impl #impl_generics ::vihaco::__private::GeneratedMachine for #ident #ty_generics #where_clause {
            type Instruction = #machine_instruction_ident #enum_ty_generics;

            fn metadata(&self) -> ::vihaco::CompositeMetadata {
                static DEVICES: &[::vihaco::metadata::DeviceMetadata] = &[
                    #( #device_entries ),*
                ];
                static SOURCE_SYMBOL_ALIASES: &[::vihaco::metadata::SourceSymbolAliasMetadata] = &[
                    #( #source_symbol_alias_entries ),*
                ];
                ::vihaco::CompositeMetadata {
                    devices: DEVICES,
                    source_symbol_aliases: SOURCE_SYMBOL_ALIASES,
                }
            }
        }

        #program_impl
        #loadable_impl
        #text_loadable_impl
    })
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Token};

struct DeviceArgs {
    code: u8,
    aliases: Vec<syn::LitStr>,
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

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let data = match input.data {
        Data::Struct(data) => data,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring can only be generated for structs",
            )
            .into_compile_error()
            .into();
        }
    };
    let fields = match data.fields {
        Fields::Named(fields) => fields.named,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring requires a struct with named fields",
            )
            .into_compile_error()
            .into();
        }
    };

    let mut devices = Vec::new();
    let mut program_field: Option<(syn::Ident, syn::Type)> = None;

    for field in fields {
        let field_ident = field.ident.expect("named field");
        let field_ty = field.ty;
        for attr in &field.attrs {
            if attr.path().is_ident("program") {
                program_field = Some((field_ident.clone(), field_ty.clone()));
            }
        }

        for attr in field.attrs {
            if attr.path().is_ident("device") {
                let args = match attr.parse_args::<DeviceArgs>() {
                    Ok(args) => args,
                    Err(err) => return err.into_compile_error().into(),
                };
                devices.push((field_ident.clone(), field_ty.clone(), args));
            }
        }
    }

    let mut seen_device_codes = BTreeMap::<u8, syn::Ident>::new();
    for (field, _, args) in &devices {
        if let Some(existing) = seen_device_codes.insert(args.code, field.clone()) {
            return syn::Error::new(
                field.span(),
                format!(
                    "duplicate device code 0x{:02X} for fields `{}` and `{}`",
                    args.code, existing, field
                ),
            )
            .into_compile_error()
            .into();
        }
    }

    let mut seen_source_symbols = BTreeMap::<String, syn::Ident>::new();
    for (field, _, args) in &devices {
        let field_name = field.to_string();
        if let Some(existing) = seen_source_symbols.insert(field_name.clone(), field.clone()) {
            return syn::Error::new(
                field.span(),
                format!(
                    "duplicate source symbol `{}` for `{}` and `{}`",
                    field_name, existing, field
                ),
            )
            .into_compile_error()
            .into();
        }

        let mut local_aliases = BTreeSet::new();
        for alias in &args.aliases {
            let alias_name = alias.value();
            if !local_aliases.insert(alias_name.clone()) {
                return syn::Error::new(
                    alias.span(),
                    format!("duplicate alias `{}` on field `{}`", alias_name, field),
                )
                .into_compile_error()
                .into();
            }
            if let Some(existing) = seen_source_symbols.insert(alias_name.clone(), field.clone()) {
                return syn::Error::new(
                    alias.span(),
                    format!(
                        "duplicate source symbol `{}` for `{}` and `{}`",
                        alias_name, existing, field
                    ),
                )
                .into_compile_error()
                .into();
            }
        }
    }

    let machine_instruction_ident = format_ident!("{}Instruction", ident);

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
            impl ::vihaco::traits::ProgramCounter for #ident {
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

    quote! {
        #[derive(Debug, Clone, ::vihaco::Instruction)]
        pub enum #machine_instruction_ident {
            #( #machine_instruction_variants ),*
        }

        impl ::vihaco::__private::GeneratedMachine for #ident {
            type Instruction = #machine_instruction_ident;

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
    }
    .into()
}

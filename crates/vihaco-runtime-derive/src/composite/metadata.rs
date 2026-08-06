// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Generics, Ident, LitStr};

use super::syntax::RouteDeclaration;
use super::validate::FieldMetadata;

pub(super) fn generate_metadata(
    root: &TokenStream2,
    name: &Ident,
    generics: &Generics,
    instruction_ident: &Ident,
    routes: &[RouteDeclaration],
    fields: &[FieldMetadata],
) -> TokenStream2 {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let instruction_type = if routes.is_empty() {
        quote! { () }
    } else {
        let enum_generics = super::codegen::retained_enum_generics(generics, routes);
        let (_, enum_ty_generics, _) = enum_generics.split_for_impl();
        quote! { #instruction_ident #enum_ty_generics }
    };
    let devices = fields.iter().filter_map(|field| {
        field.device.as_ref().map(|device| {
            let code = device.code;
            let name = LitStr::new(&field.ident.to_string(), field.ident.span());
            quote! {
                #root::metadata::DeviceMetadata { code: #code, name: #name }
            }
        })
    });
    let aliases = fields.iter().flat_map(|field| {
        let code = field.device.as_ref().map(|device| device.code);
        let field_aliases = field
            .device
            .as_ref()
            .into_iter()
            .flat_map(move |device| device.aliases.iter().map(move |alias| (code, alias)));
        field_aliases.map(move |(code, alias)| {
            let code = code.expect("device aliases have a device");
            quote! {
                #root::metadata::SourceSymbolAliasMetadata {
                    name: #alias,
                    device_code: #code,
                }
            }
        })
    });

    quote! {
        impl #impl_generics #root::__private::GeneratedMachine for #name #ty_generics #where_clause {
            type Instruction = #instruction_type;

            fn metadata(&self) -> #root::CompositeMetadata {
                static DEVICES: &[#root::metadata::DeviceMetadata] = &[ #( #devices ),* ];
                static SOURCE_SYMBOL_ALIASES:
                    &[#root::metadata::SourceSymbolAliasMetadata] = &[ #( #aliases ),* ];
                #root::CompositeMetadata {
                    devices: DEVICES,
                    source_symbol_aliases: SOURCE_SYMBOL_ALIASES,
                }
            }
        }
    }
}

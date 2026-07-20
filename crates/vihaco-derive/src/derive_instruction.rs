// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::BTreeSet;
use syn::{DataEnum, DeriveInput, Fields};

use crate::common::{
    ensure_supported_variant_fields, enum_data, instruction_attrs, variant_opcodes,
};

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    if let Err(err) = ensure_supported_variant_fields(&input) {
        return err.into_compile_error().into();
    }

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let width_override = match instruction_attrs(&input) {
        Ok(width) => width,
        Err(err) => return err.into_compile_error().into(),
    };
    let opcodes = match variant_opcodes(&input) {
        Ok(opcodes) => opcodes,
        Err(err) => return err.into_compile_error().into(),
    };
    let data = match enum_data(&input) {
        Ok(data) => data,
        Err(err) => return err.into_compile_error().into(),
    };
    let byte_width_generics = generics_with_field_bounds(
        &input.generics,
        data,
        quote! { ::vihaco::instruction::ByteWidth },
    );
    let (byte_width_impl_generics, _, byte_width_where_clause) =
        byte_width_generics.split_for_impl();
    // TODO: if we have an explicit width override, then we don't need the entire where clause collected from
    // the variant fields
    let from_generics = generics_with_field_bounds(
        &input.generics,
        data,
        quote! {
            ::vihaco::instruction::ByteWidth + ::vihaco::instruction::FromBytes
        },
    );
    let (from_impl_generics, _, from_where_clause) = from_generics.split_for_impl();
    let write_generics = generics_with_field_bounds(
        &input.generics,
        data,
        quote! {
            ::vihaco::instruction::ByteWidth + ::vihaco::instruction::WriteBytes
        },
    );
    let (write_impl_generics, _, write_where_clause) = write_generics.split_for_impl();
    let read_param = fresh_type_param(&input.generics, "__vihaco_read");
    let write_param = fresh_type_param(&input.generics, "__vihaco_write");

    let opcode_arms = data
        .variants
        .iter()
        .zip(opcodes.iter())
        .map(|(variant, opcode)| {
            let variant_ident = &variant.ident;
            match &variant.fields {
                Fields::Unit => quote! { Self::#variant_ident => #opcode, },
                Fields::Unnamed(_) => quote! { Self::#variant_ident(..) => #opcode, },
                Fields::Named(_) => unreachable!(),
            }
        });

    let width_terms = data.variants.iter().map(|variant| match &variant.fields {
        Fields::Unit => quote! { 1u32 },
        Fields::Unnamed(fields) => {
            let field_widths = fields.unnamed.iter().map(|field| {
                let ty = &field.ty;
                quote! { <#ty as ::vihaco::instruction::ByteWidth>::width() }
            });
            quote! { 1u32 #( + #field_widths )* }
        }
        Fields::Named(_) => unreachable!(),
    });

    let from_arms = data
        .variants
        .iter()
        .zip(opcodes.iter())
        .map(|(variant, opcode)| {
            let variant_ident = &variant.ident;
            match &variant.fields {
                Fields::Unit => quote! { #opcode => Ok(Self::#variant_ident), },
                Fields::Unnamed(fields) => {
                    let field_reads = fields.unnamed.iter().map(|field| {
                        let ty = &field.ty;
                        quote! {
                            <#ty as ::vihaco::instruction::FromBytes>::from_bytes(&mut cursor)?
                        }
                    });
                    quote! {
                        #opcode => Ok(Self::#variant_ident(
                            #( #field_reads ),*
                        )),
                    }
                }
                Fields::Named(_) => unreachable!(),
            }
        });

    let write_arms = data.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        match &variant.fields {
            Fields::Unit => quote! { Self::#variant_ident => {}, },
            Fields::Unnamed(fields) => {
                let bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|index| {
                        syn::Ident::new(
                            &format!("__vihaco_field_{index}"),
                            proc_macro2::Span::call_site(),
                        )
                    })
                    .collect();
                let writes = bindings.iter().map(|binding| {
                    quote! {
                        #binding.write_bytes(&mut payload)?;
                    }
                });
                quote! {
                    Self::#variant_ident( #( #bindings ),* ) => {
                        #( #writes )*
                    },
                }
            }
            Fields::Named(_) => unreachable!(),
        }
    });

    let width_impl = if let Some(width) = width_override {
        quote! { #width }
    } else {
        quote! {
            let mut width = 1u32;
            #( width = width.max(#width_terms); )*
            width
        }
    };

    quote! {
        impl #impl_generics ::vihaco::instruction::OpCode for #ident #ty_generics #where_clause {
            fn opcode(&self) -> u8 {
                match self {
                    #( #opcode_arms )*
                }
            }
        }

        impl #byte_width_impl_generics ::vihaco::instruction::ByteWidth for #ident #ty_generics #byte_width_where_clause {
            fn width() -> u32 {
                #width_impl
            }
        }

        impl #from_impl_generics ::vihaco::instruction::FromBytesWithOpcode for #ident #ty_generics #from_where_clause {
            fn from_bytes_with_opcode<#read_param: ::std::io::Read>(
                bytes: &mut #read_param,
                opcode: u8,
            ) -> ::eyre::Result<Self> {
                let mut buf = vec![0u8; (<Self as ::vihaco::instruction::ByteWidth>::width() - 1) as usize];
                ::std::io::Read::read_exact(bytes, &mut buf)?;
                let mut cursor = ::std::io::Cursor::new(buf);
                match opcode {
                    #( #from_arms )*
                    _ => Err(::eyre::eyre!("invalid opcode {}", opcode)),
                }
            }
        }

        impl #write_impl_generics ::vihaco::instruction::WriteBytes for #ident #ty_generics #write_where_clause {
            fn write_bytes<#write_param: ::std::io::Write>(&self, io: &mut #write_param) -> ::eyre::Result<()> {
                let mut payload = Vec::new();
                match self {
                    #( #write_arms )*
                }
                let total_width = <Self as ::vihaco::instruction::ByteWidth>::width() as usize;
                let mut buf = vec![0u8; total_width];
                buf[0] = <Self as ::vihaco::instruction::OpCode>::opcode(self);
                let payload_len = payload.len().min(total_width.saturating_sub(1));
                buf[1..1 + payload_len].copy_from_slice(&payload[..payload_len]);
                io.write_all(&buf)?;
                Ok(())
            }
        }
    }
    .into()
}

fn generics_with_field_bounds(
    generics: &syn::Generics,
    data: &DataEnum,
    bounds: TokenStream2,
) -> syn::Generics {
    let mut generics = generics.clone();
    let where_clause = generics.make_where_clause();
    for variant in &data.variants {
        if let Fields::Unnamed(fields) = &variant.fields {
            for field in &fields.unnamed {
                let ty = &field.ty;
                where_clause.predicates.push(syn::parse_quote! {
                    #ty: #bounds
                });
            }
        }
    }
    generics
}

fn fresh_type_param(generics: &syn::Generics, base: &str) -> syn::Ident {
    let used: BTreeSet<String> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(param) => Some(param.ident.to_string()),
            syn::GenericParam::Const(param) => Some(param.ident.to_string()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect();

    if !used.contains(base) {
        return syn::Ident::new(base, proc_macro2::Span::call_site());
    }

    for suffix in 0usize.. {
        let candidate = format!("{base}_{suffix}");
        if !used.contains(&candidate) {
            return syn::Ident::new(&candidate, proc_macro2::Span::call_site());
        }
    }

    unreachable!("unbounded fresh type parameter search")
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{DeriveInput, Fields, Ident};

use crate::common::{
    ensure_supported_variant_fields, enum_data, instruction_attrs, variant_opcodes,
};

/// Resolve the crate root that generated code should be rooted at.
///
/// A downstream crate might depend on the `vihaco` facade *or* directly on
/// `vihaco-abi`; either way the generated `::<root>::instruction::…` paths must
/// resolve. We honour an explicit `#[vihaco(crate = ::some::path)]` override
/// first, then probe (via `proc-macro-crate`) for the facade and then the abi
/// crate, emitting `crate` when the derive is used inside the trait crate itself.
fn resolve_root(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if let Some(path) = crate_override(input)? {
        return Ok(quote! { #path });
    }

    for name in ["vihaco", "vihaco-abi"] {
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

    // Neither the facade nor the abi crate is a visible dependency; fall back to
    // the abi crate's canonical extern name so a clear resolution error is
    // produced rather than an opaque one.
    Ok(quote! { ::vihaco_abi })
}

/// Parse an optional `#[vihaco(crate = ::path)]` override on the deriving type.
fn crate_override(input: &DeriveInput) -> syn::Result<Option<syn::Path>> {
    let mut found = None;
    for attr in &input.attrs {
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

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    if let Err(err) = ensure_supported_variant_fields(&input) {
        return err.into_compile_error().into();
    }

    let root = match resolve_root(&input) {
        Ok(root) => root,
        Err(err) => return err.into_compile_error().into(),
    };

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
                quote! { <#ty as #root::instruction::OpCode>::width() }
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
                            <#ty as #root::instruction::FromBytes>::from_bytes(&mut cursor)?
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
                        syn::Ident::new(&format!("field_{index}"), proc_macro2::Span::call_site())
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
        impl #impl_generics #root::instruction::OpCode for #ident #ty_generics #where_clause {
            fn width() -> u32 {
                #width_impl
            }

            fn opcode(&self) -> u8 {
                match self {
                    #( #opcode_arms )*
                }
            }
        }

        impl #impl_generics #root::instruction::FromBytesWithOpcode for #ident #ty_generics #where_clause {
            fn from_bytes_with_opcode<R: ::std::io::Read>(
                bytes: &mut R,
                opcode: u8,
            ) -> ::eyre::Result<Self> {
                let mut buf = vec![0u8; (<Self as #root::instruction::OpCode>::width() - 1) as usize];
                ::std::io::Read::read_exact(bytes, &mut buf)?;
                let mut cursor = ::std::io::Cursor::new(buf);
                match opcode {
                    #( #from_arms )*
                    _ => Err(::eyre::eyre!("invalid opcode {}", opcode)),
                }
            }
        }

        impl #impl_generics #root::instruction::WriteBytes for #ident #ty_generics #where_clause {
            fn write_bytes<W: ::std::io::Write>(&self, io: &mut W) -> ::eyre::Result<()> {
                let mut payload = Vec::new();
                match self {
                    #( #write_arms )*
                }
                let total_width = <Self as #root::instruction::OpCode>::width() as usize;
                let mut buf = vec![0u8; total_width];
                buf[0] = <Self as #root::instruction::OpCode>::opcode(self);
                let payload_len = payload.len().min(total_width.saturating_sub(1));
                buf[1..1 + payload_len].copy_from_slice(&payload[..payload_len]);
                io.write_all(&buf)?;
                Ok(())
            }
        }
    }
    .into()
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use syn::parse::{Parse, ParseStream};
use syn::{DataEnum, DeriveInput, Error, Fields, LitInt, Token, Variant};

pub fn enum_data(input: &DeriveInput) -> syn::Result<&DataEnum> {
    match &input.data {
        syn::Data::Enum(data) => Ok(data),
        _ => Err(Error::new_spanned(input, "expected an enum")),
    }
}

pub fn ensure_supported_variant_fields(input: &DeriveInput) -> syn::Result<()> {
    let data = enum_data(input)?;
    for variant in &data.variants {
        match &variant.fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) if !fields.unnamed.is_empty() => {}
            _ => {
                return Err(Error::new_spanned(
                    &variant.fields,
                    "only unit variants and tuple variants are supported in this slice",
                ));
            }
        }
    }
    Ok(())
}

pub fn variant_opcode(variant: &Variant, fallback: u8) -> syn::Result<u8> {
    let mut opcode = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("opcode") {
            continue;
        }

        let expr = match &attr.meta {
            syn::Meta::NameValue(name_value) => name_value.value.clone(),
            _ => {
                return Err(Error::new_spanned(
                    attr,
                    "opcode must use #[opcode = ...] syntax",
                ));
            }
        };
        let value = match expr {
            syn::Expr::Lit(expr_lit) => match expr_lit.lit {
                syn::Lit::Int(lit) => lit.base10_parse::<u8>()?,
                other => {
                    return Err(Error::new_spanned(
                        other,
                        "opcode must be an integer literal",
                    ));
                }
            },
            other => {
                return Err(Error::new_spanned(
                    other,
                    "opcode must be an integer literal",
                ));
            }
        };

        if opcode.replace(value).is_some() {
            return Err(Error::new_spanned(
                attr,
                "duplicate #[opcode = ...] attribute",
            ));
        }
    }

    Ok(opcode.unwrap_or(fallback))
}

pub fn variant_opcodes(input: &DeriveInput) -> syn::Result<Vec<u8>> {
    let data = enum_data(input)?;
    let mut seen = BTreeSet::new();
    let mut opcodes = Vec::with_capacity(data.variants.len());
    for (index, variant) in data.variants.iter().enumerate() {
        let opcode = variant_opcode(variant, index as u8)?;
        if !seen.insert(opcode) {
            return Err(Error::new_spanned(
                variant,
                format!("duplicate opcode 0x{opcode:02X}"),
            ));
        }
        opcodes.push(opcode);
    }
    Ok(opcodes)
}

pub struct InstructionArgs {
    pub width: Option<u32>,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut width = None;
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_str() {
                "width" => {
                    input.parse::<Token![=]>()?;
                    let lit: LitInt = input.parse()?;
                    width = Some(lit.base10_parse()?);
                }
                _ => {
                    return Err(Error::new_spanned(
                        ident,
                        "unsupported instruction argument",
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { width })
    }
}

pub fn instruction_attrs(input: &DeriveInput) -> syn::Result<Option<u32>> {
    let mut width = None;
    for attr in &input.attrs {
        if attr.path().is_ident("instruction") {
            let args = attr.parse_args::<InstructionArgs>()?;
            width = args.width.filter(|w| *w > 0).or(width);
        }
    }
    Ok(width)
}

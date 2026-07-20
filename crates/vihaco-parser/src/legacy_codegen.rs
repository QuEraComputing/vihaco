// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Compatibility code generation for the original token/delimiter syntax.

use crate::attr::{DelimiterAttrs, EnumAttrs, FieldAttrs, HeadAttr, VariantAttrs};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Attribute, DataEnum, Error, Fields, Generics, Ident, Result, Type};

pub(crate) fn expand_enum(
    data: &DataEnum,
    enum_ident: &Ident,
    attrs: &[Attribute],
    generics: &Generics,
) -> Result<TokenStream> {
    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            generics,
            "legacy Parse syntax does not support generic enums; add #[syntax_class(...)]",
        ));
    }

    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            enum_ident,
            "#[derive(Parse)] requires at least one variant",
        ));
    }

    let enum_attrs = EnumAttrs::from_attrs(attrs)?;
    let head_prefix = match &enum_attrs.head {
        None => None,
        Some(HeadAttr::Auto) => Some(format!("{enum_ident}::")),
        Some(HeadAttr::Custom(value)) => Some(value.clone()),
    };

    let mut variant_data = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let attrs = VariantAttrs::from_variant(variant)?;
        let field_attrs = variant
            .fields
            .iter()
            .map(FieldAttrs::from_field)
            .collect::<Result<Vec<_>>>()?;

        if attrs.delegate && field_attrs.iter().any(|attr| attr.parse_with.is_some()) {
            return Err(Error::new(
                attrs.delegate_span.expect("delegate has a span"),
                "#[delegate] cannot be combined with #[parse_with] on a field",
            ));
        }

        let token = compute_token(
            head_prefix.as_deref(),
            &variant.ident.to_string(),
            attrs.token.as_deref(),
        );
        variant_data.push((token, attrs, field_attrs));
    }

    let mut found_delegate = false;
    for (_, attrs, _) in &variant_data {
        if attrs.delegate {
            found_delegate = true;
        } else if found_delegate {
            return Err(Error::new_spanned(
                enum_ident,
                "#[delegate] variants must be declared after all token-bearing variants",
            ));
        }
    }

    let tokens = variant_data
        .iter()
        .filter(|(_, attrs, _)| !attrs.delegate)
        .map(|(token, _, _)| token.as_str())
        .collect::<Vec<_>>();
    if let Err((earlier, later)) = check_prefix_order(&tokens) {
        return Err(Error::new_spanned(
            enum_ident,
            format!(
                "token `{}` is a prefix of `{}` declared after it — reorder so longer tokens come first",
                tokens[earlier], tokens[later]
            ),
        ));
    }

    let variant_bindings = data
        .variants
        .iter()
        .zip(&variant_data)
        .map(|(variant, (token, attrs, field_attrs))| {
            generate_variant_parser(&VariantParserInput {
                enum_ident,
                variant_ident: &variant.ident,
                token,
                fields: &variant.fields,
                field_parse_withs: field_attrs
                    .iter()
                    .map(|attr| attr.parse_with.clone())
                    .collect(),
                delimiters: &attrs.delimiters,
                delegate: attrs.delegate,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let names = data
        .variants
        .iter()
        .map(|variant| format_ident!("variant_{}", variant.ident.to_string().to_lowercase()))
        .collect::<Vec<_>>();
    let alternatives = if names.len() == 1 {
        let only = &names[0];
        quote! { #only }
    } else if names.len() <= 26 {
        quote! { ::chumsky::primitive::choice((#(#names),*)) }
    } else {
        let chunks = names
            .chunks(26)
            .map(|chunk| quote! { ::chumsky::primitive::choice((#(#chunk),*)).boxed() })
            .collect::<Vec<_>>();
        let first = &chunks[0];
        chunks[1..].iter().fold(
            quote! { #first },
            |result, chunk| quote! { #result.or(#chunk) },
        )
    };

    let ws = if data
        .variants
        .iter()
        .any(|variant| !matches!(variant.fields, Fields::Unit))
    {
        quote! {
            let ws = ::chumsky::text::whitespace::<
                &'src str,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            >();
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl<'src> ::vihaco_parser_core::Parse<'src> for #enum_ident {
            fn parser() -> impl ::chumsky::Parser<
                'src,
                &'src str,
                Self,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            > {
                use ::chumsky::Parser as _;
                #ws
                #(#variant_bindings)*
                #alternatives
            }
        }
    }
    .into())
}

struct VariantParserInput<'a> {
    enum_ident: &'a Ident,
    variant_ident: &'a Ident,
    token: &'a str,
    fields: &'a Fields,
    field_parse_withs: Vec<Option<String>>,
    delimiters: &'a DelimiterAttrs,
    delegate: bool,
}

fn generate_variant_parser(input: &VariantParserInput<'_>) -> Result<TokenStream2> {
    let enum_ident = input.enum_ident;
    let variant_ident = input.variant_ident;
    let token = input.token;
    let name = format_ident!("variant_{}", variant_ident.to_string().to_lowercase());

    if input.delegate {
        let Fields::Unnamed(fields) = input.fields else {
            unreachable!("delegate validation requires one tuple field");
        };
        let inner = &fields.unnamed[0].ty;
        return Ok(quote! {
            let #name = <#inner as ::vihaco_parser_core::Parse>::parser()
                .map(#enum_ident::#variant_ident);
        });
    }

    if matches!(input.fields, Fields::Unit) {
        return Ok(quote! {
            let #name = ::chumsky::primitive::just(#token)
                .map(|_| #enum_ident::#variant_ident);
        });
    }

    let Fields::Unnamed(fields) = input.fields else {
        unreachable!("named variants are rejected during attribute validation");
    };
    let parsers = fields
        .unnamed
        .iter()
        .zip(&input.field_parse_withs)
        .map(|(field, parse_with)| field_parser_expr(&field.ty, parse_with.as_deref()))
        .collect::<Result<Vec<_>>>()?;
    let (chain, pattern, bindings) = build_field_chain(&parsers, &input.delimiters.separator);
    let open = delimiter_expr(&input.delimiters.open);
    let close = delimiter_expr(&input.delimiters.close);
    let trailing_ws = if input.delimiters.close.is_empty() {
        quote! {}
    } else {
        quote! { .then_ignore(ws.clone()) }
    };
    let map = if bindings.len() == 1 {
        let binding = &bindings[0];
        quote! { .map(|#binding| #enum_ident::#variant_ident(#binding)) }
    } else {
        quote! { .map(|#pattern| #enum_ident::#variant_ident(#(#bindings),*)) }
    };

    Ok(quote! {
        let #name = ::chumsky::primitive::just(#token)
            .ignore_then(ws.clone())
            .ignore_then(#open)
            .ignore_then(#chain)
            #trailing_ws
            .then_ignore(#close)
            #map;
    })
}

fn build_field_chain(
    parsers: &[TokenStream2],
    separator: &str,
) -> (TokenStream2, TokenStream2, Vec<Ident>) {
    let separator = if !separator.is_empty() && separator.chars().all(char::is_whitespace) {
        quote! {
            ::chumsky::text::whitespace::<
                &'src str,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            >().at_least(1)
        }
    } else {
        let separator = delimiter_expr(separator);
        quote! { #separator.padded() }
    };
    let bindings = (0..parsers.len())
        .map(|index| format_ident!("__vihaco_field_{index}"))
        .collect::<Vec<_>>();

    if parsers.len() == 1 {
        return (parsers[0].clone(), quote! { #(#bindings)* }, bindings);
    }

    let mut chain = {
        let first = &parsers[0];
        let second = &parsers[1];
        quote! { #first.then_ignore(#separator).then(#second) }
    };
    for parser in &parsers[2..] {
        chain = quote! { #chain.then_ignore(#separator).then(#parser) };
    }

    let mut pattern = {
        let first = &bindings[0];
        let second = &bindings[1];
        quote! { (#first, #second) }
    };
    for binding in &bindings[2..] {
        pattern = quote! { (#pattern, #binding) };
    }
    (chain, pattern, bindings)
}

fn delimiter_expr(value: &str) -> TokenStream2 {
    if value.is_empty() {
        quote! { ::chumsky::primitive::empty() }
    } else if value.chars().count() == 1 {
        let character = value.chars().next().expect("one character");
        quote! { ::chumsky::primitive::just(#character) }
    } else {
        quote! { ::chumsky::primitive::just(#value) }
    }
}

fn field_parser_expr(ty: &Type, parse_with: Option<&str>) -> Result<TokenStream2> {
    if let Some(path) = parse_with {
        let parser: TokenStream2 = path.parse().map_err(|error| {
            Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid parse_with path `{path}`: {error}"),
            )
        })?;
        Ok(quote! { #parser() })
    } else {
        Ok(quote! { <#ty as ::vihaco_parser_core::Parse>::parser() })
    }
}

fn compute_token(head: Option<&str>, variant: &str, custom: Option<&str>) -> String {
    let token = custom.map(str::to_owned).unwrap_or_else(|| {
        if head.is_some() {
            variant.to_owned()
        } else {
            variant.to_lowercase()
        }
    });
    head.map_or(token.clone(), |head| format!("{head}{token}"))
}

fn check_prefix_order(tokens: &[&str]) -> std::result::Result<(), (usize, usize)> {
    for (earlier_index, earlier) in tokens.iter().enumerate() {
        for (later_index, later) in tokens.iter().enumerate().skip(earlier_index + 1) {
            if later.starts_with(earlier) && later.len() > earlier.len() {
                return Err((earlier_index, later_index));
            }
        }
    }
    Ok(())
}

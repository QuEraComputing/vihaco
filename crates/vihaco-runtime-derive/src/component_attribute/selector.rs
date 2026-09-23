// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Attribute, Ident, Result, Token, braced,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub(super) enum Selection {
    All,
    Names(Vec<Ident>),
}

pub(super) struct Selector {
    pub(super) dialect: TokenStream2,
    key: String,
    pub(super) selection: Selection,
    span: Span,
}

impl Parse for Selector {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let leading = if input.peek(Token![::]) {
            Some(input.parse::<Token![::]>()?)
        } else {
            None
        };
        let mut segments = vec![input.call(Ident::parse_any)?];
        let mut selection = None;

        while input.peek(Token![::]) {
            input.parse::<Token![::]>()?;
            if input.peek(Token![*]) {
                input.parse::<Token![*]>()?;
                selection = Some(Selection::All);
                break;
            }
            if input.peek(syn::token::Brace) {
                let content;
                braced!(content in input);
                let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                if names.is_empty() {
                    return Err(content.error("instruction group must not be empty"));
                }
                selection = Some(Selection::Names(names.into_iter().collect()));
                break;
            }
            segments.push(input.call(Ident::parse_any)?);
        }

        let selection = match selection {
            Some(selection) => selection,
            None if segments.len() > 1 => {
                Selection::Names(vec![segments.pop().expect("multiple segments")])
            }
            None => {
                return Err(syn::Error::new(
                    segments[0].span(),
                    "expected a dialect path followed by `::*`, `::{...}`, or `::Instruction`",
                ));
            }
        };
        let span = segments[0].span();
        let key = format!(
            "{}{}",
            if leading.is_some() { "::" } else { "" },
            segments
                .iter()
                .map(Ident::to_string)
                .collect::<Vec<_>>()
                .join("::")
        );
        let dialect = if segments.len() == 1 {
            quote! { #leading #(#segments)* }
        } else {
            let first = &segments[0];
            let rest = &segments[1..];
            quote! { #leading #first #( :: #rest )* }
        };
        Ok(Self {
            dialect,
            key,
            selection,
            span,
        })
    }
}

pub(super) fn instructions(attrs: &mut Vec<Attribute>) -> Result<Vec<Selector>> {
    let mut found = None;
    attrs.retain(|attr| {
        if attr.path().is_ident("instructions") {
            if found.is_some() {
                found = Some(Err(syn::Error::new_spanned(
                    attr,
                    "duplicate `instructions` attribute",
                )));
            } else {
                found = Some(
                    attr.parse_args_with(Punctuated::<Selector, Token![,]>::parse_terminated)
                        .map(|items| items.into_iter().collect()),
                );
            }
            false
        } else {
            true
        }
    });
    let selectors: Vec<Selector> = found
        .ok_or_else(|| syn::Error::new(Span::call_site(), "missing `instructions` attribute"))??;
    if selectors.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "instruction list must not be empty",
        ));
    }

    let mut seen: HashMap<&str, (bool, HashSet<String>)> = HashMap::new();
    for selector in &selectors {
        let (wildcard, names) = seen.entry(&selector.key).or_default();
        match &selector.selection {
            Selection::All => {
                if *wildcard || !names.is_empty() {
                    return Err(syn::Error::new(
                        selector.span,
                        "duplicate or overlapping instruction selector",
                    ));
                }
                *wildcard = true;
            }
            Selection::Names(selected) => {
                if *wildcard {
                    return Err(syn::Error::new(
                        selector.span,
                        "duplicate or overlapping instruction selector",
                    ));
                }
                for name in selected {
                    if !names.insert(name.to_string()) {
                        return Err(syn::Error::new(
                            name.span(),
                            "duplicate instruction selector",
                        ));
                    }
                }
            }
        }
    }
    Ok(selectors)
}

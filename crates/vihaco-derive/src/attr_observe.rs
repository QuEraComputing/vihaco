// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::ToTokens;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    GenericArgument, ImplItem, ItemImpl, Path, PathArguments, ReturnType, Token, Type,
    parse_macro_input,
};

struct ObserveEntry {
    event_type: syn::Path,
}

struct ObserveArgs {
    entries: Vec<ObserveEntry>,
    composite_effect: Option<Type>,
}

impl Parse for ObserveArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut entries = Vec::new();
        let mut composite_effect = None;

        while !input.is_empty() {
            if input.peek(syn::Ident) {
                let lookahead = input.fork();
                let ident: syn::Ident = lookahead.parse()?;
                if lookahead.peek(Token![=]) {
                    if ident == "snapshot" {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "snapshot = ... is no longer supported in #[observe]",
                        ));
                    }
                    if ident != "effect" {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "unsupported #[observe] metadata; expected `effect = ...`",
                        ));
                    }
                    if composite_effect.is_some() {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "duplicate `effect = ...` in #[observe]",
                        ));
                    }

                    input.parse::<syn::Ident>()?;
                    input.parse::<Token![=]>()?;
                    composite_effect = Some(input.parse()?);
                } else {
                    entries.push(ObserveEntry {
                        event_type: input.parse()?,
                    });
                }
            } else {
                entries.push(ObserveEntry {
                    event_type: input.parse()?,
                });
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        if entries.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[observe] requires at least one effect type",
            ));
        }

        Ok(Self {
            entries,
            composite_effect,
        })
    }
}

fn types_equivalent(lhs: &Type, rhs: &Type) -> bool {
    match (lhs, rhs) {
        (Type::Array(lhs), Type::Array(rhs)) => {
            expr_tokens_eq(&lhs.len, &rhs.len) && types_equivalent(&lhs.elem, &rhs.elem)
        }
        (Type::Group(lhs), rhs) => types_equivalent(&lhs.elem, rhs),
        (lhs, Type::Group(rhs)) => types_equivalent(lhs, &rhs.elem),
        (Type::Paren(lhs), rhs) => types_equivalent(&lhs.elem, rhs),
        (lhs, Type::Paren(rhs)) => types_equivalent(lhs, &rhs.elem),
        (Type::Path(lhs), Type::Path(rhs)) => paths_equivalent(&lhs.path, &rhs.path),
        (Type::Ptr(lhs), Type::Ptr(rhs)) => {
            lhs.mutability.is_some() == rhs.mutability.is_some()
                && types_equivalent(&lhs.elem, &rhs.elem)
        }
        (Type::Reference(lhs), Type::Reference(rhs)) => {
            lhs.mutability.is_some() == rhs.mutability.is_some()
                && lifetimes_equivalent(lhs.lifetime.as_ref(), rhs.lifetime.as_ref())
                && types_equivalent(&lhs.elem, &rhs.elem)
        }
        (Type::Slice(lhs), Type::Slice(rhs)) => types_equivalent(&lhs.elem, &rhs.elem),
        (Type::Tuple(lhs), Type::Tuple(rhs)) => {
            lhs.elems.len() == rhs.elems.len()
                && lhs
                    .elems
                    .iter()
                    .zip(rhs.elems.iter())
                    .all(|(lhs, rhs)| types_equivalent(lhs, rhs))
        }
        _ => lhs.to_token_stream().to_string() == rhs.to_token_stream().to_string(),
    }
}

fn paths_equivalent(lhs: &Path, rhs: &Path) -> bool {
    lhs.segments.len() == rhs.segments.len()
        && lhs
            .segments
            .iter()
            .zip(rhs.segments.iter())
            .all(|(lhs, rhs)| {
                lhs.ident == rhs.ident && path_arguments_equivalent(&lhs.arguments, &rhs.arguments)
            })
}

fn path_arguments_equivalent(lhs: &PathArguments, rhs: &PathArguments) -> bool {
    match (lhs, rhs) {
        (PathArguments::None, PathArguments::None) => true,
        (PathArguments::AngleBracketed(lhs), PathArguments::AngleBracketed(rhs)) => {
            lhs.args.len() == rhs.args.len()
                && lhs
                    .args
                    .iter()
                    .zip(rhs.args.iter())
                    .all(|(lhs, rhs)| generic_arguments_equivalent(lhs, rhs))
        }
        (PathArguments::Parenthesized(lhs), PathArguments::Parenthesized(rhs)) => {
            lhs.inputs.len() == rhs.inputs.len()
                && lhs
                    .inputs
                    .iter()
                    .zip(rhs.inputs.iter())
                    .all(|(lhs, rhs)| types_equivalent(lhs, rhs))
                && match (&lhs.output, &rhs.output) {
                    (ReturnType::Default, ReturnType::Default) => true,
                    (ReturnType::Type(_, lhs), ReturnType::Type(_, rhs)) => {
                        types_equivalent(lhs, rhs)
                    }
                    _ => false,
                }
        }
        _ => false,
    }
}

fn generic_arguments_equivalent(lhs: &GenericArgument, rhs: &GenericArgument) -> bool {
    match (lhs, rhs) {
        (GenericArgument::Type(lhs), GenericArgument::Type(rhs)) => types_equivalent(lhs, rhs),
        (GenericArgument::Lifetime(lhs), GenericArgument::Lifetime(rhs)) => lhs == rhs,
        (GenericArgument::Const(lhs), GenericArgument::Const(rhs)) => expr_tokens_eq(lhs, rhs),
        (GenericArgument::AssocType(lhs), GenericArgument::AssocType(rhs)) => {
            lhs.ident == rhs.ident && types_equivalent(&lhs.ty, &rhs.ty)
        }
        (GenericArgument::AssocConst(lhs), GenericArgument::AssocConst(rhs)) => {
            lhs.ident == rhs.ident && expr_tokens_eq(&lhs.value, &rhs.value)
        }
        (GenericArgument::Constraint(lhs), GenericArgument::Constraint(rhs)) => {
            lhs.ident == rhs.ident
                && lhs.bounds.len() == rhs.bounds.len()
                && lhs.bounds.iter().zip(rhs.bounds.iter()).all(|(lhs, rhs)| {
                    lhs.to_token_stream().to_string() == rhs.to_token_stream().to_string()
                })
        }
        _ => false,
    }
}

fn lifetimes_equivalent(lhs: Option<&syn::Lifetime>, rhs: Option<&syn::Lifetime>) -> bool {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        (None, None) => true,
        _ => false,
    }
}

fn expr_tokens_eq(lhs: &syn::Expr, rhs: &syn::Expr) -> bool {
    lhs.to_token_stream().to_string() == rhs.to_token_stream().to_string()
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ObserveArgs);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let self_ty = &item_impl.self_ty;

    let mut trait_impls = Vec::new();

    for entry in &args.entries {
        let event_path = &entry.event_type;

        // Get the last segment name for the method naming convention
        let event_name = event_path.segments.last().unwrap().ident.to_string();

        // Convert to snake_case prefix: observe_channel_frame for ChannelFrame
        let method_prefix = format!("observe_{}", event_name.to_case(Case::Snake));

        // Find all methods whose name matches or starts with the prefix
        let matching_methods: Vec<&syn::Ident> = item_impl
            .items
            .iter()
            .filter_map(|item| {
                if let ImplItem::Fn(f) = item {
                    let name = f.sig.ident.to_string();
                    if name == method_prefix || name.starts_with(&format!("{}_", method_prefix)) {
                        return Some(&f.sig.ident);
                    }
                }
                None
            })
            .collect();

        if matching_methods.is_empty() {
            return syn::Error::new_spanned(
                event_path,
                format!(
                    "missing handler method `{}` (or `{}_*`) for observed effect `{}`",
                    method_prefix, method_prefix, event_name
                ),
            )
            .into_compile_error()
            .into();
        }

        let expected_inputs = 2;
        let label = "handler";
        let params_desc = "&mut self, effect";

        let mut follow_up_calls = Vec::new();
        let mut effect_error_ty = None::<syn::Type>;
        let mut has_non_unit_follow_up = false;
        for method_ident in &matching_methods {
            if let Some(ImplItem::Fn(f)) = item_impl
                .items
                .iter()
                .find(|item| matches!(item, ImplItem::Fn(f) if &f.sig.ident == *method_ident))
            {
                if f.sig.inputs.len() != expected_inputs {
                    return syn::Error::new_spanned(
                        &f.sig,
                        format!(
                            "{} `{}` must have {} parameters: {}",
                            label, f.sig.ident, expected_inputs, params_desc
                        ),
                    )
                    .into_compile_error()
                    .into();
                }

                let ReturnType::Type(_, ty) = &f.sig.output else {
                    return syn::Error::new_spanned(
                        &f.sig,
                        "observer handlers must return Result<Effects<Effect>, Error>",
                    )
                    .into_compile_error()
                    .into();
                };
                let Type::Path(type_path) = ty.as_ref() else {
                    return syn::Error::new_spanned(
                        ty,
                        "observer handlers must return Result<Effects<Effect>, Error>",
                    )
                    .into_compile_error()
                    .into();
                };
                let Some(segment) = type_path.path.segments.last() else {
                    return syn::Error::new_spanned(
                        ty,
                        "observer handlers must return Result<Effects<Effect>, Error>",
                    )
                    .into_compile_error()
                    .into();
                };

                if segment.ident == "Result" {
                    let syn::PathArguments::AngleBracketed(result_args) = &segment.arguments else {
                        return syn::Error::new_spanned(
                            ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    };
                    if result_args.args.is_empty() || result_args.args.len() > 2 {
                        return syn::Error::new_spanned(
                            ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    }
                    let _inner_ty = match &result_args.args[0] {
                        syn::GenericArgument::Type(inner_ty) => inner_ty.clone(),
                        other => {
                            return syn::Error::new_spanned(
                                other,
                                "observer handlers must return Result<Effects<Effect>, Error>",
                            )
                            .into_compile_error()
                            .into();
                        }
                    };
                    let Type::Path(success_path) = &_inner_ty else {
                        return syn::Error::new_spanned(
                            &_inner_ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    };
                    let Some(success_segment) = success_path.path.segments.last() else {
                        return syn::Error::new_spanned(
                            &_inner_ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    };
                    if success_segment.ident != "Effects" {
                        return syn::Error::new_spanned(
                            &_inner_ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    }
                    let syn::PathArguments::AngleBracketed(success_args) =
                        &success_segment.arguments
                    else {
                        return syn::Error::new_spanned(
                            &_inner_ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    };
                    if success_args.args.len() != 1
                        || !matches!(
                            success_args.args.first(),
                            Some(syn::GenericArgument::Type(_))
                        )
                    {
                        return syn::Error::new_spanned(
                            &_inner_ty,
                            "observer handlers must return Result<Effects<Effect>, Error>",
                        )
                        .into_compile_error()
                        .into();
                    }
                    let local_effect_ty = match success_args.args.first() {
                        Some(syn::GenericArgument::Type(local_effect_ty)) => {
                            local_effect_ty.clone()
                        }
                        _ => unreachable!(),
                    };
                    let is_unit_follow_up =
                        matches!(&local_effect_ty, Type::Tuple(tuple) if tuple.elems.is_empty());
                    if !is_unit_follow_up {
                        has_non_unit_follow_up = true;
                    }
                    let error_ty = if result_args.args.len() == 1 {
                        syn::parse_quote!(::eyre::Report)
                    } else {
                        match &result_args.args[1] {
                            syn::GenericArgument::Type(error_ty) => error_ty.clone(),
                            other => {
                                return syn::Error::new_spanned(
                                    other,
                                    "observer handlers must return Result<Effects<Effect>, Error>",
                                )
                                .into_compile_error()
                                .into();
                            }
                        }
                    };
                    if let Some(existing_error_ty) = &effect_error_ty {
                        if !types_equivalent(existing_error_ty, &error_ty) {
                            return syn::Error::new_spanned(
                                &f.sig.output,
                                format!(
                                    "observer handlers for `{}` must use the same error type; expected `{}`",
                                    event_name,
                                    existing_error_ty.to_token_stream()
                                ),
                            )
                            .into_compile_error()
                            .into();
                        }
                    } else {
                        effect_error_ty = Some(error_ty.clone());
                    }
                    let follow_up_call = if let Some(composite_effect) = &args.composite_effect {
                        if is_unit_follow_up {
                            quote! {
                                let __follow_ups: ::vihaco::Effects<#local_effect_ty> =
                                    ::std::convert::Into::<::vihaco::Effects<_>>::into(self.#method_ident(effect)?);
                                for () in __follow_ups {}
                            }
                        } else {
                            quote! {
                                effects = effects.extend(
                                    ::std::convert::Into::<::vihaco::Effects<_>>::into(self.#method_ident(effect)?)
                                        .map(::std::convert::Into::<#composite_effect>::into)
                                );
                            }
                        }
                    } else {
                        quote! {
                            let __follow_ups: ::vihaco::Effects<#local_effect_ty> =
                                ::std::convert::Into::<::vihaco::Effects<_>>::into(self.#method_ident(effect)?);
                            for () in __follow_ups {}
                        }
                    };
                    follow_up_calls.push(follow_up_call);
                } else {
                    return syn::Error::new_spanned(
                        ty,
                        "observer handlers must return Result<Effects<Effect>, Error>",
                    )
                    .into_compile_error()
                    .into();
                }
            }
        }

        let error_ty =
            effect_error_ty.unwrap_or_else(|| syn::parse_quote!(::std::convert::Infallible));
        let is_composite_generated =
            args.entries.len() > 1 || matching_methods.len() > 1 || has_non_unit_follow_up;
        if is_composite_generated && args.composite_effect.is_none() {
            return syn::Error::new_spanned(
                event_path,
                "generated #[observe] impls that compose multiple observed events, multiple handlers, or typed follow-up effects must declare `effect = ...` in #[observe(...)]",
            )
            .into_compile_error()
            .into();
        }
        let generated_effect_ty = args
            .composite_effect
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(()));
        trait_impls.push(quote! {
            impl ::vihaco::Observe<#event_path> for #self_ty {
                type Effect = #generated_effect_ty;
                type Error = #error_ty;

                fn observe(
                    &mut self,
                    effect: &#event_path,
                ) -> ::std::result::Result<::vihaco::Effects<Self::Effect>, Self::Error> {
                    let mut effects = ::vihaco::Effects::none();
                    #( #follow_up_calls )*
                    Ok(effects)
                }
            }
        });
    }

    quote! {
        #item_impl
        #( #trait_impls )*
    }
    .into()
}

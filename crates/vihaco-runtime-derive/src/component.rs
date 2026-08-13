// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Fields, Generics, Ident, Result, Token, Type, Visibility, WhereClause};

syn::custom_keyword!(component);
syn::custom_keyword!(instruction);
syn::custom_keyword!(value);

struct Declaration {
    attrs: Vec<Attribute>,
    visibility: Visibility,
    name: Ident,
    generics: Generics,
    state: Fields,
    type_alias: Option<Alias>,
    value_alias: Option<Alias>,
    instructions: Option<Vec<Instruction>>,
}

struct Alias {
    visibility: Visibility,
    name: Ident,
    target: Type,
}

struct Instruction {
    attrs: Vec<Attribute>,
    name: Ident,
    fields: Vec<FieldMapping>,
}

struct FieldMapping {
    syntax: Type,
    runtime: Type,
}

impl Parse for Declaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let visibility = input.parse()?;
        input.parse::<component>()?;
        let name = input.parse()?;
        let mut generics: Generics = input.parse()?;
        generics.where_clause = input.parse::<Option<WhereClause>>()?;

        let state_content;
        syn::braced!(state_content in input);
        let state = Fields::Named(syn::FieldsNamed {
            brace_token: Default::default(),
            named: syn::punctuated::Punctuated::parse_terminated_with(
                &state_content,
                syn::Field::parse_named,
            )?,
        });

        let mut type_alias = None;
        let mut value_alias = None;
        let mut instructions = None;

        while !input.is_empty() {
            if input.peek(Token![type]) {
                input.parse::<Token![type]>()?;
                let alias = parse_alias(input)?;
                if type_alias.replace(alias).is_some() {
                    return Err(input.error("duplicate component type declaration"));
                }
            } else if input.peek(value) {
                input.parse::<value>()?;
                let alias = parse_alias(input)?;
                if value_alias.replace(alias).is_some() {
                    return Err(input.error("duplicate component value declaration"));
                }
            } else if input.peek(instruction) {
                input.parse::<instruction>()?;
                let content;
                syn::braced!(content in input);
                let parsed = parse_instructions(&content)?;
                if parsed.is_empty() {
                    return Err(content.error("component instruction block must not be empty"));
                }
                if instructions.replace(parsed).is_some() {
                    return Err(input.error("duplicate component instruction declaration"));
                }
            } else {
                return Err(input.error("expected `type`, `value`, or `instruction`"));
            }
        }

        Ok(Self {
            attrs,
            visibility,
            name,
            generics,
            state,
            type_alias,
            value_alias,
            instructions,
        })
    }
}

fn parse_alias(input: ParseStream<'_>) -> Result<Alias> {
    let visibility = input.parse()?;
    let name = input.parse()?;
    input.parse::<Token![=]>()?;
    let target = input.parse()?;
    input.parse::<Token![;]>()?;
    Ok(Alias {
        visibility,
        name,
        target,
    })
}

fn parse_instructions(input: ParseStream<'_>) -> Result<Vec<Instruction>> {
    let mut result = Vec::new();
    while !input.is_empty() {
        let attrs = Attribute::parse_outer(input)?;
        let name: Ident = input.parse()?;
        let mut fields = Vec::new();
        if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            while !content.is_empty() {
                let syntax: Type = content.parse()?;
                let runtime = if content.peek(Token![=>]) {
                    content.parse::<Token![=>]>()?;
                    content.parse()?
                } else {
                    syntax.clone()
                };
                fields.push(FieldMapping { syntax, runtime });
                if content.is_empty() {
                    break;
                }
                content.parse::<Token![,]>()?;
            }
        }
        result.push(Instruction {
            attrs,
            name,
            fields,
        });
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if !input.is_empty() {
            return Err(input.error("expected `,` between instruction variants"));
        }
    }
    Ok(result)
}

fn retain_generics(generics: &Generics, types: impl IntoIterator<Item = Type>) -> Generics {
    let tokens: Vec<_> = types.into_iter().map(|ty| quote! { #ty }).collect();
    let text = quote! { #(#tokens)* }.to_string();
    let mut result = generics.clone();
    result.params = result
        .params
        .into_iter()
        .filter(|param| match param {
            syn::GenericParam::Type(p) => text.contains(&p.ident.to_string()),
            syn::GenericParam::Const(p) => text.contains(&p.ident.to_string()),
            syn::GenericParam::Lifetime(p) => text.contains(&p.lifetime.to_string()),
        })
        .collect();
    result.where_clause = result.where_clause.filter(|clause| {
        let clause_text = quote! { #clause }.to_string();
        result.params.iter().any(|param| match param {
            syn::GenericParam::Type(p) => clause_text.contains(&p.ident.to_string()),
            syn::GenericParam::Const(p) => clause_text.contains(&p.ident.to_string()),
            syn::GenericParam::Lifetime(p) => clause_text.contains(&p.lifetime.to_string()),
        })
    });
    result
}

fn public_visibility(visibility: Visibility) -> Visibility {
    if matches!(visibility, Visibility::Inherited) {
        syn::parse_quote!(pub)
    } else {
        visibility
    }
}

fn parent_fields(mut fields: Fields) -> Fields {
    if let Fields::Named(fields) = &mut fields {
        for field in &mut fields.named {
            if matches!(field.vis, Visibility::Inherited) {
                field.vis = syn::parse_quote!(pub(super));
            }
        }
    }
    fields
}

fn validate(declaration: &Declaration) -> Result<()> {
    let mut names = BTreeSet::new();
    if let Some(instructions) = &declaration.instructions {
        for instruction in instructions {
            if !names.insert(instruction.name.to_string()) {
                return Err(syn::Error::new_spanned(
                    &instruction.name,
                    "duplicate component instruction variant",
                ));
            }
            for attr in &instruction.attrs {
                if !attr.path().is_ident("pattern") && !attr.path().is_ident("doc") {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "unsupported component instruction attribute; expected `#[pattern = ...]`",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn expand(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as Declaration);
    if let Err(error) = validate(&declaration) {
        return error.into_compile_error().into();
    }

    let Declaration {
        attrs,
        visibility,
        name,
        generics,
        state,
        type_alias,
        value_alias,
        instructions,
    } = declaration;
    let module_name = format_ident!("{}", name.to_string().to_case(Case::Snake));
    let visibility = public_visibility(visibility);
    let state = parent_fields(state);
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let aliases = [type_alias, value_alias]
        .into_iter()
        .flatten()
        .map(|alias| {
            let Alias {
                visibility,
                name,
                target,
            } = alias;
            let visibility = public_visibility(visibility);
            let alias_generics = retain_generics(&generics, [target.clone()]);
            let (ig, _, wc) = alias_generics.split_for_impl();
            quote! { #visibility type #name #ig = #target #wc; }
        });

    let runtime_aliases = aliases.collect::<Vec<_>>();
    let generated_instructions = instructions.map(|instructions| {
        let syntax_variants = instructions.iter().map(|instruction| {
            let Instruction {
                attrs,
                name,
                fields,
            } = instruction;
            let types = fields.iter().map(|field| &field.syntax);
            if fields.is_empty() {
                quote! { #(#attrs)* #name }
            } else {
                quote! { #(#attrs)* #name ( #(#types),* ) }
            }
        });
        let runtime_variants = instructions.iter().map(|instruction| {
            let name = &instruction.name;
            let types = instruction.fields.iter().map(|field| &field.runtime);
            if instruction.fields.is_empty() {
                quote! { #name }
            } else {
                quote! { #name ( #(#types),* ) }
            }
        });
        let all_syntax_types = instructions
            .iter()
            .flat_map(|i| i.fields.iter().map(|f| f.syntax.clone()));
        let all_runtime_types = instructions
            .iter()
            .flat_map(|i| i.fields.iter().map(|f| f.runtime.clone()));
        let syntax_generics = retain_generics(&generics, all_syntax_types);
        let runtime_generics = retain_generics(&generics, all_runtime_types);
        let (sig, _, sw) = syntax_generics.split_for_impl();
        let (rig, _, rw) = runtime_generics.split_for_impl();
        let head = module_name.to_string();
        quote! {
            #visibility mod syntax {
                use super::*;
                #[derive(Clone, Debug, PartialEq, ::vihaco::Parse)]
                #[syntax_class(instruction, head = #head)]
                #visibility enum Instruction #sig #sw {
                    #( #syntax_variants, )*
                }
            }
            #visibility mod runtime {
                use super::*;
                #( #runtime_aliases )*
                #[derive(Clone, Debug, PartialEq)]
                #visibility enum Instruction #rig #rw {
                    #( #runtime_variants, )*
                }
            }
        }
    });

    let aliases_only = if generated_instructions.is_none() && !runtime_aliases.is_empty() {
        quote! {
            #visibility mod runtime {
                use super::*;
                #( #runtime_aliases )*
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #visibility mod #module_name {
            use super::*;
            #(#attrs)*
            #visibility struct #name #impl_generics #where_clause #state
            #generated_instructions
            #aliases_only
        }
    }
    .into()
}

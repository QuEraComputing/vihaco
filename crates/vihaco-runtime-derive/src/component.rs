// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::common::{parse_named_fields, retain_generics};
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Field, Fields, Generics, Ident, Result, Token, Visibility, WhereClause};

syn::custom_keyword!(component);
syn::custom_keyword!(instruction);

struct ComponentDeclaration {
    module: Option<Ident>,
    visibility: Visibility,
    name: Ident,
    generics: Generics,
    state: Fields,
    products: Vec<InstructionProduct>,
}

struct InstructionProduct {
    attrs: Vec<Attribute>,
    visibility: Visibility,
    name: Ident,
    fields: Fields,
}

impl Parse for ComponentDeclaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let module = parse_module_attribute(&attrs)?;
        let visibility = input.parse()?;
        input.parse::<component>()?;
        let name = input.parse()?;
        let mut generics: Generics = input.parse()?;
        generics.where_clause = input.parse::<Option<WhereClause>>()?;
        let content;
        syn::braced!(content in input);
        let state = parse_fields(&content)?;

        let products = if input.peek(instruction) {
            input.parse::<instruction>()?;
            let content;
            syn::braced!(content in input);
            syn::punctuated::Punctuated::<InstructionProduct, Token![,]>::parse_terminated(
                &content,
            )?
            .into_iter()
            .collect()
        } else {
            Vec::new()
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after component declaration"));
        }

        Ok(Self {
            module,
            visibility,
            name,
            generics,
            state,
            products,
        })
    }
}

impl Parse for InstructionProduct {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let visibility = input.parse()?;
        let name = input.parse()?;
        let fields = if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            Fields::Unnamed(syn::FieldsUnnamed {
                paren_token: Default::default(),
                unnamed: syn::punctuated::Punctuated::<Field, Token![,]>::parse_terminated_with(
                    &content,
                    Field::parse_unnamed,
                )?,
            })
        } else if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            Fields::Named(syn::FieldsNamed {
                brace_token: Default::default(),
                named: syn::punctuated::Punctuated::<Field, Token![,]>::parse_terminated_with(
                    &content,
                    Field::parse_named,
                )?,
            })
        } else {
            Fields::Unit
        };

        Ok(Self {
            attrs,
            visibility,
            name,
            fields,
        })
    }
}

fn parse_fields(input: ParseStream<'_>) -> Result<Fields> {
    let fields = parse_named_fields(input)?;
    Ok(Fields::Named(syn::FieldsNamed {
        brace_token: Default::default(),
        named: fields,
    }))
}

fn parse_module_attribute(attrs: &[Attribute]) -> Result<Option<Ident>> {
    let mut module = None;
    for attr in attrs {
        if !attr.path().is_ident("module") {
            return Err(syn::Error::new_spanned(
                attr,
                "unsupported component attribute; expected `#[module = name]`",
            ));
        }
        let value = match &attr.meta {
            syn::Meta::NameValue(value) => match &value.value {
                syn::Expr::Path(path) if path.path.segments.len() == 1 => {
                    path.path.segments[0].ident.clone()
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &value.value,
                        "module name must be an identifier",
                    ));
                }
            },
            _ => return Err(syn::Error::new_spanned(attr, "expected `#[module = name]`")),
        };
        if module.replace(value).is_some() {
            return Err(syn::Error::new_spanned(attr, "duplicate module attribute"));
        }
    }
    Ok(module)
}

fn product_generics(generics: &Generics, fields: &Fields) -> Generics {
    retain_generics(generics, &[quote! { #fields }])
}

fn validate(declaration: &ComponentDeclaration, module: &Ident) -> Result<()> {
    validate_generated_name(&module.to_string(), module.span())?;
    let mut names = BTreeMap::new();
    for product in &declaration.products {
        let normalized = module_name(&product.name).to_case(Case::Snake);
        validate_generated_name(&normalized, product.name.span())?;
        if let Some(previous) = names.insert(normalized, product.name.clone()) {
            return Err(syn::Error::new(
                product.name.span(),
                format!("instruction name collides with `{previous}` after normalization"),
            ));
        }
    }
    Ok(())
}

fn validate_generated_name(name: &str, span: Span) -> Result<()> {
    syn::parse_str::<Ident>(name)
        .map(|_| ())
        .map_err(|_| syn::Error::new(span, "generated name is not a valid Rust identifier"))
}

fn module_name(ident: &Ident) -> String {
    ident.to_string().trim_start_matches("r#").to_owned()
}

fn public_fields(fields: Fields) -> Fields {
    match fields {
        Fields::Named(mut fields) => {
            for field in &mut fields.named {
                if matches!(field.vis, Visibility::Inherited) {
                    field.vis = syn::parse_quote!(pub);
                }
            }
            Fields::Named(fields)
        }
        Fields::Unnamed(mut fields) => {
            for field in &mut fields.unnamed {
                if matches!(field.vis, Visibility::Inherited) {
                    field.vis = syn::parse_quote!(pub);
                }
            }
            Fields::Unnamed(fields)
        }
        Fields::Unit => Fields::Unit,
    }
}

fn parent_visible_fields(mut fields: Fields) -> Fields {
    if let Fields::Named(fields) = &mut fields {
        for field in &mut fields.named {
            if matches!(field.vis, Visibility::Inherited) {
                // Component implementations live in the parent module and need field access.
                field.vis = syn::parse_quote!(pub(super));
            }
        }
    }
    fields
}

fn public_by_default(visibility: Visibility) -> Visibility {
    if matches!(visibility, Visibility::Inherited) {
        syn::parse_quote!(pub)
    } else {
        visibility
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as ComponentDeclaration);
    let module_name = if let Some(module) = declaration.module.clone() {
        module
    } else {
        let name = declaration.name.to_string().to_case(Case::Snake);
        if let Err(error) = validate_generated_name(&name, declaration.name.span()) {
            return error.into_compile_error().into();
        }
        format_ident!("{name}")
    };

    if let Err(error) = validate(&declaration, &module_name) {
        return error.into_compile_error().into();
    }

    let ComponentDeclaration {
        visibility,
        name,
        generics,
        state,
        products,
        ..
    } = declaration;
    let state = parent_visible_fields(state);
    let visibility = public_by_default(visibility);
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let products = products.into_iter().map(|product| {
        let InstructionProduct {
            attrs,
            visibility: product_visibility,
            name,
            fields,
        } = product;
        let fields = public_fields(fields);
        let product_visibility = public_by_default(product_visibility);
        let product_generics = product_generics(&generics, &fields);
        let (product_impl_generics, _, product_where_clause) = product_generics.split_for_impl();
        let declaration = match fields {
            Fields::Unit => quote! {
                #product_visibility struct #name #product_impl_generics #product_where_clause;
            },
            Fields::Named(fields) => quote! {
                #product_visibility struct #name #product_impl_generics #product_where_clause #fields
            },
            Fields::Unnamed(fields) => quote! {
                #product_visibility struct #name #product_impl_generics #fields #product_where_clause;
            },
        };
        quote! {
            #(#attrs)*
            #declaration
        }
    });

    quote! {
        #visibility mod #module_name {
            use super::*;

            #visibility struct #name #impl_generics #where_clause #state

            #visibility mod instruction {
                use super::*;

                #( #products )*
            }
        }
    }
    .into()
}

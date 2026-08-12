// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::common::{parse_named_fields, retain_generics};
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Field, Fields, Generics, Ident, Result, Token, Type, Visibility, WhereClause,
};

syn::custom_keyword!(component);
syn::custom_keyword!(instruction);
syn::custom_keyword!(runtime);
syn::custom_keyword!(syntax);
syn::custom_keyword!(value);

struct ComponentDeclaration {
    module: Option<Ident>,
    visibility: Visibility,
    name: Ident,
    generics: Generics,
    state: Fields,
    runtime: Option<RuntimeDeclaration>,
    syntax: Option<ComponentSyntax>,
}

struct RuntimeDeclaration {
    type_alias: Option<RuntimeAlias>,
    value_alias: Option<RuntimeAlias>,
    products: Vec<InstructionProduct>,
}

struct RuntimeAlias {
    visibility: Visibility,
    name: Ident,
    target: Type,
}

struct ComponentSyntax {
    type_declaration: Option<SyntaxEnum>,
    value_declaration: Option<SyntaxEnum>,
    instruction_declaration: Option<SyntaxInstruction>,
}

struct SyntaxEnum {
    name: Ident,
    variants: Vec<SyntaxVariant>,
}

struct SyntaxVariant {
    name: Ident,
    fields: Fields,
    pattern: Option<syn::LitStr>,
}

struct SyntaxInstruction {
    variants: Vec<SyntaxInstructionVariant>,
}

struct SyntaxInstructionVariant {
    name: Ident,
    fields: Vec<syn::Type>,
    pattern: syn::LitStr,
}

#[derive(Clone)]
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

        let runtime_declaration = if input.peek(runtime) {
            input.parse::<runtime>()?;
            let content;
            syn::braced!(content in input);
            Some(content.parse()?)
        } else {
            None
        };

        let syntax_declaration = if input.peek(syntax) {
            input.parse::<syntax>()?;
            let content;
            syn::braced!(content in input);
            Some(content.parse()?)
        } else {
            None
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
            runtime: runtime_declaration,
            syntax: syntax_declaration,
        })
    }
}

impl Parse for RuntimeDeclaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut type_alias = None;
        let mut value_alias = None;
        let mut products = None;

        while !input.is_empty() {
            let _attrs = Attribute::parse_outer(input)?;
            let visibility: Visibility = input.parse()?;
            if input.peek(Token![type]) {
                input.parse::<Token![type]>()?;
                let name = input.parse()?;
                input.parse::<Token![=]>()?;
                let target = input.parse()?;
                input.parse::<Token![;]>()?;
                if type_alias
                    .replace(RuntimeAlias {
                        visibility,
                        name,
                        target,
                    })
                    .is_some()
                {
                    return Err(input.error("duplicate component runtime type alias"));
                }
            } else if input.peek(value) {
                input.parse::<value>()?;
                let name = input.parse()?;
                input.parse::<Token![=]>()?;
                let target = input.parse()?;
                input.parse::<Token![;]>()?;
                if value_alias
                    .replace(RuntimeAlias {
                        visibility,
                        name,
                        target,
                    })
                    .is_some()
                {
                    return Err(input.error("duplicate component runtime value alias"));
                }
            } else if input.peek(instruction) {
                input.parse::<instruction>()?;
                let content;
                syn::braced!(content in input);
                if products.is_some() {
                    return Err(input.error("duplicate component runtime instruction declaration"));
                }
                products = Some(
                    syn::punctuated::Punctuated::<InstructionProduct, Token![,]>::parse_terminated(
                        &content,
                    )?
                    .into_iter()
                    .collect(),
                );
            } else {
                return Err(
                    input.error("expected `type`, `value`, or `instruction` runtime declaration")
                );
            }
        }

        Ok(Self {
            type_alias,
            value_alias,
            products: products.unwrap_or_default(),
        })
    }
}

impl Parse for ComponentSyntax {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut type_declaration = None;
        let mut value_declaration = None;
        let mut instruction_declaration = None;

        while !input.is_empty() {
            if input.peek(Token![type]) {
                input.parse::<Token![type]>()?;
                let declaration = parse_syntax_enum(input)?;
                if type_declaration.replace(declaration).is_some() {
                    return Err(input.error("duplicate component syntax type declaration"));
                }
            } else if input.peek(value) {
                input.parse::<value>()?;
                let declaration = parse_syntax_enum(input)?;
                if value_declaration.replace(declaration).is_some() {
                    return Err(input.error("duplicate component syntax value declaration"));
                }
            } else if input.peek(instruction) {
                input.parse::<instruction>()?;
                let content;
                syn::braced!(content in input);
                let declaration = SyntaxInstruction {
                    variants: parse_syntax_instruction_variants(&content)?,
                };
                if instruction_declaration.replace(declaration).is_some() {
                    return Err(input.error("duplicate component syntax instruction declaration"));
                }
            } else {
                return Err(input.error("expected `type`, `value`, or `instruction`"));
            }
        }

        Ok(Self {
            type_declaration,
            value_declaration,
            instruction_declaration,
        })
    }
}

fn parse_syntax_enum(input: ParseStream<'_>) -> Result<SyntaxEnum> {
    let name = input.parse()?;
    let content;
    syn::braced!(content in input);
    let mut variants = Vec::new();
    while !content.is_empty() {
        let variant = SyntaxVariant {
            name: content.parse()?,
            fields: parse_variant_fields(&content)?,
            pattern: {
                if content.peek(Token![=]) {
                    content.parse::<Token![=]>()?;
                    Some(content.parse()?)
                } else {
                    None
                }
            },
        };
        variants.push(variant);
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if content.peek(Token![;]) {
            content.parse::<Token![;]>()?;
        }
    }
    Ok(SyntaxEnum { name, variants })
}

fn parse_variant_fields(input: ParseStream<'_>) -> Result<Fields> {
    if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        Ok(Fields::Unnamed(syn::FieldsUnnamed {
            paren_token: Default::default(),
            unnamed: syn::punctuated::Punctuated::<Field, Token![,]>::parse_terminated_with(
                &content,
                Field::parse_unnamed,
            )?,
        }))
    } else if input.peek(syn::token::Brace) {
        let content;
        syn::braced!(content in input);
        Ok(Fields::Named(syn::FieldsNamed {
            brace_token: Default::default(),
            named: syn::punctuated::Punctuated::<Field, Token![,]>::parse_terminated_with(
                &content,
                Field::parse_named,
            )?,
        }))
    } else {
        Ok(Fields::Unit)
    }
}

fn parse_syntax_instruction_variants(
    input: ParseStream<'_>,
) -> Result<Vec<SyntaxInstructionVariant>> {
    let mut variants = Vec::new();
    while !input.is_empty() {
        let name = input.parse()?;
        let fields = if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            syn::punctuated::Punctuated::<syn::Type, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        input.parse::<Token![=]>()?;
        let pattern = input.parse()?;
        variants.push(SyntaxInstructionVariant {
            name,
            fields,
            pattern,
        });
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
    }
    Ok(variants)
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
    if let Some(syntax) = &declaration.syntax
        && (syntax.type_declaration.is_none()
            || syntax.value_declaration.is_none()
            || syntax.instruction_declaration.is_none())
    {
        return Err(syn::Error::new(
            module.span(),
            "component syntax requires `type`, `value`, and `instruction` declarations",
        ));
    }
    if let Some(runtime) = &declaration.runtime {
        validate_products(&runtime.products)?;
    }
    Ok(())
}

fn validate_products(products: &[InstructionProduct]) -> Result<()> {
    let mut names = BTreeMap::new();
    for product in products {
        let normalized = module_name(&product.name).to_case(Case::Snake);
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
        runtime,
        syntax,
        ..
    } = declaration;
    let state = parent_visible_fields(state);
    let visibility = public_by_default(visibility);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let runtime = runtime.map(|runtime| {
        let type_alias = runtime.type_alias.map(|alias| {
            let visibility = public_by_default(alias.visibility);
            let RuntimeAlias { name, target, .. } = alias;
            quote! {
                #visibility type #name = #target;
            }
        });
        let value_alias = runtime.value_alias.map(|alias| {
            let visibility = public_by_default(alias.visibility);
            let RuntimeAlias { name, target, .. } = alias;
            quote! {
                #visibility type #name = #target;
            }
        });
        let products = runtime.products.into_iter().map(|product| {
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
            #visibility mod runtime {
                use super::*;

                #type_alias
                #value_alias

                #visibility mod instruction {
                    use super::*;

                    #( #products )*
                }
            }
        }
    });
    let syntax = syntax.map(|syntax| {
        let type_declaration = syntax.type_declaration.expect("validated component syntax");
        let value_declaration = syntax
            .value_declaration
            .expect("validated component syntax");
        let instruction_declaration = syntax
            .instruction_declaration
            .expect("validated component syntax");
        let type_name = type_declaration.name;
        let value_name = value_declaration.name;
        let type_variants = type_declaration.variants.into_iter().map(|variant| {
            let name = variant.name;
            let fields = variant.fields;
            let pattern = variant.pattern;
            let fields = match fields {
                Fields::Unit => quote! {},
                Fields::Named(fields) => quote! { #fields },
                Fields::Unnamed(fields) => quote! { #fields },
            };
            let pattern = pattern.map(|pattern| quote! { #[pattern = #pattern] });
            quote! {
                #pattern
                #name #fields
            }
        });
        let value_variants = value_declaration.variants.into_iter().map(|variant| {
            let name = variant.name;
            let fields = variant.fields;
            let pattern = variant.pattern;
            let fields = match fields {
                Fields::Unit => quote! {},
                Fields::Named(fields) => quote! { #fields },
                Fields::Unnamed(fields) => quote! { #fields },
            };
            let pattern = pattern.map(|pattern| quote! { #[pattern = #pattern] });
            quote! {
                #pattern
                #name #fields
            }
        });
        let instruction_variants = instruction_declaration.variants.into_iter().map(|variant| {
            let name = variant.name;
            let pattern = variant.pattern;
            let fields = variant.fields;
            if fields.is_empty() {
                quote! {
                    #[pattern = #pattern]
                    #name
                }
            } else {
                quote! {
                    #[pattern = #pattern]
                    #name(#(#fields),*)
                }
            }
        });
        quote! {
            #visibility mod syntax {
                use super::*;

                #[derive(Clone, Debug, PartialEq, ::vihaco::Parse)]
                #[syntax_class(type)]
                #visibility enum #type_name {
                    #( #type_variants, )*
                }

                #[derive(Clone, Debug, PartialEq, ::vihaco::Parse)]
                #[syntax_class(value)]
                #visibility enum #value_name {
                    #( #value_variants, )*
                }

                #[derive(Clone, Debug, PartialEq, ::vihaco::Parse)]
                #[syntax_class(instruction)]
                #visibility enum Instruction {
                    #( #instruction_variants, )*
                }
            }

            impl #impl_generics ::vihaco::InstructionSet for #name #ty_generics #where_clause {
                type Instruction = syntax::Instruction;
                type Value = syntax::#value_name;
                type Type = syntax::#type_name;
            }
        }
    });

    quote! {
        #visibility mod #module_name {
            use super::*;

            #visibility struct #name #impl_generics #where_clause #state

            #runtime
            #syntax
        }
    }
    .into()
}

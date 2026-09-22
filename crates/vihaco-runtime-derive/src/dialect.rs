// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Expr, ExprLit, Ident, Lit, Result, Token, Type};

use crate::common::resolve_root;

struct Declaration {
    attrs: Vec<Attribute>,
    name: Ident,
    instructions: Vec<Instruction>,
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
        let name = input.parse()?;
        let content;
        syn::braced!(content in input);
        let instructions = parse_instructions(&content)?;

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after dialect declaration"));
        }

        Ok(Self {
            attrs,
            name,
            instructions,
        })
    }
}

fn parse_instructions(input: ParseStream<'_>) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    while !input.is_empty() {
        let attrs = Attribute::parse_outer(input)?;
        let name = input.parse()?;
        let mut fields = Vec::new();

        // TODO: we should eventually support non-tuple instructions
        if input.peek(syn::token::Brace) {
            return Err(input.error("named-field dialect instructions are not supported"));
        }

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

        instructions.push(Instruction {
            attrs,
            name,
            fields,
        });

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if !input.is_empty() {
            return Err(input.error("expected `,` between dialect instructions"));
        }
    }

    Ok(instructions)
}

fn validate(declaration: &Declaration) -> Result<()> {
    if declaration.instructions.is_empty() {
        return Err(syn::Error::new_spanned(
            &declaration.name,
            "dialect instruction list must not be empty",
        ));
    }

    for attr in &declaration.attrs {
        if !attr.path().is_ident("doc") && !attr.path().is_ident("vihaco") {
            return Err(syn::Error::new_spanned(
                attr,
                "unsupported dialect attribute; expected documentation or `#[vihaco(crate = ...)]`",
            ));
        }
    }

    let mut names = BTreeSet::new();
    let mut mnemonics = BTreeSet::new();
    for instruction in &declaration.instructions {
        if !names.insert(instruction.name.to_string()) {
            return Err(syn::Error::new_spanned(
                &instruction.name,
                "duplicate dialect instruction",
            ));
        }

        for attr in &instruction.attrs {
            if !attr.path().is_ident("pattern") && !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "unsupported dialect instruction attribute; expected `#[pattern = ...]`",
                ));
            }
        }

        let Some(mnemonic) = instruction_mnemonic(instruction) else {
            continue;
        };
        if !mnemonics.insert(mnemonic.clone()) {
            let message = format!("duplicate instruction name `{mnemonic}`");
            let error = instruction
                .attrs
                .iter()
                .rev()
                .find(|attr| attr.path().is_ident("pattern"))
                .map_or_else(
                    || syn::Error::new_spanned(&instruction.name, &message),
                    |attr| syn::Error::new_spanned(attr, &message),
                );
            return Err(error);
        }
    }

    Ok(())
}

fn instruction_mnemonic(instruction: &Instruction) -> Option<String> {
    let pattern = instruction
        .attrs
        .iter()
        .rev()
        .find(|attr| attr.path().is_ident("pattern"));

    let Some(pattern) = pattern else {
        return Some(instruction.name.to_string().to_lowercase());
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(pattern),
        ..
    }) = &pattern.meta.require_name_value().ok()?.value
    else {
        return None;
    };

    let pattern = pattern.value();
    if pattern.split(' ').any(str::is_empty) {
        return None;
    }

    pattern
        .split(' ')
        .next()
        .and_then(|atom| atom.strip_prefix('\''))
        .filter(|mnemonic| !mnemonic.is_empty())
        .map(str::to_owned)
}

pub fn expand(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as Declaration);
    let root = match resolve_root(&declaration.attrs) {
        Ok(root) => root,
        Err(error) => return error.into_compile_error().into(),
    };
    if let Err(error) = validate(&declaration) {
        return error.into_compile_error().into();
    }

    let Declaration {
        attrs,
        name,
        instructions,
    } = declaration;
    let module_attrs = attrs.iter().filter(|attr| !attr.path().is_ident("vihaco"));
    let syntax_head = name.to_string();

    let runtime_structs = instructions.iter().map(|instruction| {
        let docs = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));
        let name = &instruction.name;
        let fields = instruction.fields.iter().map(|field| &field.runtime);

        if instruction.fields.is_empty() {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq)]
                pub struct #name;
            }
        } else {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq)]
                pub struct #name( #(pub #fields),* );
            }
        }
    });

    let syntax_structs = instructions.iter().map(|instruction| {
        let docs = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));
        let pattern = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("pattern"));
        let name = &instruction.name;
        let fields = instruction.fields.iter().map(|field| &field.syntax);

        if instruction.fields.is_empty() {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq, #root::Parse)]
                #[syntax_class(instruction, head = #syntax_head)]
                #(#pattern)*
                pub struct #name;
            }
        } else {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq, #root::Parse)]
                #[syntax_class(instruction, head = #syntax_head)]
                #(#pattern)*
                pub struct #name( #(pub #fields),* );
            }
        }
    });

    quote! {
        #(#module_attrs)*
        pub mod #name {
            use super::*;

            #( #runtime_structs )*

            pub mod syntax {
                use super::*;

                #( #syntax_structs )*
            }
        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn parses_syntax_and_runtime_payload_mappings() {
        let declaration: Declaration = syn::parse_quote! {
            arith {
                Add(u32 => i64, bool),
                Halt,
            }
        };

        assert_eq!(declaration.name, "arith");
        assert_eq!(declaration.instructions.len(), 2);
        let fields = &declaration.instructions[0].fields;
        assert_eq!(fields[0].syntax.to_token_stream().to_string(), "u32");
        assert_eq!(fields[0].runtime.to_token_stream().to_string(), "i64");
        assert_eq!(fields[1].syntax.to_token_stream().to_string(), "bool");
        assert_eq!(fields[1].runtime.to_token_stream().to_string(), "bool");
    }

    #[test]
    fn rejects_duplicate_instruction_names() {
        let declaration: Declaration = syn::parse_quote! {
            arith { Add, Add }
        };

        let error = validate(&declaration).expect_err("duplicate instructions must be rejected");
        assert_eq!(error.to_string(), "duplicate dialect instruction");
    }

    #[test]
    fn rejects_duplicate_instruction_mnemonics() {
        let declaration: Declaration = syn::parse_quote! {
            arith {
                Add,
                #[pattern = "'add $0"]
                AddImmediate(u32),
            }
        };

        let error = validate(&declaration).expect_err("duplicate mnemonics must be rejected");
        assert_eq!(error.to_string(), "duplicate instruction name `add`");
    }

    #[test]
    fn leaves_malformed_patterns_to_parser_derive() {
        let declaration: Declaration = syn::parse_quote! {
            arith {
                Add,
                #[pattern = "'add  $0"]
                AddImmediate(u32),
            }
        };

        assert!(validate(&declaration).is_ok());
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, Result, Token, Type};

pub(super) struct Declaration {
    pub(super) attrs: Vec<Attribute>,
    pub(super) name: Ident,
    pub(super) instructions: Vec<Instruction>,
}

pub(super) struct Instruction {
    pub(super) attrs: Vec<Attribute>,
    pub(super) name: Ident,
    pub(super) fields: Vec<FieldMapping>,
}

pub(super) struct FieldMapping {
    pub(super) syntax: Type,
    pub(super) runtime: Type,
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

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Attribute, Field, Generics, Ident, LitInt, LitStr, Result, Token, Type, Visibility, WhereClause,
};

use crate::common::parse_named_fields;

syn::custom_keyword!(composite);
syn::custom_keyword!(error);
syn::custom_keyword!(syntax);
syn::custom_keyword!(runtime);
syn::custom_keyword!(message);
syn::custom_keyword!(effects);
syn::custom_keyword!(observe);
syn::custom_keyword!(absorb);
syn::custom_keyword!(handle);
syn::custom_keyword!(none);
syn::custom_keyword!(from);
syn::custom_keyword!(with);
syn::custom_keyword!(header);

pub(super) struct CompositeDeclaration {
    pub(super) attrs: Vec<Attribute>,
    pub(super) visibility: Visibility,
    pub(super) name: Ident,
    pub(super) generics: Generics,
    pub(super) error: Option<Type>,
    pub(super) fields: Vec<Field>,
    pub(super) header: Option<HeaderDeclaration>,
    pub(super) syntax: Vec<SyntaxDeclaration>,
    pub(super) routes: Vec<RouteDeclaration>,
}

pub(super) struct HeaderDeclaration {
    pub(super) ty: Type,
    pub(super) resolver: Ident,
}

pub(super) struct SyntaxDeclaration {
    pub(super) pattern: LitStr,
    pub(super) variant: Ident,
    pub(super) payload: Option<Type>,
    pub(super) mapping: SyntaxMapping,
}

pub(super) enum SyntaxMapping {
    Lower(Ident),
    Runtime(Ident),
}

pub(super) struct RouteDeclaration {
    pub(super) variant: Ident,
    pub(super) payload: Type,
    pub(super) target: Ident,
    pub(super) message: MessageSource,
    pub(super) observers: Vec<Ident>,
    pub(super) handler: Option<Handler>,
}

pub(super) enum MessageSource {
    None,
    From(Ident),
    With(Ident),
}

pub(super) enum Handler {
    Absorb(Ident),
    With(Ident),
}

pub(super) struct DeviceArgs {
    pub(super) code: u8,
    pub(super) aliases: Vec<LitStr>,
}

pub(super) struct SyntaxArgs {
    pub(super) aliases: Vec<LitStr>,
}

impl Parse for SyntaxArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                aliases: Vec::new(),
            });
        }
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            return Ok(Self {
                aliases: vec![input.parse()?],
            });
        }
        Ok(Self {
            aliases: syn::punctuated::Punctuated::<LitStr, Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect(),
        })
    }
}

impl Parse for CompositeDeclaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let visibility = input.parse()?;
        input.parse::<composite>()?;
        let name: Ident = input.parse()?;

        let mut generics: Generics = input.parse()?;
        generics.where_clause = input.parse::<Option<WhereClause>>()?;

        let body;
        syn::braced!(body in input);
        let (error, fields) = parse_composite_body(&body)?;

        let (header, syntax) = if input.peek(syntax) {
            input.parse::<syntax>()?;
            let syntax_body;
            syn::braced!(syntax_body in input);
            parse_syntax(&syntax_body)?
        } else {
            (None, Vec::new())
        };

        let routes = if input.peek(runtime) {
            input.parse::<runtime>()?;
            let routes_body;
            syn::braced!(routes_body in input);
            parse_routes(&routes_body)?
        } else {
            Vec::new()
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after composite declaration"));
        }

        if !routes.is_empty() && error.is_none() {
            return Err(syn::Error::new(
                name.span(),
                "executable composites require `error = <type>;`",
            ));
        }

        Ok(Self {
            attrs,
            visibility,
            name,
            generics,
            error,
            fields,
            header,
            syntax,
            routes,
        })
    }
}

fn parse_syntax(
    input: ParseStream<'_>,
) -> Result<(Option<HeaderDeclaration>, Vec<SyntaxDeclaration>)> {
    let header = if input.peek(header) {
        input.parse::<header>()?;
        let ty = input.parse::<Type>()?;
        input.parse::<Token![=>]>()?;
        let resolver = input.parse::<Ident>()?;
        input.parse::<Token![;]>()?;
        Some(HeaderDeclaration { ty, resolver })
    } else {
        None
    };
    let mut declarations = Vec::new();
    while !input.is_empty() {
        let attrs = Attribute::parse_outer(input)?;
        let [pattern] = attrs.as_slice() else {
            return Err(input.error("syntax entries require one `#[pattern = \"...\"]` attribute"));
        };
        if !pattern.path().is_ident("pattern") {
            return Err(syn::Error::new(
                pattern.span(),
                "syntax entries require `#[pattern = \"...\"]`",
            ));
        }
        let value = &pattern.meta.require_name_value()?.value;
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(pattern),
            ..
        }) = value
        else {
            return Err(syn::Error::new(
                input.span(),
                "`pattern` must be a string literal",
            ));
        };

        let variant = input.parse::<Ident>()?;
        let payload = if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            if content.is_empty() {
                None
            } else {
                Some(content.parse()?)
            }
        } else {
            None
        };
        input.parse::<Token![=>]>()?;
        let mapping = if input.peek(runtime) {
            input.parse::<runtime>()?;
            SyntaxMapping::Runtime(input.parse()?)
        } else {
            SyntaxMapping::Lower(input.parse()?)
        };
        input.parse::<Token![;]>()?;
        declarations.push(SyntaxDeclaration {
            pattern: pattern.clone(),
            variant,
            payload,
            mapping,
        });
    }
    Ok((header, declarations))
}

fn parse_composite_body(input: ParseStream<'_>) -> Result<(Option<Type>, Vec<Field>)> {
    let mut error_type = None;

    if input.peek(error) {
        input.parse::<error>()?;
        input.parse::<Token![=]>()?;
        error_type = Some(input.parse::<Type>()?);
        input.parse::<Token![;]>()?;
    }

    let fields = parse_named_fields(input)?.into_iter().collect();

    Ok((error_type, fields))
}

fn parse_routes(input: ParseStream<'_>) -> Result<Vec<RouteDeclaration>> {
    let mut routes = Vec::new();
    while !input.is_empty() {
        routes.push(input.parse::<RouteDeclaration>()?);
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(routes)
}

impl Parse for RouteDeclaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let variant: Ident = input.parse()?;
        let payload_content;
        syn::parenthesized!(payload_content in input);
        let payload = payload_content.parse()?;
        input.parse::<Token![=>]>()?;
        let target = input.parse()?;

        let body;
        syn::braced!(body in input);

        let mut message_source = None;
        let mut observers = Vec::new();
        let mut handler = None;

        while !body.is_empty() {
            if body.peek(message) {
                body.parse::<message>()?;
                if message_source.is_some() {
                    return Err(body.error("route has more than one message clause"));
                }
                let source = if body.peek(none) {
                    body.parse::<none>()?;
                    MessageSource::None
                } else if body.peek(from) {
                    body.parse::<from>()?;
                    MessageSource::From(body.parse()?)
                } else if body.peek(with) {
                    body.parse::<with>()?;
                    MessageSource::With(body.parse()?)
                } else {
                    return Err(body.error("expected `none`, `from <field>`, or `with <method>`"));
                };
                body.parse::<Token![;]>()?;
                message_source = Some(source);
            } else if body.peek(effects) {
                body.parse::<effects>()?;
                let effects_body;
                syn::braced!(effects_body in body);
                if handler.is_some() || !observers.is_empty() {
                    return Err(body.error("route has more than one effects block"));
                }
                parse_effects(&effects_body, &mut observers, &mut handler)?;
            } else {
                return Err(body.error("expected a message clause or effects block"));
            }
        }

        let message = message_source
            .ok_or_else(|| syn::Error::new(variant.span(), "route is missing a message clause"))?;

        Ok(Self {
            variant,
            payload,
            target,
            message,
            observers,
            handler,
        })
    }
}

fn parse_effects(
    input: ParseStream<'_>,
    observers: &mut Vec<Ident>,
    handler: &mut Option<Handler>,
) -> Result<()> {
    while !input.is_empty() {
        if input.peek(observe) {
            input.parse::<observe>()?;
            let mut names = Vec::new();
            loop {
                names.push(input.parse::<Ident>()?);
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                } else {
                    break;
                }
            }
            if names.is_empty() {
                return Err(input.error("`observe` requires at least one field"));
            }
            observers.extend(names);
            input.parse::<Token![;]>()?;
        } else if input.peek(absorb) || input.peek(handle) {
            let is_absorb = input.peek(absorb);
            if is_absorb {
                input.parse::<absorb>()?;
            } else {
                input.parse::<handle>()?;
            }
            input.parse::<with>()?;
            let method_or_field: Ident = input.parse()?;
            if handler.is_some() {
                return Err(syn::Error::new(
                    method_or_field.span(),
                    "route has more than one effect handler",
                ));
            }
            *handler = Some(if is_absorb {
                Handler::Absorb(method_or_field)
            } else {
                Handler::With(method_or_field)
            });
            input.parse::<Token![;]>()?;
        } else {
            return Err(input.error(
                "expected `observe <field>`, `absorb with <field>`, or `handle with <method>`",
            ));
        }
    }

    if handler.is_none() {
        return Err(input.error("effects block is missing an effect handler"));
    }
    Ok(())
}

impl Parse for DeviceArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let literal: LitInt = input.parse()?;
        let code = literal.base10_parse::<u8>()?;
        let mut aliases = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if key != "alias" {
                return Err(syn::Error::new(key.span(), "unsupported device argument"));
            }
            aliases.push(input.parse()?);
        }
        Ok(Self { code, aliases })
    }
}

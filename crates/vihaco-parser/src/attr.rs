// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::Span;
use syn::{
    Attribute, Error, Fields, LitStr, Meta, Result, Variant, meta::ParseNestedMeta,
    spanned::Spanned,
};

// --- Enum-level ---

pub enum HeadAttr {
    Auto,           // #[head]
    Custom(String), // #[head = "X::"]
}

#[derive(Clone, Copy)]
pub enum SyntaxClassAttr {
    Instruction,
    Type,
    Value,
}

pub struct EnumAttrs {
    pub head: Option<HeadAttr>,
    pub syntax_class: Option<SyntaxClassAttr>,
}

pub struct StructAttrs {
    pub pattern: Option<PatternInfo>,
    pub syntax_class: Option<SyntaxClassAttr>,
}

// --- Variant-level ---

pub struct DelimiterAttrs {
    pub open: String,
    pub close: String,
    pub separator: String,
}

impl Default for DelimiterAttrs {
    fn default() -> Self {
        Self {
            open: "(".into(),
            close: ")".into(),
            separator: ",".into(),
        }
    }
}

pub struct VariantAttrs {
    pub pattern: Option<PatternInfo>,
    pub token: Option<String>,
    pub delimiters: DelimiterAttrs,
    pub delegate: bool,
    pub delegate_span: Option<Span>,
}

// --- Field-level ---

pub struct FieldAttrs {
    pub parse_with: Option<String>,
}

impl EnumAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut head = None;
        let mut syntax_class = None;
        for attr in attrs {
            let span = attr.span();
            if attr.path().is_ident("head") {
                match &attr.meta {
                    Meta::Path(_) => {
                        head = Some(HeadAttr::Auto);
                    }
                    Meta::NameValue(nv) => {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = &nv.value
                        {
                            head = Some(HeadAttr::Custom(s.value()));
                        } else {
                            return Err(Error::new(span, "#[head] value must be a string literal"));
                        }
                    }
                    _ => return Err(Error::new(span, "invalid #[head] syntax")),
                }
            }

            if attr.path().is_ident("syntax_class") {
                attr.parse_nested_meta(|meta| parse_syntax_class(meta, &mut syntax_class, span))?;
            }
        }

        Ok(Self { head, syntax_class })
    }
}

fn parse_syntax_class(
    meta: ParseNestedMeta,
    syntax_class: &mut Option<SyntaxClassAttr>,
    span: Span,
) -> Result<()> {
    if meta.path.is_ident("instruction") {
        *syntax_class = Some(SyntaxClassAttr::Instruction);
        return Ok(());
    }

    if meta.path.is_ident("type") {
        *syntax_class = Some(SyntaxClassAttr::Type);
        return Ok(());
    }

    if meta.path.is_ident("value") {
        *syntax_class = Some(SyntaxClassAttr::Value);
        return Ok(());
    }

    Err(Error::new(
        span,
        "invalid syntax class: expected #[syntax_class(class)], where class is instruction, type, or value",
    ))
}

impl StructAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut pattern = None;
        let mut syntax_class = None;

        for attr in attrs {
            let span = attr.span();

            if attr.path().is_ident("pattern") {
                pattern = Some(pattern_attr(attr)?);
            }

            if attr.path().is_ident("syntax_class") {
                attr.parse_nested_meta(|meta| parse_syntax_class(meta, &mut syntax_class, span))?;
            }
        }

        Ok(Self {
            pattern,
            syntax_class,
        })
    }
}

fn string_attr(attr: &Attribute, attr_name: &str, attr_val: &str) -> Result<String> {
    let nv = attr.meta.require_name_value()?;

    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(s),
        ..
    }) = &nv.value
    {
        Ok(s.value())
    } else {
        Err(Error::new_spanned(
            &nv.value,
            format!("#[{attr_name}] requires a string value: #[{attr_name}] = {attr_val}"),
        ))
    }
}

pub struct PatternInfo(pub String, pub Span);

fn pattern_attr(attr: &Attribute) -> Result<PatternInfo> {
    let name_value = attr.meta.require_name_value()?;
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(pattern_literal),
        ..
    }) = &name_value.value
    else {
        return Err(Error::new_spanned(
            &name_value.value,
            "#[pattern] requires a string value: #[pattern] = pattern",
        ));
    };

    Ok(PatternInfo(pattern_literal.value(), pattern_literal.span()))
}

impl VariantAttrs {
    pub fn from_variant(variant: &Variant) -> Result<Self> {
        let mut token = None;
        let mut pattern_info = None;
        let mut delimiters = DelimiterAttrs::default();
        let mut delegate = false;
        let mut delegate_span = None;

        for attr in &variant.attrs {
            let span = attr.span();

            if attr.path().is_ident("pattern") {
                pattern_info = Some(pattern_attr(attr)?);
                continue;
            }

            if attr.path().is_ident("token") {
                token = Some(string_attr(attr, "token", "name")?);
                continue;
            }

            if attr.path().is_ident("delimiters") {
                attr.parse_nested_meta(|meta| {
                    let ident = meta
                        .path
                        .get_ident()
                        .ok_or_else(|| meta.error("expected ident in #[delimiters(...)]"))?
                        .to_string();
                    let value: LitStr = meta.value()?.parse()?;
                    match ident.as_str() {
                        "open" => delimiters.open = value.value(),
                        "close" => delimiters.close = value.value(),
                        "separator" => delimiters.separator = value.value(),
                        other => {
                            return Err(
                                meta.error(format!("unknown key `{other}` in #[delimiters]"))
                            );
                        }
                    }
                    Ok(())
                })?;
                continue;
            }

            if attr.path().is_ident("delegate") {
                delegate = true;
                delegate_span = Some(span);
                continue;
            }
        }

        // Validate: struct-style variants are not supported
        if let Fields::Named(_) = variant.fields {
            return Err(Error::new_spanned(
                &variant.ident,
                "#[derive(Parse)] does not support struct-style variants (e.g., `Foo { x: T }`) — use tuple variants (`Foo(T)`) or unit variants (`Foo`)",
            ));
        }

        // Validate: #[delimiters] on unit variant
        if let Fields::Unit = variant.fields {
            for attr in &variant.attrs {
                if attr.path().is_ident("delimiters") {
                    return Err(Error::new(
                        attr.span(),
                        "#[delimiters] cannot be used on a unit variant (no fields)",
                    ));
                }
            }
        }

        // Validate: #[delegate] conflicts
        if delegate {
            let span = delegate_span.unwrap();
            if token.is_some() {
                return Err(Error::new(
                    span,
                    "#[delegate] cannot be combined with #[token]",
                ));
            }
            for attr in &variant.attrs {
                if attr.path().is_ident("delimiters") {
                    return Err(Error::new(
                        span,
                        "#[delegate] cannot be combined with #[delimiters]",
                    ));
                }
            }
            match &variant.fields {
                Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                    // valid: single-field tuple variant
                }
                _ => {
                    return Err(Error::new(
                        span,
                        "#[delegate] is only valid on single-field tuple variants",
                    ));
                }
            }
        }

        Ok(Self {
            pattern: pattern_info,
            token,
            delimiters,
            delegate,
            delegate_span,
        })
    }
}

impl FieldAttrs {
    pub fn from_field(field: &syn::Field) -> Result<Self> {
        let mut parse_with = None;
        for attr in &field.attrs {
            if attr.path().is_ident("parse_with") {
                if let Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        parse_with = Some(s.value());
                        continue;
                    }
                }
                return Err(Error::new(
                    attr.span(),
                    "#[parse_with] requires a string path: #[parse_with = \"fn_name\"]",
                ));
            }
        }
        Ok(Self { parse_with })
    }
}

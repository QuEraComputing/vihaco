// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Attribute, Error, Fields, LitStr, Result, Token, Variant,
};

mod kw {
    syn::custom_keyword!(head);
    syn::custom_keyword!(instruction);
    syn::custom_keyword!(metadata);
    syn::custom_keyword!(value);
}

// --- Enum-level ---

#[derive(Clone)]
pub enum SyntaxClassAttr {
    Instruction { head: Option<String> },
    Metadata { head: String },
    Type,
    Value,
}

impl Parse for SyntaxClassAttr {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(kw::instruction) {
            input.parse::<kw::instruction>()?;

            if input.is_empty() {
                return Ok(Self::Instruction { head: None });
            }

            input.parse::<Token![,]>()?;
            input.parse::<kw::head>()?;
            input.parse::<Token![=]>()?;
            let head = input.parse::<LitStr>()?.value();

            return Ok(Self::Instruction { head: Some(head) });
        }

        if input.peek(kw::metadata) {
            input.parse::<kw::metadata>()?;

            if input.is_empty() {
                return Err(input.error("metadata syntax class must have `head` argument"));
            }

            input.parse::<Token![,]>()?;
            input.parse::<kw::head>()?;
            input.parse::<Token![=]>()?;
            let head = input.parse::<LitStr>()?.value();

            return Ok(Self::Metadata { head });
        }

        if input.peek(kw::value) {
            input.parse::<kw::value>()?;
            return Ok(Self::Value);
        }

        if input.peek(Token![type]) {
            input.parse::<Token![type]>()?;
            return Ok(Self::Type);
        }

        Err(input.error("expected `instruction`, `metadata`, `value`, or `type`"))
    }
}

pub struct EnumAttrs {
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
}

impl EnumAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut syntax_class = None;
        for attr in attrs {
            if attr.path().is_ident("syntax_class") {
                syntax_class = Some(attr.parse_args()?);
            }
        }

        Ok(Self { syntax_class })
    }
}

impl StructAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut pattern = None;
        let mut syntax_class = None;

        for attr in attrs {
            if attr.path().is_ident("pattern") {
                pattern = Some(pattern_attr(attr)?);
            }

            if attr.path().is_ident("syntax_class") {
                syntax_class = Some(attr.parse_args()?);
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
        })
    }
}

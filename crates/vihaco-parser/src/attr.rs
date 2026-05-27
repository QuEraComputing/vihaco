use proc_macro2::Span;
use syn::{spanned::Spanned, Attribute, Error, Fields, LitStr, Meta, Result, Variant};

// --- Enum-level ---

pub enum HeadAttr {
    Auto,           // #[head]
    Custom(String), // #[head = "X::"]
}

pub struct EnumAttrs {
    pub head: Option<HeadAttr>,
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
        for attr in attrs {
            if attr.path().is_ident("head") {
                let span = attr.span();
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
        }
        Ok(Self { head })
    }
}

impl VariantAttrs {
    pub fn from_variant(variant: &Variant) -> Result<Self> {
        let mut token = None;
        let mut delimiters = DelimiterAttrs::default();
        let mut delegate = false;
        let mut delegate_span = None;

        for attr in &variant.attrs {
            let span = attr.span();

            if attr.path().is_ident("token") {
                if let Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        token = Some(s.value());
                        continue;
                    }
                }
                return Err(Error::new(
                    span,
                    "#[token] requires a string value: #[token = \"name\"]",
                ));
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
                            )
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

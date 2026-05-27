use crate::attr::{EnumAttrs, FieldAttrs, HeadAttr, VariantAttrs};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, Fields, Ident, Result, Type};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    let enum_ident = &input.ident;
    let enum_name = enum_ident.to_string();

    let data = match &input.data {
        Data::Enum(e) => e,
        _ => {
            return Err(Error::new_spanned(
                &input,
                "#[derive(Parse)] is only supported on enums",
            ))
        }
    };

    let enum_attrs = EnumAttrs::from_attrs(&input.attrs)?;

    // Resolve head prefix string
    let head_prefix: Option<String> = match &enum_attrs.head {
        None => None,
        Some(HeadAttr::Auto) => Some(format!("{}::", enum_name)),
        Some(HeadAttr::Custom(s)) => Some(s.clone()),
    };

    // Parse all variant attrs + compute tokens
    let mut variant_data: Vec<(String, VariantAttrs, Vec<FieldAttrs>)> = vec![];
    for variant in &data.variants {
        let vattrs = VariantAttrs::from_variant(variant)?;
        let field_attrs: Vec<FieldAttrs> = variant
            .fields
            .iter()
            .map(FieldAttrs::from_field)
            .collect::<Result<_>>()?;

        // Validate: #[parse_with] on a #[delegate] variant
        if vattrs.delegate {
            for fa in &field_attrs {
                if fa.parse_with.is_some() {
                    return Err(Error::new(
                        vattrs.delegate_span.unwrap(),
                        "#[delegate] cannot be combined with #[parse_with] on a field",
                    ));
                }
            }
        }

        let token = compute_token(
            head_prefix.as_deref(),
            &variant.ident.to_string(),
            vattrs.token.as_deref(),
        );
        variant_data.push((token, vattrs, field_attrs));
    }

    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            enum_ident,
            "#[derive(Parse)] requires at least one variant",
        ));
    }

    // Validate: #[delegate] variants must come after all token-bearing variants
    let mut found_delegate = false;
    for (_, vattrs, _) in &variant_data {
        if vattrs.delegate {
            found_delegate = true;
        } else if found_delegate {
            return Err(Error::new_spanned(
                enum_ident,
                "#[delegate] variants must be declared after all token-bearing variants",
            ));
        }
    }

    // Validate prefix ordering among token-bearing variants
    let token_strs: Vec<&str> = variant_data
        .iter()
        .filter(|(_, va, _)| !va.delegate)
        .map(|(t, _, _)| t.as_str())
        .collect();
    if let Err((i, j)) = check_prefix_order(&token_strs) {
        return Err(Error::new_spanned(
            enum_ident,
            format!(
                "token `{}` is a prefix of `{}` declared after it — reorder so longer tokens come first",
                token_strs[i], token_strs[j]
            ),
        ));
    }

    // Generate variant parser bindings
    let variant_bindings: Vec<TokenStream2> = data
        .variants
        .iter()
        .zip(variant_data.iter())
        .map(|(variant, (token, vattrs, field_attrs))| {
            let input = VariantParserInput {
                enum_ident,
                variant_ident: &variant.ident,
                token,
                fields: &variant.fields,
                field_parse_withs: field_attrs.iter().map(|fa| fa.parse_with.clone()).collect(),
                delimiters: &vattrs.delimiters,
                delegate: vattrs.delegate,
            };
            generate_variant_parser(&input)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    // Build the `.or()` chain — but for enums with many variants the nested
    // `Or<Or<Or<...>>>` type can blow the trait-resolver recursion limit
    // (observed at ~41 variants). `choice((a, b, c, …))` is a flat alternative
    // up to 26 elements. Above that, fall back to a Boxed `.or()` chain.
    let var_names: Vec<syn::Ident> = data
        .variants
        .iter()
        .map(|v| format_ident!("variant_{}", v.ident.to_string().to_lowercase()))
        .collect();

    let or_chain = if var_names.len() == 1 {
        // chumsky's `choice` is only impl'd for tuples of size 2..=26;
        // a single-variant enum just yields its sole binding directly.
        let only = &var_names[0];
        quote! { #only }
    } else if var_names.len() <= 26 {
        quote! { ::chumsky::primitive::choice((#(#var_names),*)) }
    } else {
        // Fall back: chunk into groups of 26 with `choice`, then `.or()` between
        // the chunks. `.boxed()` flattens the type at each chunk boundary so the
        // trait-resolver doesn't recurse on a giant nested tuple.
        let chunks: Vec<TokenStream2> = var_names
            .chunks(26)
            .map(|chunk| {
                let parts = chunk.iter();
                quote! { ::chumsky::primitive::choice((#(#parts),*)).boxed() }
            })
            .collect();
        let first = &chunks[0];
        chunks[1..]
            .iter()
            .fold(quote! { #first }, |acc, c| quote! { #acc.or(#c) })
    };

    // Check if any variant has fields (needs `ws`)
    let needs_ws = data
        .variants
        .iter()
        .any(|v| !matches!(v.fields, Fields::Unit));

    let ws_binding = if needs_ws {
        // `text::whitespace()` returns `Repeated<...>` which implements `Parser<_, ()>` and is Clone.
        quote! {
            let ws = ::chumsky::text::whitespace::<
                &'src str,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            >();
        }
    } else {
        quote! {}
    };

    let output = quote! {
        impl<'src> ::vihaco_parser_core::Parse<'src> for #enum_ident {
            fn parser() -> impl ::chumsky::Parser<
                'src,
                &'src str,
                Self,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            > {
                use ::chumsky::Parser as _;
                #ws_binding
                #(#variant_bindings)*
                #or_chain
            }
        }
    };

    Ok(output.into())
}

/// Build the left-nested `.then()` chain for N fields with separators between them.
/// Returns `(chain_expr, destructure_pattern, constructor_args)`.
///
/// For 1 field: `(field_parser, a, [a])`
/// For 2 fields: `(p1.then_ignore(sep).then(p2), (a, b), [a, b])`
/// For 3 fields: `(p1.then_ignore(sep).then(p2).then_ignore(sep).then(p3), ((a, b), c), [a, b, c])`
pub fn build_field_chain(
    field_parsers: &[TokenStream2], // one expr per field
    sep: &str,                      // separator string (e.g. ",")
) -> (TokenStream2, TokenStream2, Vec<syn::Ident>) {
    assert!(!field_parsers.is_empty());

    // Whitespace-only separator (e.g. " ") means "one or more whitespace chars
    // between fields" — `just(' ').padded()` is broken for this because
    // `.padded()` consumes the lone literal as leading whitespace before the
    // inner `just` runs. Emit `text::whitespace().at_least(1).ignored()`
    // instead so multi-whitespace stretches between operands work too.
    let sep_expr = if !sep.is_empty() && sep.chars().all(char::is_whitespace) {
        quote! {
            ::chumsky::text::whitespace::<
                &'src str,
                ::chumsky::extra::Err<::chumsky::error::Simple<'src, char>>,
            >().at_least(1)
        }
    } else {
        let sep_chars: Vec<char> = sep.chars().collect();
        let sep_just = if sep_chars.len() == 1 {
            let c = sep_chars[0];
            quote! { ::chumsky::primitive::just(#c) }
        } else {
            quote! { ::chumsky::primitive::just(#sep) }
        };
        // `.padded()` strips whitespace on both sides of the separator.
        quote! { #sep_just.padded() }
    };

    // Generate ident names: a, b, c, ...
    let names: Vec<syn::Ident> = (0..field_parsers.len())
        .map(|i| format_ident!("{}", (b'a' + i as u8) as char))
        .collect();

    if field_parsers.len() == 1 {
        let p = &field_parsers[0];
        let n = &names[0];
        return (quote! { #p }, quote! { #n }, names);
    }

    // Build chain: p0.then_ignore(sep).then(p1).then_ignore(sep).then(p2) ...
    let mut chain = {
        let p0 = &field_parsers[0];
        let p1 = &field_parsers[1];
        quote! { #p0.then_ignore(#sep_expr).then(#p1) }
    };
    for p in &field_parsers[2..] {
        chain = quote! { #chain.then_ignore(#sep_expr).then(#p) };
    }

    // Build destructure pattern: left-nested tuples  ((a, b), c)
    let pattern = build_pattern(&names);

    (chain, pattern, names)
}

fn build_pattern(names: &[syn::Ident]) -> TokenStream2 {
    assert!(names.len() >= 2);
    let mut pat = {
        let a = &names[0];
        let b = &names[1];
        quote! { (#a, #b) }
    };
    for n in &names[2..] {
        pat = quote! { (#pat, #n) };
    }
    pat
}

fn delimiter_expr(s: &str) -> TokenStream2 {
    if s.is_empty() {
        quote! { ::chumsky::primitive::empty() }
    } else {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() == 1 {
            let c = chars[0];
            quote! { ::chumsky::primitive::just(#c) }
        } else {
            quote! { ::chumsky::primitive::just(#s) }
        }
    }
}

/// Returns the parser expression for a single field.
/// Uses `parse_with` if specified, otherwise calls `<T as Parse>::parser()`.
pub fn field_parser_expr(ty: &Type, parse_with: Option<&str>) -> syn::Result<TokenStream2> {
    if let Some(path) = parse_with {
        let tokens: proc_macro2::TokenStream = path.parse().map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid parse_with path `{}`: {}", path, e),
            )
        })?;
        Ok(quote! { #tokens() })
    } else {
        Ok(quote! { <#ty as ::vihaco_parser_core::Parse>::parser() })
    }
}

pub struct VariantParserInput<'a> {
    pub enum_ident: &'a Ident,
    pub variant_ident: &'a Ident,
    pub token: &'a str, // full token string (already computed)
    pub fields: &'a Fields,
    pub field_parse_withs: Vec<Option<String>>, // one per field
    pub delimiters: &'a crate::attr::DelimiterAttrs,
    pub delegate: bool,
}

/// Generates: `let <var_name> = <parser_expr>;`
pub fn generate_variant_parser(input: &VariantParserInput) -> syn::Result<TokenStream2> {
    let enum_ident = input.enum_ident;
    let variant_ident = input.variant_ident;
    let token = &input.token;
    let var_name = format_ident!("variant_{}", variant_ident.to_string().to_lowercase());

    // Delegate: just use the inner type's parser directly
    if input.delegate {
        let inner_ty = match input.fields {
            Fields::Unnamed(f) => &f.unnamed[0].ty,
            _ => unreachable!("delegate already validated to be single-field tuple"),
        };
        return Ok(quote! {
            let #var_name = <#inner_ty as ::vihaco_parser_core::Parse>::parser()
                .map(#enum_ident::#variant_ident);
        });
    }

    // Unit variant
    if let Fields::Unit = input.fields {
        return Ok(quote! {
            let #var_name = ::chumsky::primitive::just(#token)
                .map(|_| #enum_ident::#variant_ident);
        });
    }

    // Tuple variant with fields (struct-style variants are not supported)
    let fields = match input.fields {
        Fields::Unnamed(f) => f,
        Fields::Named(_) => unreachable!("named struct variants are not supported by #[derive(Parse)] — this should have been caught in attr.rs validation"),
        Fields::Unit => unreachable!("unit variant handled above"),
    };
    let field_types: Vec<&Type> = fields.unnamed.iter().map(|f| &f.ty).collect();
    debug_assert_eq!(
        field_types.len(),
        input.field_parse_withs.len(),
        "field_parse_withs length must match field count"
    );
    let field_parsers: Vec<TokenStream2> = field_types
        .iter()
        .zip(input.field_parse_withs.iter())
        .map(|(ty, pw)| field_parser_expr(ty, pw.as_deref()))
        .collect::<syn::Result<Vec<_>>>()?;

    let (chain, pattern, names) = build_field_chain(&field_parsers, &input.delimiters.separator);

    let open_expr = delimiter_expr(&input.delimiters.open);
    let close_expr = delimiter_expr(&input.delimiters.close);

    // Trailing-ws-before-close only makes sense when there *is* a close
    // delimiter — `foo(1)` wants to accept `foo(1 )`. With `close = ""` the
    // trailing ws would eat the inter-statement separator (newline between
    // instructions in a function body), so the consumer's body parser can't
    // tell where the canonical instruction ends.
    let close_is_empty = input.delimiters.close.is_empty();
    let trailing_ws = if close_is_empty {
        quote! {}
    } else {
        quote! { .then_ignore(ws.clone()) }
    };

    let map_expr = if names.len() == 1 {
        let n = &names[0];
        quote! { .map(|#n| #enum_ident::#variant_ident(#n)) }
    } else {
        quote! { .map(|#pattern| #enum_ident::#variant_ident(#(#names),*)) }
    };

    Ok(quote! {
        let #var_name = ::chumsky::primitive::just(#token)
            .ignore_then(ws.clone())
            .ignore_then(#open_expr)
            .ignore_then(#chain)
            #trailing_ws
            .then_ignore(#close_expr)
            #map_expr;
    })
}

/// Compute the full token string for a variant.
/// `head` is the resolved head prefix (e.g. `Some("A::")` or `None`).
/// `variant_name` is the Rust identifier (e.g. `"Foo"`).
/// `custom_token` is an override from `#[token = "..."]`.
pub fn compute_token(head: Option<&str>, variant_name: &str, custom_token: Option<&str>) -> String {
    let base = match custom_token {
        Some(t) => t.to_string(),
        None => match head {
            None => variant_name.to_lowercase(),
            Some(_) => variant_name.to_string(), // keep PascalCase when head is present
        },
    };
    match head {
        None => base,
        Some(prefix) => format!("{}{}", prefix, base),
    }
}

/// Check that no token is a strict prefix of a previously-declared token.
/// Returns Err with the offending index pair if a violation is found.
pub fn check_prefix_order(tokens: &[&str]) -> std::result::Result<(), (usize, usize)> {
    for (i, earlier) in tokens.iter().enumerate() {
        for (j, later) in tokens.iter().enumerate() {
            if j <= i {
                continue;
            }
            // Check: does `earlier` appear at the start of `later`?
            if later.starts_with(*earlier) && later.len() > earlier.len() {
                return Err((i, j));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_no_head_defaults_to_lowercase() {
        assert_eq!(compute_token(None, "Foo", None), "foo");
    }
    #[test]
    fn token_no_head_custom_token() {
        assert_eq!(compute_token(None, "Foo", Some("my_foo")), "my_foo");
    }
    #[test]
    fn token_auto_head_keeps_pascal_case() {
        assert_eq!(compute_token(Some("A::"), "Foo", None), "A::Foo");
    }
    #[test]
    fn token_auto_head_custom_token() {
        assert_eq!(
            compute_token(Some("A::"), "Foo", Some("my_foo")),
            "A::my_foo"
        );
    }
    #[test]
    fn token_custom_head() {
        assert_eq!(compute_token(Some("Ns::"), "Bar", None), "Ns::Bar");
    }
    #[test]
    fn prefix_check_passes_when_no_overlap() {
        assert!(check_prefix_order(&["foo", "bar", "baz"]).is_ok());
    }
    #[test]
    fn prefix_check_fails_when_shorter_before_longer() {
        // "foo" declared before "foobar" — "foo" is prefix of "foobar"
        assert!(check_prefix_order(&["foo", "foobar"]).is_err());
    }
    #[test]
    fn prefix_check_passes_when_longer_before_shorter() {
        assert!(check_prefix_order(&["foobar", "foo"]).is_ok());
    }
}

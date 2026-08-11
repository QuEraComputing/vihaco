// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use syn::spanned::Spanned;
use syn::{Field, Ident, LitStr, Result, Type};

use super::syntax::{
    DeviceArgs, Handler, MessageSource, RouteDeclaration, SyntaxArgs, SyntaxDeclaration,
    SyntaxMapping,
};

pub(super) struct FieldMetadata {
    pub(super) ident: Ident,
    pub(super) ty: Type,
    pub(super) device: Option<DeviceArgs>,
    pub(super) syntax: Option<SyntaxArgs>,
    pub(super) loadable: Option<String>,
    pub(super) program: bool,
}

fn validate_loadable_name(name: &str, span: Span) -> Result<()> {
    if name.is_empty() {
        return Err(syn::Error::new(
            span,
            "loadable section name cannot be empty",
        ));
    }
    if name.contains('/') {
        return Err(syn::Error::new(
            span,
            "loadable section name cannot contain `/`",
        ));
    }
    Ok(())
}

pub(super) fn metadata_fields(fields: &[Field]) -> Result<Vec<FieldMetadata>> {
    let mut metadata = Vec::with_capacity(fields.len());
    for field in fields {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new(field.span(), "composite fields must be named"))?;
        let mut device = None;
        let mut loadable = None;
        let mut program = false;
        let mut syntax = None;
        for attr in &field.attrs {
            if attr.path().is_ident("device") {
                if device.is_some() {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("duplicate device attribute on field `{ident}`"),
                    ));
                }
                device = Some(attr.parse_args::<DeviceArgs>()?);
            } else if attr.path().is_ident("loadable") {
                if loadable.is_some() {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("duplicate loadable attribute on field `{ident}`"),
                    ));
                }
                let name = if matches!(&attr.meta, syn::Meta::Path(_)) {
                    ident.to_string()
                } else {
                    attr.parse_args::<LitStr>()?.value()
                };
                validate_loadable_name(&name, attr.span())?;
                loadable = Some(name);
            } else if attr.path().is_ident("program") {
                if program {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("duplicate program attribute on field `{ident}`"),
                    ));
                }
                program = true;
            } else if attr.path().is_ident("syntax") {
                if syntax.is_some() {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("duplicate syntax attribute on field `{ident}`"),
                    ));
                }
                let mut args = match &attr.meta {
                    syn::Meta::Path(_) => SyntaxArgs {
                        aliases: Vec::new(),
                    },
                    syn::Meta::NameValue(value) => {
                        let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(alias),
                            ..
                        }) = &value.value
                        else {
                            return Err(syn::Error::new(
                                value.value.span(),
                                "syntax namespace must be a string literal",
                            ));
                        };
                        SyntaxArgs {
                            aliases: vec![alias.clone()],
                        }
                    }
                    syn::Meta::List(_) => attr.parse_args::<SyntaxArgs>()?,
                };
                if args.aliases.is_empty() {
                    args.aliases
                        .push(syn::LitStr::new(&ident.to_string(), ident.span()));
                }
                syntax = Some(args);
            }
        }
        if loadable.is_some() && device.is_none() {
            return Err(syn::Error::new(
                ident.span(),
                format!("field `{ident}` marked #[loadable] must also be marked #[device(...)]"),
            ));
        }
        metadata.push(FieldMetadata {
            ident,
            ty: field.ty.clone(),
            device,
            syntax,
            loadable,
            program,
        });
    }

    let mut device_codes = BTreeMap::<u8, Ident>::new();
    let mut source_symbols = BTreeMap::<String, Ident>::new();
    let mut loadable_names = BTreeMap::<String, Ident>::new();
    let mut program_field = None;
    for field in &metadata {
        if field.program
            && let Some(previous) = program_field.replace(field.ident.clone())
        {
            return Err(syn::Error::new(
                field.ident.span(),
                format!(
                    "multiple `#[program]` fields: `{previous}` and `{}`",
                    field.ident
                ),
            ));
        }
        let Some(device) = &field.device else {
            continue;
        };
        if let Some(previous) = device_codes.insert(device.code, field.ident.clone()) {
            return Err(syn::Error::new(
                field.ident.span(),
                format!(
                    "duplicate device code 0x{:02X} for fields `{previous}` and `{}`",
                    device.code, field.ident
                ),
            ));
        }
        insert_source_symbol(&mut source_symbols, field.ident.to_string(), &field.ident)?;
        let mut aliases = BTreeSet::new();
        for alias in &device.aliases {
            let name = alias.value();
            if !aliases.insert(name.clone()) {
                return Err(syn::Error::new(
                    alias.span(),
                    format!("duplicate alias `{name}` on field `{}`", field.ident),
                ));
            }
            insert_source_symbol(&mut source_symbols, name, &field.ident)?;
        }
        if let Some(name) = &field.loadable
            && let Some(previous) = loadable_names.insert(name.clone(), field.ident.clone())
        {
            return Err(syn::Error::new(
                field.ident.span(),
                format!(
                    "duplicate loadable section name `{name}` for fields `{previous}` and `{}`",
                    field.ident
                ),
            ));
        }
    }
    Ok(metadata)
}

fn insert_source_symbol(
    symbols: &mut BTreeMap<String, Ident>,
    name: String,
    field: &Ident,
) -> Result<()> {
    if let Some(previous) = symbols.insert(name.clone(), field.clone()) {
        return Err(syn::Error::new(
            field.span(),
            format!("duplicate source symbol `{name}` for `{previous}` and `{field}`"),
        ));
    }
    Ok(())
}

pub(super) fn validate_routes(routes: &[RouteDeclaration], fields: &[FieldMetadata]) -> Result<()> {
    let field_names: BTreeSet<_> = fields.iter().map(|field| field.ident.to_string()).collect();
    let mut variants = BTreeSet::new();
    for route in routes {
        if !variants.insert(route.variant.to_string()) {
            return Err(syn::Error::new(
                route.variant.span(),
                format!("duplicate runtime instruction variant `{}`", route.variant),
            ));
        }
        if !field_names.contains(&route.target.to_string()) {
            return Err(syn::Error::new(
                route.target.span(),
                format!("unknown composite field `{}`", route.target),
            ));
        }
        match &route.message {
            MessageSource::From(field) if !field_names.contains(&field.to_string()) => {
                return Err(syn::Error::new(
                    field.span(),
                    format!("unknown composite field `{field}`"),
                ));
            }
            _ => {}
        }
        let mut observer_names = BTreeSet::new();
        for observer in &route.observers {
            if !field_names.contains(&observer.to_string()) {
                return Err(syn::Error::new(
                    observer.span(),
                    format!("unknown observer field `{observer}`"),
                ));
            }
            if !observer_names.insert(observer.to_string()) {
                return Err(syn::Error::new(
                    observer.span(),
                    format!("duplicate observer field `{observer}`"),
                ));
            }
        }
        if let Some(Handler::Absorb(field)) = &route.handler
            && !field_names.contains(&field.to_string())
        {
            return Err(syn::Error::new(
                field.span(),
                format!("unknown effect destination field `{field}`"),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_syntax(
    syntax: &[SyntaxDeclaration],
    routes: &[RouteDeclaration],
) -> Result<()> {
    let mut variants = BTreeSet::new();
    let mut patterns = BTreeSet::new();
    let route_variants: BTreeSet<_> = routes
        .iter()
        .map(|route| route.variant.to_string())
        .collect();

    for entry in syntax {
        if !variants.insert(entry.variant.to_string()) {
            return Err(syn::Error::new(
                entry.variant.span(),
                format!("duplicate syntax instruction variant `{}`", entry.variant),
            ));
        }
        if !patterns.insert(entry.pattern.value()) {
            return Err(syn::Error::new(
                entry.pattern.span(),
                "duplicate syntax instruction pattern",
            ));
        }
        match &entry.mapping {
            SyntaxMapping::Runtime(runtime_variant) => {
                if entry.payload.is_some() {
                    return Err(syn::Error::new(
                        entry.variant.span(),
                        "direct runtime mappings may only be used for unit syntax instructions",
                    ));
                }
                if !route_variants.contains(&runtime_variant.to_string()) {
                    return Err(syn::Error::new(
                        runtime_variant.span(),
                        format!("unknown runtime route `{runtime_variant}`"),
                    ));
                }
            }
            SyntaxMapping::Lower(lowerer) if entry.payload.is_none() => {
                return Err(syn::Error::new(
                    lowerer.span(),
                    "named syntax lowerers require an instruction payload",
                ));
            }
            SyntaxMapping::Lower(_) => {}
        }
    }
    Ok(())
}

pub(super) fn validate_syntax_mounts(fields: &[FieldMetadata]) -> Result<()> {
    let mut namespaces = BTreeMap::<String, Ident>::new();
    for field in fields {
        let Some(syntax) = &field.syntax else {
            continue;
        };
        for alias in &syntax.aliases {
            let namespace = alias.value();
            if syn::parse_str::<Ident>(&namespace).is_err() {
                return Err(syn::Error::new(
                    alias.span(),
                    format!("syntax namespace `{namespace}` must be a Rust identifier"),
                ));
            }
            if let Some(previous) = namespaces.insert(namespace.clone(), field.ident.clone()) {
                return Err(syn::Error::new(
                    alias.span(),
                    format!(
                        "duplicate syntax namespace `{namespace}` for fields `{previous}` and `{}`",
                        field.ident
                    ),
                ));
            }
        }
    }
    Ok(())
}

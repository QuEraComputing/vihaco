// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use syn::spanned::Spanned;
use syn::{Field, Ident, LitStr, Result, Type};

use super::syntax::{DeviceArgs, Handler, MessageSource, RouteDeclaration};

pub(super) struct FieldMetadata {
    pub(super) ident: Ident,
    pub(super) ty: Type,
    pub(super) device: Option<DeviceArgs>,
    pub(super) loadable: Option<String>,
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
            loadable,
        });
    }

    let mut device_codes = BTreeMap::<u8, Ident>::new();
    let mut source_symbols = BTreeMap::<String, Ident>::new();
    let mut loadable_names = BTreeMap::<String, Ident>::new();
    for field in &metadata {
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

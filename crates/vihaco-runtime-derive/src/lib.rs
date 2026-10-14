// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod attr_component;

#[cfg(not(feature = "typed-declarations"))]
mod attr_composite;

mod attr_observe;

#[cfg(feature = "typed-declarations")]
mod attr_typed_composite;

mod common;

#[cfg(not(feature = "typed-declarations"))]
mod component;

mod composite_loader;

mod derive_message;

#[cfg(not(feature = "typed-declarations"))]
use crate::common::strip_vihaco_attrs;
use proc_macro::TokenStream;

#[cfg(not(feature = "typed-declarations"))]
use quote::quote;

#[cfg(not(feature = "typed-declarations"))]
use syn::{Data, DeriveInput, Fields};

/// Mark a runtime-supplied message type with the Message trait.
#[proc_macro_derive(Message, attributes(vihaco))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    derive_message::expand(input)
}

/// Generate legacy device wiring and remove the consumed field helper attributes.
#[cfg(not(feature = "typed-declarations"))]
#[proc_macro_attribute]
pub fn composite(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let original = proc_macro2::TokenStream::from(item.clone());
    let generated = proc_macro2::TokenStream::from(attr_composite::expand(item));
    let mut sanitized: DeriveInput = match syn::parse2(original) {
        Ok(input) => input,
        Err(err) => return err.into_compile_error().into(),
    };

    strip_vihaco_attrs(&mut sanitized.attrs);

    if let Data::Struct(data) = &mut sanitized.data
        && let Fields::Named(fields) = &mut data.fields
    {
        for field in &mut fields.named {
            field.attrs.retain(|attr| {
                let path = attr.path();
                !(path.is_ident("device") || path.is_ident("loadable"))
            });
        }
    }

    quote! {
        #sanitized
        #generated
    }
    .into()
}

/// Adapt an authored execute method to the generated component runtime interface.
#[proc_macro_attribute]
pub fn dispatch(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_component::expand(attr, item)
}

/// Expand the legacy component declaration DSL.
#[proc_macro]
#[cfg(not(feature = "typed-declarations"))]
pub fn component_macro(input: TokenStream) -> TokenStream {
    component::expand(input)
}

/// Preserve the annotated item as the compatibility machine marker.
#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Generate effect observer implementations from the annotated methods.
#[proc_macro_attribute]
pub fn observe(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_observe::expand(attr, item)
}

#[cfg(feature = "typed-declarations")]
mod attr_dialect;

#[cfg(feature = "typed-declarations")]
mod attr_typed_component;

/// Declare complete dialect support on a struct with `#[instructions { dialects }]`.
#[cfg(feature = "typed-declarations")]
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_typed_component::expand(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

/// Declare public runtime instructions and their direct parsers.
///
/// The framework root is discovered automatically from a dependency on `vihaco`
/// or `vihaco-runtime`, including renamed dependencies. Apply
/// `#[vihaco(crate = path)]` to the dialect module to select an explicit root.
/// The path must name a module exposing the framework API and is resolved in
/// the annotated module's parent scope. Relative `self` and `super` paths are
/// supported. Operand types retain their authored scope.
#[cfg(feature = "typed-declarations")]
#[proc_macro_attribute]
pub fn dialect(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_dialect::expand(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

/// Declare typed device fields, complete capabilities, and section boundaries.
#[cfg(feature = "typed-declarations")]
#[proc_macro_attribute]
pub fn composite(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_typed_composite::expand(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

#[cfg(feature = "typed-declarations")]
mod field;

/// Name an eligible composite field or nested route in type position.
#[cfg(feature = "typed-declarations")]
#[proc_macro]
pub fn field(input: TokenStream) -> TokenStream {
    field::expand(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

#[cfg(feature = "typed-declarations")]
mod inventory;

/// Internal collector invoked by generated inventory callbacks.
#[doc(hidden)]
#[cfg(feature = "typed-declarations")]
#[proc_macro]
pub fn __inventory_collect(input: TokenStream) -> TokenStream {
    inventory::collect(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod attr_observe;

mod attr_typed_composite;

mod common;

mod derive_message;

use proc_macro::TokenStream;

/// Mark a runtime-supplied message type with the Message trait.
#[proc_macro_derive(Message, attributes(vihaco))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    derive_message::expand(input)
}

/// Generate effect observer implementations from the annotated methods.
#[proc_macro_attribute]
pub fn observe(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_observe::expand(attr, item)
}

mod attr_dialect;

mod attr_typed_component;

/// Declare complete dialect support on a struct with `#[instructions { dialects }]`.
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
#[proc_macro_attribute]
pub fn dialect(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_dialect::expand(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

/// Declare typed device fields and complete capabilities.
#[proc_macro_attribute]
pub fn composite(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_typed_composite::expand(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

mod field;

/// Name an eligible composite field or nested route in type position.
#[proc_macro]
pub fn field(input: TokenStream) -> TokenStream {
    field::expand(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

mod inventory;

/// Internal collector invoked by generated inventory callbacks.
#[doc(hidden)]
#[proc_macro]
pub fn __inventory_collect(input: TokenStream) -> TokenStream {
    inventory::collect(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

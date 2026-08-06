// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod common;
mod component;
mod composite;

use proc_macro::TokenStream;

#[proc_macro]
pub fn composite(input: TokenStream) -> TokenStream {
    composite::expand(input)
}

#[proc_macro]
/// Declares a reusable runtime component and its instruction types.
///
/// The component state is declared in the first block. An optional `instruction` block declares
/// the owned runtime instruction types that can be executed by the component. The macro generates
/// a public module whose name is the snake-case form of the component name; instruction types are
/// nested in that module's `instruction` namespace.
///
/// State fields without an explicit visibility are available to component implementations in
/// the surrounding module through `pub(super)`. Instruction fields without an explicit visibility
/// are public so composite-generated code can construct them. Names used in state and instruction
/// fields are resolved from the module containing the macro invocation.
///
/// `component!` does not define source syntax, select a machine's instruction set, generate a
/// dispatch implementation, or implement `Execute<I>`. Those responsibilities belong to the
/// composite and component implementation.
///
/// # Example
///
/// ```
/// use vihaco_runtime_derive::component;
///
/// component! {
///     component Counter {
///         value: u64,
///     }
///
///     instruction {
///         Add(u64),
///         Reset,
///     }
/// }
///
/// let _: counter::instruction::Add = counter::instruction::Add(1);
/// let _: counter::instruction::Reset = counter::instruction::Reset;
/// let _: counter::Counter = counter::Counter { value: 0 };
/// ```
///
/// A component with no runtime instruction types may omit the `instruction` block entirely.
pub fn component(input: TokenStream) -> TokenStream {
    component::expand(input)
}

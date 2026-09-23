// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Ident, LitStr};

use super::parse::Instruction;
use super::validation::instruction_mnemonic;

/// Generates the dialect's compile-time instruction enumerator.
///
/// The enumerator is exposed from every generated dialect module under the
/// fixed path `__private::__vihaco_instructions`. It exposes the instruction declarations
/// as tokens to other macros without introducing runtime metadata or requiring
/// procedural macros to inspect previously expanded Rust items (which Rust's
/// macro expansion model does not support).
///
/// A consumer invokes the generated macro with a callback, the path by which
/// the dialect is visible at the invocation site, and arbitrary contextual
/// tokens:
///
/// ```text
/// crate::dialects::arith::__private::__vihaco_instructions! {
///     callback: collect_instruction_descriptors,
///     dialect: { crate::dialects::arith },
///     select: { * },
///     context: { MyComponent },
/// }
/// ```
///
/// The enumerator invokes the callback exactly once and forwards the complete,
/// selected instructions in this shape:
///
/// ```text
/// collect_instruction_descriptors! {
///     dialect: { crate::dialects::arith },
///     context: { MyComponent },
///     instructions: {
///         {
///             ident: { Add },
///             runtime: { crate::dialects::arith::Add },
///             syntax: { crate::dialects::arith::syntax::Add },
///             mnemonic: { "add" },
///             ordinal: { 0 },
///         },
///     },
/// }
/// ```
///
/// Sending the entire list in one callback invocation lets consumers generate
/// complete Rust items, such as enums, parser alternatives, or one static
/// assertion per instruction. `context` is deliberately opaque to this macro:
/// future component and composite macros can pass generics, bounds, route
/// metadata, or other state without changing the dialect-side protocol.
/// `select: { * }` preserves declaration order; `select: { Add, Halt }` preserves
/// the written order and reports names absent from the dialect.
///
/// The caller supplies `dialect` instead of the enumerator embedding the path
/// at its definition site. This preserves aliases and qualified paths chosen
/// by the consumer. Runtime and syntax paths are derived from that same token
/// sequence, so both refer to the same visible dialect module.
///
/// Each descriptor contains:
///
/// - `ident`: the instruction's Rust identifier;
/// - `runtime`: its canonical runtime struct;
/// - `syntax`: its independently parsable surface struct;
/// - `mnemonic`: the explicit pattern's leading mnemonic, or the lowercase
///   instruction name for a generated default pattern; and
/// - `ordinal`: its zero-based position in the dialect declaration.
///
/// The callback is currently restricted to a single macro identifier.
/// `macro_rules!` requires a macro intended for cross-crate use to be exported
/// from the defining crate's root, so the implementation creates a hidden
/// dialect-named `__vihaco_<dialect>_instructions` backing macro there. The
/// dialect defines and re-exports it inside `__private`, separate from its
/// `use super::*` import. Consumers use only that module-qualified re-export.
/// The reserved names and `#[doc(hidden)]` mark it as macro plumbing rather
/// than ordinary public API.
///
/// The return value contains both the backing macro's identifier, which the
/// generated dialect module needs for its re-export, and the backing macro
/// declaration itself.
pub(super) fn expand_instruction_enumerator(
    dialect_name: &Ident,
    instructions: &[Instruction],
) -> (Ident, TokenStream2) {
    let enumerator_name = format_ident!("__vihaco_{}_instructions", dialect_name);
    // Build each descriptor once. Both wildcard forwarding and the per-name
    // selection arms use these tokens, so explicit selection keeps the dialect
    // ordinal and other metadata from the original declaration.
    let instruction_descriptors: Vec<_> = instructions
        .iter()
        .enumerate()
        .map(|(ordinal, instruction)| {
            let instruction_name = &instruction.name;
            let mnemonic = instruction_mnemonic(instruction)
                .unwrap_or_else(|| instruction_name.to_string().to_lowercase());
            let mnemonic = LitStr::new(&mnemonic, instruction_name.span());

            quote! {
                {
                    ident: { #instruction_name },
                    runtime: { $($dialect)+::#instruction_name },
                    syntax: { $($dialect)+::syntax::#instruction_name },
                    mnemonic: { #mnemonic },
                    ordinal: { #ordinal },
                },
            }
        })
        .collect();

    // Generate one matching arm per declared instruction. An arm consumes the
    // first requested name, appends that instruction's descriptor, and recurses
    // on the rest; these arms are inserted before the unknown-name fallback.
    // Recursion uses the caller's module-qualified re-export because `$crate`
    // triggers the macro-export absolute-path lint in the defining crate.
    let selection_arms =
        instructions
            .iter()
            .zip(&instruction_descriptors)
            .map(|(instruction, descriptor)| {
                let name = &instruction.name;
                quote! {
                    (
                        @select
                        callback: $callback:ident,
                        dialect: { $($dialect:tt)+ },
                        context: { $($context:tt)* },
                        remaining: { #name, $($remaining:ident,)* },
                        instructions: { $($instructions:tt)* },
                    ) => {
                        $($dialect)+::__private::__vihaco_instructions! {
                            @select
                            callback: $callback,
                            dialect: { $($dialect)+ },
                            context: { $($context)* },
                            remaining: { $($remaining,)* },
                            instructions: {
                                $($instructions)*
                                #descriptor
                            },
                        }
                    };
                }
            });

    let expansion = quote! {
        #[doc(hidden)]
        #[macro_export]
        macro_rules! #enumerator_name {
            // A wildcard needs no name-by-name matching: the dialect already
            // has its complete descriptor list in declaration order. Forward
            // that list to the callback in one invocation.
            (
                callback: $callback:ident,
                dialect: { $($dialect:tt)+ },
                select: { * },
                context: { $($context:tt)* } $(,)?
            ) => {
                $callback! {
                    dialect: { $($dialect)+ },
                    context: { $($context)* },
                    instructions: {
                        #( #instruction_descriptors )*
                    },
                }
            };
            // Capture the requested identifiers in their written order and
            // start the internal `@select` walk with an empty accumulator. Each
            // recursive step consumes one name and appends its descriptor.
            (
                callback: $callback:ident,
                dialect: { $($dialect:tt)+ },
                select: { $($selected:ident),* $(,)? },
                context: { $($context:tt)* } $(,)?
            ) => {
                $($dialect)+::__private::__vihaco_instructions! {
                    @select
                    callback: $callback,
                    dialect: { $($dialect)+ },
                    context: { $($context)* },
                    remaining: { $($selected,)* },
                    instructions: {},
                }
            };
            // This `quote!` repetition inserts the matching arms generated
            // above, one per declared instruction. `macro_rules!` tries arms
            // in order, so these must precede the general identifier fallback.
            #( #selection_arms )*
            // With no names remaining, the accumulator holds the full ordered
            // selection. Invoke the callback only now, so it receives one
            // complete descriptor list rather than a call for each name.
            (
                @select
                callback: $callback:ident,
                dialect: { $($dialect:tt)+ },
                context: { $($context:tt)* },
                remaining: {},
                instructions: { $($instructions:tt)* },
            ) => {
                $callback! {
                    dialect: { $($dialect)+ },
                    context: { $($context)* },
                    instructions: { $($instructions)* },
                }
            };
            // This arm accepts any remaining identifier that none of the
            // generated instruction arms recognized. Report that name here;
            // the earlier descriptors do not reach the callback on an error.
            (
                @select
                callback: $callback:ident,
                dialect: { $($dialect:tt)+ },
                context: { $($context:tt)* },
                remaining: { $unknown:ident, $($remaining:ident,)* },
                instructions: { $($instructions:tt)* },
            ) => {
                compile_error!(concat!(
                    "unknown instruction `",
                    stringify!($unknown),
                    "` in the selected dialect",
                ));
            };
        }
    };

    (enumerator_name, expansion)
}

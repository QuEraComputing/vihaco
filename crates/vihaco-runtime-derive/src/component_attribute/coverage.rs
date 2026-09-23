// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct};

use super::selector::{Selection, Selector};

/// Generates compile-time coverage checks for selected instructions.
///
/// Explicit names become ordinary type paths in the initial attribute
/// expansion. This gives editors a direct, span-preserving use of each written
/// dialect and instruction name, as well as a direct `Execute<I>` obligation.
/// Wildcard contents are unknown to the procedural macro, so their coverage
/// checks come from the dialect's descriptor enumerator.
///
/// Every selector invokes its enumerator directly instead of through a chain
/// of callbacks. The shared callback emits marker implementations for all
/// canonical runtime types, catching duplicates even across dialect aliases.
/// An anonymous const keeps the helper macros and marker trait local to this
/// component.
pub(super) fn expand_coverage_check(
    component: &ItemStruct,
    root: &TokenStream2,
    selectors: &[Selector],
) -> TokenStream2 {
    let ident = &component.ident;
    let generics = &component.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let explicit_paths: Vec<_> = selectors
        .iter()
        .flat_map(|selector| match &selector.selection {
            Selection::All => Vec::new(),
            Selection::Names(names) => {
                let dialect = &selector.dialect;
                names
                    .iter()
                    .map(|name| quote! { #dialect::#name })
                    .collect()
            }
        })
        .collect();
    let enumerations: Vec<_> = selectors
        .iter()
        .map(|selector| {
            let coverage = match selector.selection {
                Selection::All => quote! { wildcard },
                Selection::Names(_) => quote! { explicit },
            };
            enumerate(
                selector,
                &format_ident!("__vihaco_component_collect"),
                quote! {
                    component: { #ident #ty_generics },
                    generics: { #generics },
                    coverage: { #coverage }
                },
            )
        })
        .collect();

    quote! {
        const _: () = {
            trait __VihacoSelected<I> {}

            // Explicit paths come from the original selector tokens. Keeping
            // them in typed Rust code lets the editor resolve both dialect and
            // instruction names without traversing the enumerator callbacks.
            fn __vihaco_require_explicit<I, C: #root::Execute<I>>() {}
            fn __vihaco_check_explicit #impl_generics () #where_clause {
                #( __vihaco_require_explicit::<#explicit_paths, #ident #ty_generics>(); )*
            }

            // Only wildcard selectors need descriptor-based coverage. Explicit
            // selectors are checked above, avoiding duplicate error messages.
            macro_rules! __vihaco_component_coverage {
                (explicit, { $($runtime:path,)* }) => {};
                (wildcard, { $($runtime:path,)* }) => {
                    const _: () = {
                        #[allow(dead_code)]
                        fn require<I, C: #root::Execute<I>>() {}
                        #[allow(dead_code)]
                        fn check #impl_generics () #where_clause {
                            $( require::<$runtime, #ident #ty_generics>(); )*
                        }
                    };
                };
            }

            // The callback matches the complete dialect descriptor, then uses
            // its canonical runtime path for duplicate detection and wildcard
            // coverage. Each invocation emits whole items in this const block.
            macro_rules! __vihaco_component_collect {
                (
                    dialect: { $($dialect:tt)* },
                    context: {
                        component: { $($component:tt)* },
                        generics: { $($generics:tt)* },
                        coverage: { $coverage:ident }
                    },
                    instructions: {
                        $(
                            {
                                ident: { $name:ident },
                                runtime: { $runtime:path },
                                syntax: { $syntax:path },
                                mnemonic: { $mnemonic:literal },
                                ordinal: { $ordinal:literal },
                            },
                        )*
                    },
                ) => {
                    $( impl #impl_generics __VihacoSelected<$runtime> for #ident #ty_generics #where_clause {} )*
                    __vihaco_component_coverage! {
                        $coverage, { $( $runtime, )* }
                    }
                };
            }
            #( #enumerations )*
        };
    }
}

/// Invokes one dialect's instruction enumerator with a local callback name.
/// The dialect returns canonical descriptor tokens, including runtime paths;
/// this function never reconstructs those paths from selector names.
fn enumerate(selector: &Selector, callback: &Ident, context: TokenStream2) -> TokenStream2 {
    let dialect = &selector.dialect;
    let select = match &selector.selection {
        Selection::All => quote! { * },
        Selection::Names(names) => quote! { #( #names, )* },
    };
    quote! {
        #dialect::__private::__vihaco_instructions! {
            callback: #callback,
            dialect: { #dialect },
            select: { #select },
            context: { #context },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Token, punctuated::Punctuated};

    #[test]
    fn generated_selections_keep_written_order() {
        let selectors: Punctuated<Selector, Token![,]> = syn::parse::Parser::parse2(
            Punctuated::<Selector, Token![,]>::parse_terminated,
            quote! { arith::{Sub, Add}, boolean::*, arith::Halt },
        )
        .unwrap();
        let selections: Vec<_> = selectors
            .iter()
            .map(|selector| {
                enumerate(
                    selector,
                    &format_ident!("callback"),
                    quote! { accumulated: {} },
                )
                .to_string()
            })
            .collect();

        assert!(selections[0].contains("select : { Sub , Add , }"));
        assert!(selections[1].contains("select : { * }"));
        assert!(selections[2].contains("select : { Halt , }"));
    }

    #[test]
    fn explicit_paths_are_emitted_before_independent_enumerators() {
        let expanded = super::super::expand_inner(
            quote! {},
            quote! {
                #[instructions { arith::Add, boolean::Not }]
                struct Device;
            },
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("__vihaco_require_explicit :: < arith :: Add"));
        assert!(expanded.contains("__vihaco_require_explicit :: < boolean :: Not"));
        assert_eq!(expanded.matches("__vihaco_instructions !").count(), 2);
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod enumerator;
mod parse;
mod validation;

#[cfg(test)]
mod tests;

use proc_macro::TokenStream;
use quote::quote;

use crate::common::resolve_root;

use self::enumerator::expand_instruction_enumerator;
use self::parse::Declaration;
use self::validation::validate;

pub fn expand(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as Declaration);
    let root = match resolve_root(&declaration.attrs) {
        Ok(root) => root,
        Err(error) => return error.into_compile_error().into(),
    };
    if let Err(error) = validate(&declaration) {
        return error.into_compile_error().into();
    }

    let Declaration {
        attrs,
        name,
        instructions,
    } = declaration;
    let module_attrs = attrs.iter().filter(|attr| !attr.path().is_ident("vihaco"));
    let syntax_head = name.to_string();
    let (instruction_enumerator_name, instruction_enumerator) =
        expand_instruction_enumerator(&name, &instructions);

    let runtime_structs = instructions.iter().map(|instruction| {
        let docs = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));
        let name = &instruction.name;
        let fields = instruction.fields.iter().map(|field| &field.runtime);

        if instruction.fields.is_empty() {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq)]
                pub struct #name;
            }
        } else {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq)]
                pub struct #name( #(pub #fields),* );
            }
        }
    });

    let syntax_structs = instructions.iter().map(|instruction| {
        let docs = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));
        let pattern = instruction
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("pattern"));
        let name = &instruction.name;
        let fields = instruction.fields.iter().map(|field| &field.syntax);

        if instruction.fields.is_empty() {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq, #root::Parse)]
                #[syntax_class(instruction, head = #syntax_head)]
                #(#pattern)*
                pub struct #name;
            }
        } else {
            quote! {
                #(#docs)*
                #[derive(Clone, Debug, PartialEq, #root::Parse)]
                #[syntax_class(instruction, head = #syntax_head)]
                #(#pattern)*
                pub struct #name( #(pub #fields),* );
            }
        }
    });

    // Stable Rust does not let a `macro_rules!` macro be both module-scoped and
    // publicly available to downstream crates. `#[macro_export]` provides the
    // required cross-crate visibility, but always places the backing macro at
    // the defining crate's root. We therefore re-export that backing macro from
    // the dialect module under the fixed, module-qualified name consumers use.
    //
    // The extra private module is intentional. Referring to a proc-macro-
    // generated `#[macro_export]` macro through `crate::...` is rejected by
    // `macro_expanded_macro_exports_accessed_by_absolute_paths`. Defining and
    // re-exporting it inside `__private` avoids ambiguity with the dialect's
    // `use super::*` while keeping the dialect module first in IDE expansions.
    quote! {
        #(#module_attrs)*
        pub mod #name {
            use super::*;

            #( #runtime_structs )*

            pub mod syntax {
                use super::*;

                #( #syntax_structs )*
            }

            #[doc(hidden)]
            pub mod __private {
                #instruction_enumerator

                pub use #instruction_enumerator_name as __vihaco_instructions;
            }
        }
    }
    .into()
}

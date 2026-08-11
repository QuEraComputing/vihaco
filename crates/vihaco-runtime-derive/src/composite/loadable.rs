// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Generics, Ident};

use super::validate::FieldMetadata;

pub(super) fn generate_loadable_impls(
    root: &TokenStream2,
    name: &Ident,
    generics: &Generics,
    fields: &[FieldMetadata],
) -> TokenStream2 {
    let loadables: Vec<_> = fields
        .iter()
        .filter(|field| field.loadable.is_some())
        .collect();
    let context = format_ident!("__VihacoContext");

    let (_, ty_generics, _) = generics.split_for_impl();
    let own_sst_predicate = quote! {
        #name #ty_generics: #root::loader::LoadSstProgram<#context>
    };
    let sst_method_predicates: Vec<_> = loadables
        .iter()
        .map(|field| {
            let field_ty = &field.ty;
            quote! { #field_ty: #root::loader::LoadSstSubtree<#context> }
        })
        .collect();
    let sst_children: Vec<_> = loadables
        .iter()
        .map(|field| {
            let field_ident = &field.ident;
            let field_ty = &field.ty;
            let section_name = field.loadable.as_ref().expect("loadable field");
            quote! {
                if let ::std::option::Option::Some(child) = section.child(#section_name) {
                    <#field_ty as #root::loader::LoadSstSubtree<#context>>
                        ::load_sst_subtree(&mut self.#field_ident, child)?;
                }
            }
        })
        .collect();
    let loadable_names: Vec<_> = loadables
        .iter()
        .map(|field| field.loadable.as_ref().expect("loadable field").as_str())
        .collect();
    let validate_children = quote! {
        let expected: &[&str] = &[#(#loadable_names),*];
        for child in section.children() {
            let child_name = child.local_name().ok_or_else(|| {
                ::eyre::eyre!(
                    "section `{}` yielded a root section as a child",
                    section.display_path(),
                )
            })?;
            if !expected.iter().any(|expected| *expected == child_name) {
                return Err(::eyre::eyre!(
                    "section `{}` has unexpected child section `{}`",
                    section.display_path(),
                    child.display_path(),
                ));
            }
        }
    };
    let forward_children = quote! {
        #( #sst_children )*
    };

    let mut sst_impl_generics = generics.clone();
    sst_impl_generics.params.push(syn::parse_quote!(#context));
    {
        let where_clause = sst_impl_generics.make_where_clause();
        where_clause
            .predicates
            .push(syn::parse2(own_sst_predicate.clone()).expect("valid predicate"));
        for field in &loadables {
            let field_ty = &field.ty;
            where_clause.predicates.push(
                syn::parse2(quote! {
                    #field_ty: #root::loader::LoadSstSubtree<#context>
                })
                .expect("valid predicate"),
            );
        }
    }
    let (sst_impl_generics, _, sst_where_clause) = sst_impl_generics.split_for_impl();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            pub fn load_generated_sst_children<'__vihaco_sst, #context>(
                &mut self,
                section: #root::SstSectionView<'__vihaco_sst, #context>,
            ) -> ::eyre::Result<()>
            where
                #( #sst_method_predicates ),*
            {
                #validate_children
                #forward_children
                Ok(())
            }
        }

        impl #sst_impl_generics #root::loader::LoadSstSubtree<#context>
            for #name #ty_generics
            #sst_where_clause
        {
            fn load_sst_subtree<'__vihaco_sst>(
                &mut self,
                section: #root::SstSectionView<'__vihaco_sst, #context>,
            ) -> ::eyre::Result<()> {
                #root::loader::LoadSstProgram::<#context>::load_sst_program(
                    self,
                    section.clone(),
                )?;
                #validate_children
                #forward_children
                Ok(())
            }
        }
    }
}

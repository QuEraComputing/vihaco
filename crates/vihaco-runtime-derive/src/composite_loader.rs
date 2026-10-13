// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Section forwarding shared by the current and prepared typed composite emitters.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Lifetime;

pub(crate) struct SectionLoadField {
    pub(crate) field: syn::Ident,
    pub(crate) ty: syn::Type,
    pub(crate) section_name: String,
}

fn method_where_clause(predicates: &[TokenStream2]) -> TokenStream2 {
    if predicates.is_empty() {
        quote! {}
    } else {
        quote! {
            where
                #( #predicates ),*
        }
    }
}

pub(crate) fn emit(
    ident: &syn::Ident,
    generics: &syn::Generics,
    root: &syn::Path,
    loadables: &[SectionLoadField],
    gates: &[syn::Meta],
) -> syn::Result<TokenStream2> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let bc_lifetime = Lifetime::new(
        &format!("'{}", fresh_generic("__vihaco_bc", generics)),
        proc_macro2::Span::call_site(),
    );
    let loadable_context_param = format_ident!("{}", fresh_generic("__VihacoContext", generics));
    let mut loadable_predicates = Vec::<TokenStream2>::new();
    loadable_predicates.push(
        quote! { #ident #ty_generics: #root::loader::LoadOwnBytecodeSection<#loadable_context_param> },
    );
    for loadable in loadables {
        let ty = &loadable.ty;
        loadable_predicates
            .push(quote! { #ty: #root::loader::LoadBytecodeSection<#loadable_context_param> });
    }
    let loadable_method_where = method_where_clause(&loadable_predicates);

    let mut loadable_impl_generics = generics.clone();
    loadable_impl_generics
        .params
        .push(syn::parse_quote!(#loadable_context_param));
    if !loadable_predicates.is_empty() {
        let where_clause = loadable_impl_generics.make_where_clause();
        for predicate in &loadable_predicates {
            where_clause
                .predicates
                .push(syn::parse2(predicate.clone())?);
        }
    }
    let (loadable_impl_generics, _, loadable_where_clause) =
        loadable_impl_generics.split_for_impl();

    let loadable_names: Vec<_> = loadables
        .iter()
        .map(|loadable| loadable.section_name.clone())
        .collect();
    let child_loads: Vec<_> = loadables
        .iter()
        .map(|loadable| {
            let field = &loadable.field;
            let ty = &loadable.ty;
            let name = &loadable.section_name;
            quote! {
                if let ::std::option::Option::Some(__vihaco_child) = section.child(#name) {
                    <#ty as #root::loader::LoadBytecodeSection<#loadable_context_param>>::load_bytecode_section(
                        &mut self.#field,
                        __vihaco_child,
                    )?;
                }
            }
        })
        .collect();

    let loadable_impl = quote! {
        #(#[#gates])*
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn load_generated_bytecode_sections<#bc_lifetime, #loadable_context_param>(
                &mut self,
                section: #root::BytecodeSectionView<#bc_lifetime, #loadable_context_param>,
            ) -> #root::eyre::Result<()>
            #loadable_method_where
            {
                #root::loader::LoadOwnBytecodeSection::<#loadable_context_param>::load_own_bytecode_section(
                    self,
                    section.clone(),
                )?;

                let __vihaco_expected_children: &[&str] = &[#(#loadable_names),*];

                for __vihaco_child in section.children() {
                    let __vihaco_child_name = __vihaco_child.local_name().ok_or_else(|| {
                        #root::eyre::eyre!(
                            "section `{}` yielded a root section as a child",
                            section.display_path(),
                        )
                    })?;
                    if !__vihaco_expected_children
                        .iter()
                        .any(|__vihaco_expected| *__vihaco_expected == __vihaco_child_name)
                    {
                        return Err(#root::eyre::eyre!(
                            "section `{}` has unexpected child section `{}`",
                            section.display_path(),
                            __vihaco_child.display_path(),
                        ));
                    }
                }

                #( #child_loads )*
                Ok(())
            }
        }

        #(#[#gates])*
        impl #loadable_impl_generics #root::loader::LoadBytecodeSection<#loadable_context_param>
            for #ident #ty_generics
            #loadable_where_clause
        {
            fn load_bytecode_section<#bc_lifetime>(
                &mut self,
                section: #root::BytecodeSectionView<#bc_lifetime, #loadable_context_param>,
            ) -> #root::eyre::Result<()> {
                self.load_generated_bytecode_sections(section)
            }
        }
    };

    let mut text_loadable_predicates = Vec::<TokenStream2>::new();
    text_loadable_predicates.push(
        quote! { #ident #ty_generics: #root::loader::LoadOwnSstSection<#loadable_context_param> },
    );
    for loadable in loadables {
        let ty = &loadable.ty;
        text_loadable_predicates
            .push(quote! { #ty: #root::loader::LoadSstSection<#loadable_context_param> });
    }
    let text_loadable_method_where = method_where_clause(&text_loadable_predicates);

    let mut text_loadable_impl_generics = generics.clone();
    text_loadable_impl_generics
        .params
        .push(syn::parse_quote!(#loadable_context_param));
    if !text_loadable_predicates.is_empty() {
        let where_clause = text_loadable_impl_generics.make_where_clause();
        for predicate in &text_loadable_predicates {
            where_clause
                .predicates
                .push(syn::parse2(predicate.clone())?);
        }
    }
    let (text_loadable_impl_generics, _, text_loadable_where_clause) =
        text_loadable_impl_generics.split_for_impl();

    let text_child_loads: Vec<_> = loadables
        .iter()
        .map(|loadable| {
            let field = &loadable.field;
            let ty = &loadable.ty;
            let name = &loadable.section_name;
            quote! {
                if let ::std::option::Option::Some(__vihaco_child) = section.child(#name) {
                    <#ty as #root::loader::LoadSstSection<#loadable_context_param>>::load_sst_section(
                        &mut self.#field,
                        __vihaco_child,
                    )?;
                }
            }
        })
        .collect();

    let text_loadable_impl = quote! {
        #(#[#gates])*
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn load_generated_sst_sections<#bc_lifetime, #loadable_context_param>(
                &mut self,
                section: #root::SstSectionView<#bc_lifetime, #loadable_context_param>,
            ) -> #root::eyre::Result<()>
            #text_loadable_method_where
            {
                #root::loader::LoadOwnSstSection::<#loadable_context_param>::load_own_sst_section(
                    self,
                    section.clone(),
                )?;

                let __vihaco_expected_children: &[&str] = &[#(#loadable_names),*];

                for __vihaco_child in section.children() {
                    let __vihaco_child_name = __vihaco_child.local_name().ok_or_else(|| {
                        #root::eyre::eyre!(
                            "section `{}` yielded a root section as a child",
                            section.display_path(),
                        )
                    })?;
                    if !__vihaco_expected_children
                        .iter()
                        .any(|__vihaco_expected| *__vihaco_expected == __vihaco_child_name)
                    {
                        return Err(#root::eyre::eyre!(
                            "section `{}` has unexpected child section `{}`",
                            section.display_path(),
                            __vihaco_child.display_path(),
                        ));
                    }
                }

                #( #text_child_loads )*
                Ok(())
            }
        }

        #(#[#gates])*
        impl #text_loadable_impl_generics #root::loader::LoadSstSection<#loadable_context_param>
            for #ident #ty_generics
            #text_loadable_where_clause
        {
            fn load_sst_section<#bc_lifetime>(
                &mut self,
                section: #root::SstSectionView<#bc_lifetime, #loadable_context_param>,
            ) -> #root::eyre::Result<()> {
                self.load_generated_sst_sections(section)
            }
        }
    };

    Ok(quote! { #loadable_impl #text_loadable_impl })
}

// Generated method/impl parameters must not shadow authored parameters.
fn fresh_generic(base: &str, generics: &syn::Generics) -> String {
    let names: Vec<_> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Type(p) => p.ident.to_string(),
            syn::GenericParam::Const(p) => p.ident.to_string(),
            syn::GenericParam::Lifetime(p) => p.lifetime.ident.to_string(),
        })
        .collect();
    let mut candidate = base.to_owned();
    while names.contains(&candidate) {
        candidate.push('_');
    }
    candidate
}

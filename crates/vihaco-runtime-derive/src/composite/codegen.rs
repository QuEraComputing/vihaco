// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Field, Generics, Ident, Result, Type};

use super::syntax::{CompositeDeclaration, Handler, MessageSource, RouteDeclaration};
use crate::common::{resolve_root, retain_generics};

pub(super) fn retained_enum_generics(generics: &Generics, routes: &[RouteDeclaration]) -> Generics {
    let payloads: Vec<TokenStream2> = routes
        .iter()
        .map(|route| {
            let payload = &route.payload;
            quote!(#payload)
        })
        .collect();
    retain_generics(generics, &payloads)
}

fn marker_ident(variant: &Ident) -> Ident {
    let name = variant.to_string();
    let name = name.strip_prefix("r#").unwrap_or(&name);
    format_ident!("__VihacoRoute_{name}")
}

fn strip_consumed_field_attrs(mut field: Field) -> Field {
    field.attrs.retain(|attr| {
        !attr.path().is_ident("device")
            && !attr.path().is_ident("loadable")
            && !attr.path().is_ident("program")
    });
    field
}

pub(super) fn try_expand(declaration: CompositeDeclaration) -> Result<TokenStream2> {
    let root = resolve_root(&declaration.attrs)?;
    let fields_metadata = super::validate::metadata_fields(&declaration.fields)?;
    super::validate::validate_routes(&declaration.routes, &fields_metadata)?;

    let CompositeDeclaration {
        mut attrs,
        visibility,
        name,
        generics,
        error,
        fields,
        routes,
    } = declaration;
    crate::common::strip_vihaco_attrs(&mut attrs);
    let fields = fields.into_iter().map(strip_consumed_field_attrs);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let instruction_ident = format_ident!("{name}Instruction");
    let route_module = format_ident!("__Vihaco{name}Routes");

    let instruction_declaration = if routes.is_empty() {
        quote! {}
    } else {
        let enum_generics = retained_enum_generics(&generics, &routes);
        let variants = routes.iter().map(|route| {
            let variant = &route.variant;
            let payload = &route.payload;
            quote!(#variant(#payload))
        });
        quote! {
            #[derive(Clone)]
            #[allow(non_camel_case_types)]
            pub enum #instruction_ident #enum_generics {
                #( #variants ),*
            }
        }
    };

    let route_markers = routes.iter().map(|route| {
        let marker = marker_ident(&route.variant);
        quote! {
            #[allow(non_camel_case_types)]
            pub struct #marker;
        }
    });

    let field_ty = |field: &Ident| -> &Type {
        &fields_metadata
            .iter()
            .find(|candidate| candidate.ident == *field)
            .expect("validated composite field")
            .ty
    };

    let handle_impls = routes.iter().map(|route| {
        let marker = marker_ident(&route.variant);
        let target_ty = field_ty(&route.target);
        let payload = &route.payload;
        let effect = quote!(<#target_ty as #root::Execute<#payload>>::Effect);
        let error_type = error.as_ref().expect("validated executable composite");
        let body = match route.handler.as_ref().expect("validated handler") {
            Handler::Absorb(field) => {
                let absorb_ty = field_ty(field);
                quote! {
                    <#absorb_ty as #root::Absorb<#effect>>::absorb(&mut self.#field, effect)
                        .map_err(::std::convert::Into::<#error_type>::into)
                }
            }
            Handler::With(method) => quote! {
                self.#method(effect).map_err(::std::convert::Into::<#error_type>::into)
            },
        };
        quote! {
            impl #impl_generics #root::Handle<#effect, #route_module::#marker>
                for #name #ty_generics #where_clause
            {
                type Error = #error_type;

                fn handle(&mut self, effect: #effect) -> ::std::result::Result<(), Self::Error> {
                    #body
                }
            }
        }
    });

    let dispatch_arms = routes.iter().map(|route| {
        let variant = &route.variant;
        let target = &route.target;
        let target_ty = field_ty(target);
        let payload = &route.payload;
        let marker = marker_ident(variant);
        let error_type = error.as_ref().expect("validated executable composite");
        let message = match &route.message {
            MessageSource::None => quote!(#root::NoMessage),
            MessageSource::From(field) => {
                let source_ty = field_ty(field);
                quote! {
                    <#source_ty as #root::Supply<
                        <#target_ty as #root::Execute<#payload>>::Message
                    >>::supply(&mut self.#field)
                        .map_err(::std::convert::Into::<#error_type>::into)?
                }
            }
            MessageSource::With(method) => quote! {
                self.#method(instruction)
                    .map_err(::std::convert::Into::<#error_type>::into)?
            },
        };
        let observers = route.observers.iter().map(|observer| {
            let observer_ty = field_ty(observer);
            quote! {
                <#observer_ty as #root::Observe<
                    <#target_ty as #root::Execute<#payload>>::Effect,
                    #route_module::#marker
                >>::observe(&mut self.#observer, &effect)
                    .map_err(::std::convert::Into::<#error_type>::into)?;
            }
        });
        quote! {
            #instruction_ident::#variant(instruction) => {
                let message = #message;
                let result = <#target_ty as #root::Execute<#payload>>::execute(
                    &mut self.#target,
                    instruction,
                    message,
                )
                .map_err(::std::convert::Into::<#error_type>::into)?;
                for effect in result.effects {
                    #( #observers )*
                    <Self as #root::Handle<
                        <#target_ty as #root::Execute<#payload>>::Effect,
                        #route_module::#marker
                    >>::handle(self, effect)
                    .map_err(::std::convert::Into::<#error_type>::into)?;
                }
                Ok(result.execution)
            }
        }
    });

    let dispatch = if routes.is_empty() {
        quote! {}
    } else {
        let error_type = error.as_ref().expect("validated executable composite");
        let enum_generics = retained_enum_generics(&generics, &routes);
        let (_, enum_ty_generics, _) = enum_generics.split_for_impl();
        quote! {
            #[allow(clippy::useless_conversion)]
            fn execute_generated(
                &mut self,
                instruction: &#instruction_ident #enum_ty_generics,
            ) -> ::std::result::Result<#root::Execution, #error_type> {
                match instruction {
                    #( #dispatch_arms ),*
                }
            }
        }
    };

    let metadata_impl = super::metadata::generate_metadata(
        &root,
        &name,
        &generics,
        &instruction_ident,
        &routes,
        &fields_metadata,
    );
    let loadable_impl =
        super::loadable::generate_loadable_impls(&root, &name, &generics, &fields_metadata);

    Ok(quote! {
        #( #attrs )*
        #visibility struct #name #impl_generics #where_clause {
            #( #fields ),*
        }

        #instruction_declaration

        #[doc(hidden)]
        mod #route_module {
            #( #route_markers )*
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #dispatch
        }

        #( #handle_impls )*

        #metadata_impl
        #loadable_impl
    })
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::{Field, Generics, Ident, Result, Type};

use super::syntax::{
    CompositeDeclaration, Handler, MessageSource, RouteDeclaration, SyntaxDeclaration,
    SyntaxMapping,
};
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

fn syntax_generics(generics: &Generics, syntax: &[SyntaxDeclaration]) -> Generics {
    let payloads = syntax
        .iter()
        .filter_map(|entry| entry.payload.as_ref())
        .map(|payload| quote!(#payload))
        .collect::<Vec<_>>();
    retain_generics(generics, &payloads)
}

fn generate_syntax_module(
    root: &TokenStream2,
    generics: &Generics,
    error: Option<&Type>,
    syntax: &[SyntaxDeclaration],
) -> TokenStream2 {
    if syntax.is_empty() {
        return quote! {};
    }

    let enum_generics = syntax_generics(generics, syntax);
    let variants = syntax.iter().map(|entry| {
        let variant = &entry.variant;
        let pattern = &entry.pattern;
        let payload = entry
            .payload
            .as_ref()
            .map(|payload| quote!((#payload)))
            .unwrap_or_default();
        quote! {
            #[pattern = #pattern]
            #variant #payload
        }
    });
    let error = error
        .map(|error| quote!(#error))
        .unwrap_or_else(|| quote!(::core::convert::Infallible));
    let lowerer_methods = syntax.iter().filter_map(|entry| {
        let SyntaxMapping::Lower(method) = &entry.mapping else {
            return None;
        };
        let payload = entry.payload.as_ref()?;
        Some(quote! {
            fn #method(
                &mut self,
                instruction: #payload,
            ) -> ::std::result::Result<
                ::std::vec::Vec<super::runtime::Instruction>,
                #error,
            >;
        })
    });

    quote! {
        pub mod syntax {
            use super::*;
            #[derive(Clone, #root::Parse)]
            #[syntax_class(instruction)]
            pub enum Instruction #enum_generics {
                #( #variants ),*
            }

            pub trait Resolver {
                #( #lowerer_methods )*
            }
        }
    }
}

fn generate_resolver_traits(
    root: &TokenStream2,
    generics: &Generics,
    error: Option<&Type>,
    routes: &[RouteDeclaration],
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    let Some(error) = error else {
        return quote! {};
    };
    let field_ty = |field: &Ident| -> &Type {
        &fields
            .iter()
            .find(|candidate| candidate.ident == *field)
            .expect("validated composite field")
            .ty
    };
    let message_methods = routes.iter().filter_map(|route| {
        let MessageSource::With(method) = &route.message else {
            return None;
        };
        let target_ty = field_ty(&route.target);
        let payload = &route.payload;
        let message_ty = quote!(<#target_ty as #root::Execute<#payload>>::Message);
        Some(quote! {
            fn #method(
                &mut self,
                instruction: &#payload,
            ) -> ::std::result::Result<#message_ty, #error>;
        })
    });
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    quote! {
        pub trait MessageResolver #impl_generics #where_clause {
            #( #message_methods )*
        }
    }
}

fn generate_surface_lowering(
    module: &Ident,
    instruction_ident: &Ident,
    generics: &Generics,
    syntax: &[SyntaxDeclaration],
    routes: &[RouteDeclaration],
    error: Option<&Type>,
) -> TokenStream2 {
    if syntax.is_empty() || routes.is_empty() || error.is_none() {
        return quote! {};
    }
    let error = error.expect("checked above");
    let route_generics = retained_enum_generics(generics, routes);
    let (_, route_ty_generics, _) = route_generics.split_for_impl();
    let arms = syntax.iter().filter_map(|entry| {
        let variant = &entry.variant;
        let route = match &entry.mapping {
            SyntaxMapping::Runtime(runtime_variant) => routes
                .iter()
                .find(|route| route.variant == *runtime_variant)
                .expect("validated runtime route"),
            SyntaxMapping::Lower(_) => return None,
        };
        let runtime_variant = &route.variant;
        let payload = &route.payload;
        Some(quote! {
            #module::syntax::Instruction::#variant => {
                Ok(vec![#instruction_ident::#runtime_variant(#payload)])
            }
        })
    });
    let lowerer_arms = syntax.iter().filter_map(|entry| {
        let SyntaxMapping::Lower(method) = &entry.mapping else {
            return None;
        };
        let variant = &entry.variant;
        Some(quote! {
            #module::syntax::Instruction::#variant(instruction) => {
                <Self as #module::syntax::Resolver>::#method(self, instruction)
                    .map_err(::std::convert::Into::<#error>::into)
            }
        })
    });
    quote! {
        fn lower_surface_instruction(
            &mut self,
            instruction: &#module::syntax::Instruction,
        ) -> ::std::result::Result<
            ::std::vec::Vec<#instruction_ident #route_ty_generics>,
            #error,
        > {
            match instruction.clone() {
                #( #arms, )*
                #( #lowerer_arms, )*
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_program_loading(
    root: &TokenStream2,
    name: &Ident,
    module: &Ident,
    generics: &Generics,
    error: Option<&Type>,
    syntax: &[SyntaxDeclaration],
    routes: &[RouteDeclaration],
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    let Some(error) = error else {
        return quote! {};
    };
    let Some(program) = fields.iter().find(|field| field.program) else {
        return quote! {};
    };
    if syntax.is_empty() || routes.is_empty() {
        return quote! {};
    }

    let program_field = &program.ident;
    let program_ty = &program.ty;
    let instruction_ident = format_ident!("{name}Instruction");
    let route_generics = retained_enum_generics(generics, routes);
    let (_, route_ty_generics, _) = route_generics.split_for_impl();
    let syntax_generics = syntax_generics(generics, syntax);
    let (_, syntax_ty_generics, _) = syntax_generics.split_for_impl();
    let surface_ty = format_ident!("__VihacoSurfaceType");
    let header_ty = format_ident!("__VihacoHeader");
    let context_ty = format_ident!("__VihacoContext");

    quote! {
        pub fn resolve_parsed<#surface_ty, #header_ty>(
            &mut self,
            parsed: #root::syntax::ParsedModule<
                #module::syntax::Instruction #syntax_ty_generics,
                #surface_ty,
                #header_ty,
            >,
        ) -> ::eyre::Result<
            <#program_ty as #root::BuildProgramModule>::Module,
        >
        where
            #surface_ty: ::std::clone::Clone
                + ::std::convert::Into<
                    <#program_ty as #root::BuildProgramModule>::Type,
                >,
            #program_ty: #root::BuildProgramModule<
                    Instruction = #instruction_ident #route_ty_generics,
                >,
        {
            let mut module = <#program_ty as #root::BuildProgramModule>::empty_module();
            for (function_index, function) in parsed.functions.into_iter().enumerate() {
                let start_address =
                    <#program_ty as #root::BuildProgramModule>::instruction_count(&module);
                for instruction in function.body {
                    let lowered = self.lower_surface_instruction(&instruction)?;
                    <#program_ty as #root::BuildProgramModule>::append_instructions(
                        &mut module,
                        lowered,
                    );
                }
                let end_address =
                    <#program_ty as #root::BuildProgramModule>::instruction_count(&module);
                let function_name =
                    <#program_ty as #root::BuildProgramModule>::intern_string(
                        &mut module,
                        function.name.as_str().to_owned(),
                    );
                let params = function
                    .params
                    .into_iter()
                    .map(|param| {
                        let name = <#program_ty as #root::BuildProgramModule>::intern_string(
                            &mut module,
                            param.name.as_str().to_owned(),
                        );
                        #root::module::Parameter {
                            name,
                            ty: param.ty.into(),
                        }
                    })
                    .collect();
                let ret = function
                    .return_ty
                    .into_iter()
                    .map(::std::convert::Into::into)
                    .collect();
                <#program_ty as #root::BuildProgramModule>::add_function(
                    &mut module,
                    #root::module::FunctionInfo {
                        name: function_name,
                        signature: #root::module::Signature { params, ret },
                        local_count: 0,
                        start_address,
                        end_address,
                        file: 0,
                    },
                );
                if function.name.as_str() == "main" {
                    <#program_ty as #root::BuildProgramModule>::set_main_function(
                        &mut module,
                        Some(function_index as u32),
                    );
                }
            }
            <#program_ty as #root::BuildProgramModule>::finish(module)
                .map_err(::std::convert::Into::<#error>::into)
        }

        pub fn load_parsed<#surface_ty, #header_ty, #context_ty>(
            &mut self,
            parsed: #root::syntax::ParsedModule<
                #module::syntax::Instruction #syntax_ty_generics,
                #surface_ty,
                #header_ty,
            >,
            context: #root::ContextHandle<#context_ty>,
        ) -> ::eyre::Result<()>
        where
            #surface_ty: ::std::clone::Clone
                + ::std::convert::Into<
                    <#program_ty as #root::BuildProgramModule>::Type,
                >,
            #program_ty: #root::BuildProgramModule<
                    Instruction = #instruction_ident #route_ty_generics,
                > + #root::InstallProgramModule<
                    #context_ty,
                    Module = <#program_ty as #root::BuildProgramModule>::Module,
                >,
        {
            let module = self.resolve_parsed(parsed)?;
            <#program_ty as #root::InstallProgramModule<#context_ty>>
                ::install_program_module(&mut self.#program_field, module, context)
                .map_err(::std::convert::Into::<#error>::into)
        }
    }
}

pub(super) fn try_expand(declaration: CompositeDeclaration) -> Result<TokenStream2> {
    let root = resolve_root(&declaration.attrs)?;
    let fields_metadata = super::validate::metadata_fields(&declaration.fields)?;
    super::validate::validate_syntax(&declaration.syntax, &declaration.routes)?;
    super::validate::validate_routes(&declaration.routes, &fields_metadata)?;

    let CompositeDeclaration {
        mut attrs,
        visibility,
        name,
        generics,
        error,
        fields,
        syntax,
        routes,
    } = declaration;
    crate::common::strip_vihaco_attrs(&mut attrs);
    let fields = fields.into_iter().map(strip_consumed_field_attrs);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let instruction_ident = format_ident!("{name}Instruction");
    let route_module = format_ident!("__Vihaco{name}Routes");
    let composite_module = format_ident!("{}", name.to_string().to_case(Case::Snake));

    let runtime_instruction_declaration = if routes.is_empty() {
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

    let syntax_declaration = generate_syntax_module(&root, &generics, error.as_ref(), &syntax);
    let resolver_traits =
        generate_resolver_traits(&root, &generics, error.as_ref(), &routes, &fields_metadata);
    let surface_lowering = generate_surface_lowering(
        &composite_module,
        &instruction_ident,
        &generics,
        &syntax,
        &routes,
        error.as_ref(),
    );
    let program_loading = generate_program_loading(
        &root,
        &name,
        &composite_module,
        &generics,
        error.as_ref(),
        &syntax,
        &routes,
        &fields_metadata,
    );
    let runtime_alias = quote! {};
    let runtime_instruction_alias = if syntax.is_empty() {
        quote! {}
    } else {
        quote! { pub use super::super::#instruction_ident as Instruction; }
    };
    let syntax_alias = if syntax.is_empty() {
        quote! {}
    } else {
        let resolver_ident = format_ident!("{name}SyntaxResolver");
        quote! {
            pub use #composite_module::syntax::Instruction as SurfaceInstruction;
            pub use #composite_module::syntax::Resolver as #resolver_ident;
        }
    };
    let message_alias = if routes.is_empty() {
        quote! {}
    } else {
        let resolver_ident = format_ident!("{name}MessageResolver");
        quote! { pub use #composite_module::runtime::MessageResolver as #resolver_ident; }
    };
    let generated_modules = if syntax.is_empty() && routes.is_empty() {
        quote! {}
    } else {
        quote! {
            pub mod #composite_module {
                use super::*;
                pub mod runtime {
                    use super::*;
                    #runtime_instruction_alias
                    #resolver_traits
                }
                #syntax_declaration
            }
            #runtime_alias
            #syntax_alias
            #message_alias
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

    let handle_impls = routes.iter().filter_map(|route| {
        let handler = route.handler.as_ref()?;
        let marker = marker_ident(&route.variant);
        let target_ty = field_ty(&route.target);
        let payload = &route.payload;
        let effect = quote!(<#target_ty as #root::Execute<#payload>>::Effect);
        let error_type = error.as_ref().expect("validated executable composite");
        let body = match handler {
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
        Some(quote! {
            impl #impl_generics #root::Handle<#effect, #route_module::#marker>
                for #name #ty_generics #where_clause
            {
                type Error = #error_type;

                fn handle(&mut self, effect: #effect) -> ::std::result::Result<(), Self::Error> {
                    #body
                }
            }
        })
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
        let effect_handling = if route.handler.is_some() {
            quote! {
                for effect in result.effects {
                    #( #observers )*
                    <Self as #root::Handle<
                        <#target_ty as #root::Execute<#payload>>::Effect,
                        #route_module::#marker
                    >>::handle(self, effect)
                    .map_err(::std::convert::Into::<#error_type>::into)?;
                }
            }
        } else {
            let no_effect_assertion = quote_spanned! {route.variant.span()=>
                let _: #root::NoEffect = effect;
            };
            quote! {
                for effect in result.effects {
                    #no_effect_assertion
                }
            }
        };
        quote! {
            #instruction_ident::#variant(instruction) => {
                let message = #message;
                let result = <#target_ty as #root::Execute<#payload>>::execute(
                    &mut self.#target,
                    instruction,
                    message,
                )
                .map_err(::std::convert::Into::<#error_type>::into)?;
                #effect_handling
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

        #runtime_instruction_declaration
        #generated_modules

        #[doc(hidden)]
        mod #route_module {
            #( #route_markers )*
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #surface_lowering
            #program_loading
            #dispatch
        }

        #( #handle_impls )*

        #metadata_impl
        #loadable_impl
    })
}

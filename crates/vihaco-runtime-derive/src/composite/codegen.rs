// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::{Field, Generics, Ident, Result, Type};

use super::syntax::{
    CompositeDeclaration, Handler, HeaderDeclaration, MessageSource, ObserverDeclaration,
    RouteDeclaration, SyntaxDeclaration, SyntaxMapping,
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

fn syntax_variant_ident(field: &Ident) -> Ident {
    format_ident!("{}", field.to_string().to_case(Case::Pascal))
}

fn strip_consumed_field_attrs(mut field: Field) -> Field {
    field.attrs.retain(|attr| {
        !attr.path().is_ident("device")
            && !attr.path().is_ident("loadable")
            && !attr.path().is_ident("program")
            && !attr.path().is_ident("syntax")
    });
    field
}

fn generate_observers(
    root: &TokenStream2,
    observers: &[ObserverDeclaration],
    input: &TokenStream2,
    marker: &TokenStream2,
    error: &Type,
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    let field_ty = |field: &Ident| -> &Type {
        &fields
            .iter()
            .find(|candidate| candidate.ident == *field)
            .expect("validated observer field")
            .ty
    };
    let branches = observers.iter().map(|observer| {
        let field = &observer.field;
        let observer_ty = field_ty(field);
        let output = quote!(<#observer_ty as #root::Observe<#input, #marker>>::Effect);
        let nested = generate_observers(root, &observer.observers, &output, marker, error, fields);
        let terminal = if observer.observers.is_empty() && observer.handler.is_none() {
            quote_spanned! {observer.field.span()=>
                let _: #root::NoEffect = effect;
            }
        } else {
            quote! {}
        };
        let handler = observer.handler.as_ref().map(|handler| match handler {
            Handler::With(method) => quote! {
                self.#method(effect)
                    .map_err(::std::convert::Into::<#error>::into)?;
            },
            Handler::Absorb(field) => {
                let destination_ty = field_ty(field);
                quote! {
                    <#destination_ty as #root::Absorb<#output>>::absorb(&mut self.#field, effect)
                        .map_err(::std::convert::Into::<#error>::into)?;
                }
            }
        }).unwrap_or_default();
        quote! {
            for effect in <#observer_ty as #root::Observe<#input, #marker>>::observe(
                &mut self.#field,
                &effect,
            ).map_err(::std::convert::Into::<#error>::into)? {
                #nested
                #handler
                #terminal
            }
        }
    });
    quote!( #( #branches )* )
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
    header: Option<&HeaderDeclaration>,
    syntax: &[SyntaxDeclaration],
    routes: &[RouteDeclaration],
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    let mounts = fields
        .iter()
        .filter(|field| field.syntax.is_some())
        .collect::<Vec<_>>();
    if syntax.is_empty() && mounts.is_empty() && header.is_none() {
        return quote! {};
    }

    let enum_generics = syntax_generics(generics, syntax);
    let component_instruction_variants = mounts
        .iter()
        .map(|field| {
            let variant = syntax_variant_ident(&field.ident);
            let ty = &field.ty;
            quote!(#variant(<#ty as #root::InstructionSet>::Instruction))
        })
        .collect::<Vec<_>>();
    let component_value_variants = mounts
        .iter()
        .map(|field| {
            let variant = syntax_variant_ident(&field.ident);
            let ty = &field.ty;
            quote!(#variant(<#ty as #root::InstructionSet>::Value))
        })
        .collect::<Vec<_>>();
    let component_type_variants = mounts
        .iter()
        .map(|field| {
            let variant = syntax_variant_ident(&field.ident);
            let ty = &field.ty;
            quote!(#variant(<#ty as #root::InstructionSet>::Type))
        })
        .collect::<Vec<_>>();
    let public_variants = syntax
        .iter()
        .map(|entry| {
            let variant = &entry.variant;
            let payload = entry
                .payload
                .as_ref()
                .map(|payload| quote!((#payload)))
                .unwrap_or_default();
            quote!(#variant #payload)
        })
        .collect::<Vec<_>>();
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
    let component_lowerer_methods = routes.iter().next().into_iter().flat_map(|_| {
        mounts.iter().map(|field| {
            let method = format_ident!("lower_{}", field.ident);
            let ty = &field.ty;
            quote! {
                fn #method(
                    &mut self,
                    instruction: <#ty as #root::InstructionSet>::Instruction,
                ) -> ::std::result::Result<
                    ::std::vec::Vec<super::runtime::Instruction>,
                    #error,
                >;
            }
        })
    });
    let header_method = header.iter().map(|header| {
        let ty = &header.ty;
        let method = &header.resolver;
        quote! {
            fn #method(
                &mut self,
                header: #ty,
            ) -> ::std::result::Result<(), #error>;
        }
    });

    let helper_variants = syntax
        .iter()
        .map(|entry| {
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
        })
        .collect::<Vec<_>>();
    let parser_alternatives = mounts
        .iter()
        .flat_map(|field| {
            let variant = syntax_variant_ident(&field.ident);
            let ty = &field.ty;
            field
                .syntax
                .as_ref()
                .expect("validated syntax mount")
                .aliases
                .iter()
                .map(move |alias| {
                    let namespace = alias.value();
                    quote! {
                        #root::namespaced_parser::<
                            <#ty as #root::InstructionSet>::Instruction
                        >(#namespace)
                        .map(Self::#variant)
                    }
                })
        })
        .collect::<Vec<_>>();
    let composite_parser = if syntax.is_empty() {
        None
    } else {
        let arms = syntax.iter().map(|entry| {
            let variant = &entry.variant;
            if entry.payload.is_some() {
                quote!(__VihacoCompositeInstruction::#variant(value) => Self::#variant(value))
            } else {
                quote!(__VihacoCompositeInstruction::#variant => Self::#variant)
            }
        });
        Some(quote! {
            __VihacoCompositeInstruction::parser().map(|instruction| match instruction {
                #( #arms ),*
            })
        })
    };
    let mut parser_alternatives = parser_alternatives;
    if let Some(parser) = composite_parser {
        parser_alternatives.push(parser);
    }
    let parser = parser_alternatives
        .into_iter()
        .reduce(|left, right| quote!(#left.or(#right)))
        .expect("syntax module has a parser alternative");
    let component_value_parser = mounts.iter().flat_map(|field| {
        let variant = syntax_variant_ident(&field.ident);
        let ty = &field.ty;
        field
            .syntax
            .as_ref()
            .expect("validated syntax mount")
            .aliases
            .iter()
            .map(move |alias| {
                let namespace = alias.value();
                quote! {
                    #root::namespaced_parser::<
                        <#ty as #root::InstructionSet>::Value
                    >(#namespace)
                    .map(Self::#variant)
                }
            })
    });
    let component_type_parser = mounts.iter().flat_map(|field| {
        let variant = syntax_variant_ident(&field.ident);
        let ty = &field.ty;
        field
            .syntax
            .as_ref()
            .expect("validated syntax mount")
            .aliases
            .iter()
            .map(move |alias| {
                let namespace = alias.value();
                quote! {
                    #root::namespaced_parser::<
                        <#ty as #root::InstructionSet>::Type
                    >(#namespace)
                    .map(Self::#variant)
                }
            })
    });
    let value_parser = component_value_parser
        .reduce(|left, right| quote!(#left.or(#right)))
        .unwrap_or_else(|| quote!(#root::bare_token().map(|_| unreachable!())));
    let type_parser = component_type_parser
        .reduce(|left, right| quote!(#left.or(#right)))
        .unwrap_or_else(|| quote!(#root::bare_token().map(|_| unreachable!())));
    let helper_declaration = if syntax.is_empty() {
        quote! {}
    } else {
        quote! {
            #[derive(Clone, Debug, PartialEq, #root::Parse)]
            #[syntax_class(instruction)]
            enum __VihacoCompositeInstruction #enum_generics {
                #( #helper_variants ),*
            }
        }
    };

    let header_declaration = header
        .map(|header| {
            let ty = &header.ty;
            quote!(pub type Header = #ty;)
        })
        .unwrap_or_else(|| {
            quote! {
                #[derive(Clone, Debug, PartialEq)]
                pub struct Header;

                impl #root::FromText for Header {
                    fn from_text(_text: &str) -> ::eyre::Result<Self> {
                        Ok(Self)
                    }
                }

                impl #root::SstHeader for Header {}
            }
        });
    let header_parser = header.iter().map(|_| {
        quote! {
            pub fn parse_header<'__vihaco_src, __VihacoContext>(
                section: #root::SstSectionView<'__vihaco_src, __VihacoContext>,
            ) -> ::eyre::Result<Header> {
                section.parse_header::<Header>()
            }
        }
    });

    quote! {
        pub mod syntax {
            use super::*;
            use #root::Parser as _;

            #[derive(Clone, Debug, PartialEq)]
            #[allow(non_camel_case_types)]
            pub enum Instruction #enum_generics {
                #( #component_instruction_variants, )*
                #( #public_variants, )*
            }

            impl #root::SurfaceInstruction for Instruction #enum_generics {}

            impl<'__vihaco_src> #root::Parse<'__vihaco_src> for Instruction #enum_generics {
                fn parser() -> impl #root::Parser<
                    '__vihaco_src,
                    &'__vihaco_src str,
                    Self,
                    #root::extra::Err<#root::Simple<'__vihaco_src, char>>,
                > {
                    #parser
                }
            }

            #[derive(Clone, Debug, PartialEq)]
            pub enum Value {
                #( #component_value_variants, )*
            }

            impl<'__vihaco_src> #root::Parse<'__vihaco_src> for Value {
                fn parser() -> impl #root::Parser<
                    '__vihaco_src,
                    &'__vihaco_src str,
                    Self,
                    #root::extra::Err<#root::Simple<'__vihaco_src, char>>,
                > {
                    #value_parser
                }
            }

            #[derive(Clone, Debug, PartialEq)]
            pub enum Type {
                #( #component_type_variants, )*
            }

            impl<'__vihaco_src> #root::Parse<'__vihaco_src> for Type {
                fn parser() -> impl #root::Parser<
                    '__vihaco_src,
                    &'__vihaco_src str,
                    Self,
                    #root::extra::Err<#root::Simple<'__vihaco_src, char>>,
                > {
                    #type_parser
                }
            }

            #header_declaration
            #( #header_parser )*

            pub struct Module;

            impl #root::ModuleSyntax for Module {
                type Instruction = Instruction #enum_generics;
                type Value = Value;
                type Type = Type;
                type Header = Header;
            }

            pub trait Resolver {
                #( #header_method )*
                #( #lowerer_methods )*
                #( #component_lowerer_methods )*
            }

            #helper_declaration
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
    if error.is_none() {
        return quote! {};
    }
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
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    let mounts = fields
        .iter()
        .filter(|field| field.syntax.is_some())
        .collect::<Vec<_>>();
    if (syntax.is_empty() && mounts.is_empty()) || routes.is_empty() || error.is_none() {
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
    let component_lowerer_arms = mounts.iter().map(|field| {
        let variant = syntax_variant_ident(&field.ident);
        let method = format_ident!("lower_{}", field.ident);
        quote! {
            #module::syntax::Instruction::#variant(instruction) => {
                <Self as #module::syntax::Resolver>::#method(self, instruction)
                    .map_err(::std::convert::Into::<#error>::into)
            }
        }
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
                #( #component_lowerer_arms, )*
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
    header: Option<&HeaderDeclaration>,
    syntax: &[SyntaxDeclaration],
    routes: &[RouteDeclaration],
    fields: &[super::validate::FieldMetadata],
) -> TokenStream2 {
    if error.is_none() {
        return quote! {};
    }
    let Some(program) = fields.iter().find(|field| field.program) else {
        return quote! {};
    };
    let has_component_syntax = fields.iter().any(|field| field.syntax.is_some());
    if (syntax.is_empty() && !has_component_syntax) || routes.is_empty() {
        return quote! {};
    }

    let program_field = &program.ident;
    let program_ty = &program.ty;
    let instruction_ident = format_ident!("{name}Instruction");
    let route_generics = retained_enum_generics(generics, routes);
    let (_, route_ty_generics, _) = route_generics.split_for_impl();
    let context_ty = format_ident!("__VihacoContext");
    let loadable_predicates = fields
        .iter()
        .filter(|field| field.loadable.is_some())
        .map(|field| {
            let field_ty = &field.ty;
            quote! { #field_ty: #root::loader::LoadSstSubtree<#context_ty> }
        });
    let header_resolution = header.iter().map(|header| {
        let method = &header.resolver;
        quote! {
            <Self as #module::syntax::Resolver>::#method(self, parsed.header)
                .map_err(|error| ::eyre::eyre!(
                    "failed to resolve section header: {:?}",
                    error,
                ))?;
        }
    });

    quote! {
        pub fn resolve_parsed(
            &mut self,
            parsed: #root::syntax::ParsedModule<#module::syntax::Module>,
        ) -> ::eyre::Result<
            <#program_ty as #root::BuildProgramModule>::Module,
        >
        where
            #module::syntax::Type: ::std::convert::Into<
                <#program_ty as #root::BuildProgramModule>::Type,
            >,
            #program_ty: #root::BuildProgramModule<
                    Instruction = #instruction_ident #route_ty_generics,
                >,
        {
            let #root::syntax::ParsedModule {
                header,
                functions,
                labels,
                constants,
                strings,
                source_symbols,
            } = parsed;
            let parsed = #root::syntax::ParsedModule {
                header,
                functions,
                labels,
                constants,
                strings,
                source_symbols,
            };
            #( #header_resolution )*
            let mut module = <#program_ty as #root::BuildProgramModule>::empty_module();
            for string in parsed.strings {
                <#program_ty as #root::BuildProgramModule>::intern_string(&mut module, string);
            }
            for constant in parsed.constants {
                <#program_ty as #root::BuildProgramModule>::add_constant(
                    &mut module,
                    constant,
                );
            }
            for source_symbol in parsed.source_symbols {
                <#program_ty as #root::BuildProgramModule>::add_source_symbol(
                    &mut module,
                    #root::module::SourceSymbolInfo {
                        index: source_symbol.index,
                        name: source_symbol.name.as_str().to_owned(),
                    },
                );
            }
            for (function_index, function) in parsed.functions.into_iter().enumerate() {
                let start_address =
                    <#program_ty as #root::BuildProgramModule>::instruction_count(&module);
                for (instruction_index, instruction) in function.body.into_iter().enumerate() {
                    let lowered = self
                        .lower_surface_instruction(&instruction)
                        .map_err(|error| ::eyre::eyre!(
                            "failed to lower function `{}` instruction {} ({:?}): {}",
                            function.name.as_str(),
                            instruction_index,
                            instruction,
                            error,
                        ))?;
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
                for label in parsed.labels.iter().filter(|label| label.function == function.name) {
                    let name = <#program_ty as #root::BuildProgramModule>::intern_string(
                        &mut module,
                        label.name.as_str().to_owned(),
                    );
                    <#program_ty as #root::BuildProgramModule>::add_label(
                        &mut module,
                        #root::module::LabelInfo {
                            address: start_address + label.instruction,
                            name,
                        },
                    );
                }
                if function.name.as_str() == "main" {
                    <#program_ty as #root::BuildProgramModule>::set_main_function(
                        &mut module,
                        Some(function_index as u32),
                    );
                }
            }
            <#program_ty as #root::BuildProgramModule>::finish(module)
        }

        pub fn load_parsed<#context_ty>(
            &mut self,
            parsed: #root::syntax::ParsedModule<#module::syntax::Module>,
            context: #root::ContextHandle<#context_ty>,
        ) -> ::eyre::Result<()>
        where
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
        }

        pub fn load_source<'__vihaco_sst, #context_ty>(
            &mut self,
            section: #root::SstSectionView<'__vihaco_sst, #context_ty>,
        ) -> ::eyre::Result<()>
        where
            #module::syntax::Instruction: #root::Parse<'__vihaco_sst> + '__vihaco_sst,
            #module::syntax::Type: ::std::convert::Into<
                <#program_ty as #root::BuildProgramModule>::Type,
            >,
            #program_ty: #root::BuildProgramModule<
                    Instruction = #instruction_ident #route_ty_generics,
                > + #root::InstallProgramModule<
                    #context_ty,
                    Module = <#program_ty as #root::BuildProgramModule>::Module,
                >,
            #( #loadable_predicates ),*
        {
            let parsed = #root::syntax::ParsedModule::<#module::syntax::Module>
                ::parse_section(section.clone())?;
            let module = self.resolve_parsed(parsed)?;
            let children_section = section.clone();
            self.load_generated_sst_children(children_section)?;
            <#program_ty as #root::InstallProgramModule<#context_ty>>
                ::install_program_module(
                    &mut self.#program_field,
                    module,
                    section.context_handle(),
                )
        }
    }
}

pub(super) fn try_expand(declaration: CompositeDeclaration) -> Result<TokenStream2> {
    let root = resolve_root(&declaration.attrs)?;
    let fields_metadata = super::validate::metadata_fields(&declaration.fields)?;
    super::validate::validate_syntax(&declaration.syntax, &declaration.routes)?;
    super::validate::validate_syntax_mounts(&fields_metadata)?;
    super::validate::validate_routes(&declaration.routes, &fields_metadata)?;

    let CompositeDeclaration {
        mut attrs,
        visibility,
        name,
        generics,
        error,
        fields,
        header,
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

    let syntax_declaration = generate_syntax_module(
        &root,
        &generics,
        error.as_ref(),
        header.as_ref(),
        &syntax,
        &routes,
        &fields_metadata,
    );
    let resolver_traits =
        generate_resolver_traits(&root, &generics, error.as_ref(), &routes, &fields_metadata);
    let surface_lowering = generate_surface_lowering(
        &composite_module,
        &instruction_ident,
        &generics,
        &syntax,
        &routes,
        error.as_ref(),
        &fields_metadata,
    );
    let program_loading = generate_program_loading(
        &root,
        &name,
        &composite_module,
        &generics,
        error.as_ref(),
        header.as_ref(),
        &syntax,
        &routes,
        &fields_metadata,
    );
    let runtime_alias = quote! {};
    let has_component_syntax = fields_metadata.iter().any(|field| field.syntax.is_some());
    let runtime_instruction_alias = if routes.is_empty() {
        quote! {}
    } else {
        quote! { pub use super::super::#instruction_ident as Instruction; }
    };
    let syntax_alias = if syntax.is_empty() && !has_component_syntax {
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
    let generated_modules =
        if syntax.is_empty() && routes.is_empty() && !has_component_syntax && header.is_none() {
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
                <Self as #composite_module::runtime::MessageResolver>::#method(self, instruction)
                    .map_err(::std::convert::Into::<#error_type>::into)?
            },
        };
        let component_effect = quote!(<#target_ty as #root::Execute<#payload>>::Effect);
        let route_marker = quote!(#route_module::#marker);
        let observers = generate_observers(
            &root,
            &route.observers,
            &component_effect,
            &route_marker,
            error_type,
            &fields_metadata,
        );
        let effect_handling = if route.handler.is_some() {
            quote! {
                for effect in result.effects {
                    #observers
                    <Self as #root::Handle<
                        <#target_ty as #root::Execute<#payload>>::Effect,
                        #route_module::#marker
                    >>::handle(self, effect)
                    .map_err(::std::convert::Into::<#error_type>::into)?;
                }
            }
        } else {
            let no_effect_assertion = if route.observers.is_empty() {
                quote_spanned! {route.variant.span()=>
                    let _: #root::NoEffect = effect;
                }
            } else {
                quote! {}
            };
            quote! {
                for effect in result.effects {
                    #observers
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

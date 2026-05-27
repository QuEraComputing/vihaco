use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{ImplItem, ItemImpl, ReturnType, Token, Type};

struct ComponentArgs {
    instruction: syn::Type,
    message: syn::Type,
    outcome: Option<syn::Type>,
    effect: Option<syn::Type>,
}

impl Parse for ComponentArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut instruction = None;
        let mut message = None;
        let mut outcome = None;
        let mut effect = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ty: syn::Type = input.parse()?;
            match ident.to_string().as_str() {
                "instruction" => instruction = Some(ty),
                "message" => message = Some(ty),
                "outcome" => outcome = Some(ty),
                "effect" => effect = Some(ty),
                _ => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "unsupported component argument",
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            instruction: instruction.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing instruction = ...")
            })?,
            message: message.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing message = ...")
            })?,
            outcome,
            effect,
        })
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ComponentArgs);
    let item_impl = syn::parse_macro_input!(item as ItemImpl);
    let instruction_ty = args.instruction;
    let message_ty = args.message;

    let self_ty = &item_impl.self_ty;
    let has_execute = item_impl.items.iter().any(|item| match item {
        ImplItem::Fn(func) => func.sig.ident == "execute" && func.sig.inputs.len() == 3,
        _ => false,
    });
    if !has_execute {
        return syn::Error::new_spanned(
            &item_impl.self_ty,
            "expected fn execute(&mut self, inst, msg)",
        )
        .into_compile_error()
        .into();
    }

    for item in &item_impl.items {
        let ImplItem::Fn(func) = item else {
            continue;
        };
        if func.sig.ident != "execute" || func.sig.inputs.len() != 3 {
            continue;
        }

        let ReturnType::Type(_, ty) = &func.sig.output else {
            return syn::Error::new_spanned(
                &func.sig,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        let Type::Path(type_path) = ty.as_ref() else {
            return syn::Error::new_spanned(
                ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        let Some(result_segment) = type_path.path.segments.last() else {
            return syn::Error::new_spanned(
                ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        if result_segment.ident != "Result" {
            return syn::Error::new_spanned(
                ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        }
        let syn::PathArguments::AngleBracketed(result_args) = &result_segment.arguments else {
            return syn::Error::new_spanned(
                ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        if result_args.args.is_empty() || result_args.args.len() > 2 {
            return syn::Error::new_spanned(
                ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        }
        let success_ty = match result_args.args.first() {
            Some(syn::GenericArgument::Type(success_ty)) => success_ty,
            _ => {
                return syn::Error::new_spanned(
                    ty,
                    "component execute handlers must return Result<Effects<Effect>, Error>",
                )
                .into_compile_error()
                .into();
            }
        };
        let Type::Path(success_path) = success_ty else {
            return syn::Error::new_spanned(
                success_ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        let Some(success_segment) = success_path.path.segments.last() else {
            return syn::Error::new_spanned(
                success_ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        if success_segment.ident != "Effects" {
            return syn::Error::new_spanned(
                success_ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        }
        let syn::PathArguments::AngleBracketed(success_args) = &success_segment.arguments else {
            return syn::Error::new_spanned(
                success_ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        };
        if success_args.args.len() != 1
            || !matches!(
                success_args.args.first(),
                Some(syn::GenericArgument::Type(_))
            )
        {
            return syn::Error::new_spanned(
                success_ty,
                "component execute handlers must return Result<Effects<Effect>, Error>",
            )
            .into_compile_error()
            .into();
        }
    }

    if args.outcome.is_some() && args.effect.is_some() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "use either effect = ... or outcome = ..., not both",
        )
        .into_compile_error()
        .into();
    }

    let effect_ty = args
        .effect
        .or(args.outcome)
        .map(|ty| quote! { #ty })
        .unwrap_or_else(|| quote! { () });

    // split out generics and where clause
    let (impl_generics, _ty_generics, where_clause) = item_impl.generics.split_for_impl();

    quote! {
        #item_impl

        impl #impl_generics ::vihaco::GeneratedComponent for #self_ty #where_clause {
            type Instruction = #instruction_ty;
            type Message = #message_ty;
            type Effect = #effect_ty;

            fn execute_generated(
                &mut self,
                inst: Self::Instruction,
                msg: Self::Message,
            ) -> ::eyre::Result<::vihaco::Effects<Self::Effect>> {
                self.execute(inst, msg)
            }
        }
    }
    .into()
}

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Token};

struct ObserveEntry {
    event_ty: syn::Path,
}

struct DeviceArgs {
    code: u8,
    resolve_with: Option<syn::Ident>,
    aliases: Vec<syn::LitStr>,
    #[allow(dead_code)]
    custom_parser: bool,
    observe_entries: Vec<ObserveEntry>,
}

impl Parse for DeviceArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let code_lit: syn::LitInt = input.parse()?;
        let code = code_lit.base10_parse::<u8>()?;
        let mut resolve_with = None;
        let mut aliases = Vec::new();
        let mut custom_parser = false;
        let mut observe_entries = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() || !input.peek(syn::Ident) {
                break; // trailing comma
            }
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_str() {
                "custom_parser" => {
                    custom_parser = true;
                }
                "observe" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let event_ty: syn::Path = content.parse()?;
                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        let kw: syn::Ident = content.parse()?;
                        if kw != "ctx_with" {
                            return Err(syn::Error::new(kw.span(), "expected `ctx_with`"));
                        }
                        return Err(syn::Error::new(
                            kw.span(),
                            "ctx_with = ... is no longer supported for #[device(..., observe(...))]",
                        ));
                    }
                    observe_entries.push(ObserveEntry { event_ty });
                }
                _ => {
                    input.parse::<Token![=]>()?;
                    match ident.to_string().as_str() {
                        "resolve_with" => {
                            let value: syn::Ident = input.parse()?;
                            resolve_with = Some(value);
                        }
                        "alias" => {
                            aliases.push(input.parse::<syn::LitStr>()?);
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                "unsupported device argument",
                            ));
                        }
                    }
                }
            }
        }
        Ok(Self {
            code,
            resolve_with,
            aliases,
            custom_parser,
            observe_entries,
        })
    }
}

struct SchedulerArgs {
    device: u8,
    instruction: syn::Ident,
}

impl Parse for SchedulerArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut device = None;
        let mut instruction = None;
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match ident.to_string().as_str() {
                "device" => {
                    let lit: syn::LitInt = input.parse()?;
                    device = Some(lit.base10_parse::<u8>()?);
                }
                "instruction" => {
                    instruction = Some(input.parse::<syn::Ident>()?);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "unsupported scheduler argument",
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            device: device.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing device = ...")
            })?,
            instruction: instruction.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing instruction = ...")
            })?,
        })
    }
}

struct SharedArgs {
    core_fields: Vec<syn::Ident>,
}

impl Parse for SharedArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut core_fields = Vec::new();
        while !input.is_empty() {
            core_fields.push(input.parse()?);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { core_fields })
    }
}

fn pascal_case(ident: &syn::Ident) -> syn::Ident {
    let mut out = String::new();
    for part in ident.to_string().split('_') {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    format_ident!("{}", out)
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let data = match input.data {
        Data::Struct(data) => data,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring can only be generated for structs",
            )
            .into_compile_error()
            .into();
        }
    };
    let fields = match data.fields {
        Fields::Named(fields) => fields.named,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "composite wiring requires a struct with named fields",
            )
            .into_compile_error()
            .into();
        }
    };

    let mut scheduler = None;
    for attr in input.attrs {
        if attr.path().is_ident("scheduler") {
            scheduler = Some(match attr.parse_args::<SchedulerArgs>() {
                Ok(args) => args,
                Err(err) => return err.into_compile_error().into(),
            });
        }
    }

    let mut devices = Vec::new();
    // Tracks all observer entries in field declaration order for `deliver_any`.
    // Each entry is either a device observe entry or a standalone observer entry.
    enum ObserverKind {
        Device {
            field: syn::Ident,
            entry: ObserveEntry,
        },
        Standalone {
            field: syn::Ident,
            ty: syn::Path,
        },
    }
    let mut ordered_observers: Vec<ObserverKind> = Vec::new();
    let mut core_fields = BTreeSet::new();
    let mut shared_devices = Vec::new();
    let mut program_field: Option<(syn::Ident, syn::Type)> = None;

    for field in fields {
        let field_ident = field.ident.expect("named field");
        let field_ty = field.ty;
        let mut is_core = false;
        let mut shared_with = None;
        for attr in &field.attrs {
            if attr.path().is_ident("core") {
                is_core = true;
            } else if attr.path().is_ident("shared") {
                shared_with = Some(match attr.parse_args::<SharedArgs>() {
                    Ok(args) => args,
                    Err(err) => return err.into_compile_error().into(),
                });
            } else if attr.path().is_ident("program") {
                program_field = Some((field_ident.clone(), field_ty.clone()));
            }
        }

        for attr in field.attrs {
            if attr.path().is_ident("device") {
                let args = match attr.parse_args::<DeviceArgs>() {
                    Ok(args) => args,
                    Err(err) => return err.into_compile_error().into(),
                };
                if is_core {
                    core_fields.insert(field_ident.clone());
                }
                if let Some(shared_with) = &shared_with {
                    shared_devices.push((
                        field_ident.clone(),
                        field_ty.clone(),
                        args.code,
                        args.resolve_with.clone(),
                        shared_with.core_fields.clone(),
                    ));
                }
                for entry in &args.observe_entries {
                    ordered_observers.push(ObserverKind::Device {
                        field: field_ident.clone(),
                        entry: ObserveEntry {
                            event_ty: entry.event_ty.clone(),
                        },
                    });
                }
                devices.push((field_ident.clone(), field_ty.clone(), args));
            } else if attr.path().is_ident("observe") {
                let paths: syn::punctuated::Punctuated<syn::Path, Token![,]> =
                    match attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated) {
                        Ok(paths) => paths,
                        Err(err) => return err.into_compile_error().into(),
                    };
                let observe_tys: Vec<syn::Path> = paths.into_iter().collect();
                if observe_tys.is_empty() {
                    return syn::Error::new_spanned(
                        attr,
                        "observe(...) requires at least one effect type",
                    )
                    .into_compile_error()
                    .into();
                }
                for ty in &observe_tys {
                    ordered_observers.push(ObserverKind::Standalone {
                        field: field_ident.clone(),
                        ty: ty.clone(),
                    });
                }
            }
        }
    }

    if !shared_devices.is_empty() && scheduler.is_none() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "shared devices require #[scheduler(device = ..., instruction = ...)]",
        )
        .into_compile_error()
        .into();
    }

    let mut seen_device_codes = BTreeMap::<u8, syn::Ident>::new();
    for (field, _, args) in &devices {
        if let Some(existing) = seen_device_codes.insert(args.code, field.clone()) {
            return syn::Error::new(
                field.span(),
                format!(
                    "duplicate device code 0x{:02X} for fields `{}` and `{}`",
                    args.code, existing, field
                ),
            )
            .into_compile_error()
            .into();
        }
    }
    if let Some(scheduler) = &scheduler
        && let Some(existing) = seen_device_codes.get(&scheduler.device)
    {
        return syn::Error::new(
            existing.span(),
            format!(
                "scheduler device code 0x{:02X} conflicts with field `{}`",
                scheduler.device, existing
            ),
        )
        .into_compile_error()
        .into();
    }

    for (field, _, _, _, shared_with) in &shared_devices {
        for core in shared_with {
            if !core_fields.contains(core) {
                return syn::Error::new(
                    core.span(),
                    format!(
                        "shared device `{}` references `{}`, which is not a declared #[core] field",
                        field, core
                    ),
                )
                .into_compile_error()
                .into();
            }
        }
    }

    let mut seen_source_symbols = BTreeMap::<String, syn::Ident>::new();
    if scheduler.is_some() {
        seen_source_symbols.insert("scheduler".to_string(), format_ident!("scheduler"));
    }
    for (field, _, args) in &devices {
        let field_name = field.to_string();
        if let Some(existing) = seen_source_symbols.insert(field_name.clone(), field.clone()) {
            return syn::Error::new(
                field.span(),
                format!(
                    "duplicate source symbol `{}` for `{}` and `{}`",
                    field_name, existing, field
                ),
            )
            .into_compile_error()
            .into();
        }

        let mut local_aliases = BTreeSet::new();
        for alias in &args.aliases {
            let alias_name = alias.value();
            if !local_aliases.insert(alias_name.clone()) {
                return syn::Error::new(
                    alias.span(),
                    format!("duplicate alias `{}` on field `{}`", alias_name, field),
                )
                .into_compile_error()
                .into();
            }
            if let Some(existing) = seen_source_symbols.insert(alias_name.clone(), field.clone()) {
                return syn::Error::new(
                    alias.span(),
                    format!(
                        "duplicate source symbol `{}` for `{}` and `{}`",
                        alias_name, existing, field
                    ),
                )
                .into_compile_error()
                .into();
            }
        }
    }

    let scheduler = scheduler;
    let state_ident = format_ident!("__{}GeneratedState", ident);
    let state_fn_ident = format_ident!("__{}_state", ident.to_string().to_lowercase());
    let machine_instruction_ident = format_ident!("{}Instruction", ident);

    let machine_instruction_variants: Vec<_> = devices
        .iter()
        .map(|(field, field_ty, _)| {
            let variant_ident = pascal_case(field);
            quote! {
                #variant_ident(<#field_ty as ::vihaco::GeneratedComponent>::Instruction)
            }
        })
        .collect();

    let device_entries: Vec<_> = devices
        .iter()
        .map(|(field, _, args)| {
            let name = field.to_string();
            let code = args.code;
            quote! { ::vihaco::metadata::DeviceMetadata { code: #code, name: #name } }
        })
        .collect();
    let source_symbol_alias_entries: Vec<_> = devices
        .iter()
        .flat_map(|(_, _, args)| {
            let code = args.code;
            args.aliases.iter().map(move |alias| {
                quote! {
                    ::vihaco::metadata::SourceSymbolAliasMetadata {
                        name: #alias,
                        device_code: #code,
                    }
                }
            })
        })
        .collect();
    let shared_device_entries: Vec<_> = shared_devices
        .iter()
        .map(|(field, _, code, _, core_fields)| {
            let _field_name = field.to_string();
            let shared_with: Vec<_> = core_fields
                .iter()
                .map(|core| {
                    let name = core.to_string();
                    quote! { #name }
                })
                .collect();
            quote! {
                ::vihaco::metadata::SharedDeviceMetadata {
                    device_code: #code,
                    shared_with: &[ #( #shared_with ),* ],
                }
            }
        })
        .collect();
    let scheduler_metadata_static = scheduler.as_ref().map(|scheduler| {
        let device = scheduler.device;
        let instruction_name = scheduler.instruction.to_string();
        quote! {
            static SCHEDULER: ::vihaco::metadata::SchedulerMetadata = ::vihaco::metadata::SchedulerMetadata {
                device_code: #device,
                instruction_name: #instruction_name,
            };
        }
    });
    let scheduler_metadata_expr = if scheduler.is_some() {
        quote! { Some(&SCHEDULER) }
    } else {
        quote! { None }
    };

    let dispatch_arms: Vec<_> = devices.iter().map(|(field, field_ty, args)| {
        let code = args.code;
        let resolver = args.resolve_with.as_ref();
        let resolve_expr = if let Some(resolver) = resolver {
            quote! { let msg = self.#resolver(&inst)?; }
        } else {
            quote! { let msg = (); }
        };
        quote! {
            #code => {
                let inst = *inst.downcast::<<#field_ty as ::vihaco::GeneratedComponent>::Instruction>()
                    .map_err(|_| ::eyre::eyre!("instruction type mismatch for device {}", #code))?;
                #resolve_expr
                ::vihaco::GeneratedComponent::execute_generated(&mut self.#field, inst, msg)?
                    .map(|effect| Box::new(effect) as Box<dyn ::std::any::Any>)
            }
        }
    }).collect();

    // Generate observe calls in field declaration order so that observers
    // declared earlier in the struct fire before those declared later.
    let observe_calls: Vec<_> = ordered_observers
        .iter()
        .map(|obs| match obs {
            ObserverKind::Device { field, entry } => {
                let ty = &entry.event_ty;
                quote! {
                    if let Some(__e) = effect.downcast_ref::<#ty>() {
                        let __follow_ups = ::vihaco::Observe::<#ty>::observe(&mut self.#field, __e)
                            .map_err(::eyre::Error::from)?;
                        follow_ups.extend(
                            __follow_ups
                                .into_iter()
                                .map(|effect| Box::new(effect) as Box<dyn ::std::any::Any>)
                        );
                        handled = true;
                    }
                }
            }
            ObserverKind::Standalone { field, ty } => {
                quote! {
                    if let Some(__e) = effect.downcast_ref::<#ty>() {
                        let __follow_ups = ::vihaco::Observe::<#ty>::observe(&mut self.#field, __e)
                            .map_err(::eyre::Error::from)?;
                        follow_ups.extend(
                            __follow_ups
                                .into_iter()
                                .map(|effect| Box::new(effect) as Box<dyn ::std::any::Any>)
                        );
                        handled = true;
                    }
                }
            }
        })
        .collect();

    let shared_code = shared_devices.first().map(|(_, _, code, _, _)| *code);
    let program_impl = if let Some((ref field_name, ref field_ty)) = program_field {
        quote! {
            impl ::vihaco::traits::ProgramCounter for #ident {
                type Instruction = <#field_ty as ::vihaco::traits::ProgramCounter>::Instruction;

                fn pc(&self) -> u32 {
                    self.#field_name.pc()
                }

                fn pc_mut(&mut self) -> &mut u32 {
                    self.#field_name.pc_mut()
                }

                fn get_instruction(&self, pc: u32) -> eyre::Result<&Self::Instruction> {
                    self.#field_name.get_instruction(pc)
                }
            }
        }
    } else {
        quote! {}
    };

    let scheduler_methods = if let (Some(scheduler), Some(shared_code)) = (scheduler, shared_code) {
        let scheduler_instruction = scheduler.instruction;
        let _scheduler_device = scheduler.device;
        quote! {
            #[derive(Default)]
            struct #state_ident {
                lock_holder: ::std::option::Option<u8>,
                parked: ::std::collections::BTreeSet<u8>,
            }

            fn #state_fn_ident() -> &'static ::std::sync::Mutex<::std::collections::HashMap<usize, #state_ident>> {
                static STATE: ::std::sync::OnceLock<::std::sync::Mutex<::std::collections::HashMap<usize, #state_ident>>> = ::std::sync::OnceLock::new();
                STATE.get_or_init(|| ::std::sync::Mutex::new(::std::collections::HashMap::new()))
            }

            impl #ident {
                fn __state_key(&self) -> usize {
                    self as *const Self as usize
                }

                fn __with_state<R>(&self, f: impl FnOnce(&mut #state_ident) -> R) -> R {
                    let mut guard = #state_fn_ident().lock().expect("machine scheduler state lock poisoned");
                    let entry = guard.entry(self.__state_key()).or_default();
                    f(entry)
                }

                pub fn scheduler_dispatch(&mut self, core_code: u8, inst: #scheduler_instruction) -> ::eyre::Result<()> {
                    self.__with_state(|state| {
                        match inst {
                            #scheduler_instruction::Acquire => {
                                if state.lock_holder.is_none() {
                                    state.lock_holder = Some(core_code);
                                } else if state.lock_holder != Some(core_code) {
                                    state.parked.insert(core_code);
                                }
                            }
                            #scheduler_instruction::Release => {
                                if state.lock_holder == Some(core_code) {
                                    state.lock_holder = None;
                                    state.parked.remove(&core_code);
                                    let parked: Vec<u8> = state.parked.iter().copied().collect();
                                    for parked_core in parked {
                                        state.parked.remove(&parked_core);
                                    }
                                }
                            }
                        }
                    });
                    Ok(())
                }

                pub fn lock_holder(&self, device_code: u8) -> Option<u8> {
                    if device_code != #shared_code {
                        return None;
                    }
                    self.__with_state(|state| state.lock_holder)
                }

                pub fn core_is_parked(&self, core_code: u8) -> bool {
                    self.__with_state(|state| state.parked.contains(&core_code))
                }

                pub fn dispatch_boxed_as_core(
                    &mut self,
                    core_code: u8,
                    device_code: u8,
                    inst: Box<dyn ::std::any::Any>,
                ) -> ::eyre::Result<()> {
                    if device_code == #shared_code && self.lock_holder(device_code) != Some(core_code) {
                        return Err(::eyre::eyre!("lock required for shared device {}", device_code));
                    }
                    let effects = match device_code {
                        #( #dispatch_arms )*
                        _ => return Err(::eyre::eyre!("unknown device code {}", device_code)),
                    };
                    self.__continue_effects(effects)?;
                    Ok(())
                }
            }
        }
    } else {
        quote! {}
    };

    // `instruction_registry()` and `register_*` plumbing were tied to the
    // legacy vihaco-parser pipeline. The new pipeline (vihaco::syntax +
    // per-consumer resolvers) doesn't need them, so the composite macro no
    // longer emits the registry block.
    let _ = &devices;
    let registry_blocks: Vec<proc_macro2::TokenStream> = Vec::new();

    quote! {
        #[derive(Debug, Clone, ::vihaco::Instruction)]
        pub enum #machine_instruction_ident {
            #( #machine_instruction_variants ),*
        }

        impl ::vihaco::__private::GeneratedMachine for #ident {
            type Instruction = #machine_instruction_ident;

            fn metadata(&self) -> ::vihaco::CompositeMetadata {
                static DEVICES: &[::vihaco::metadata::DeviceMetadata] = &[
                    #( #device_entries ),*
                ];
                static SOURCE_SYMBOL_ALIASES: &[::vihaco::metadata::SourceSymbolAliasMetadata] = &[
                    #( #source_symbol_alias_entries ),*
                ];
                static SHARED_DEVICES: &[::vihaco::metadata::SharedDeviceMetadata] = &[
                    #( #shared_device_entries ),*
                ];
                #scheduler_metadata_static
                ::vihaco::CompositeMetadata {
                    devices: DEVICES,
                    scheduler: #scheduler_metadata_expr,
                    shared_devices: SHARED_DEVICES,
                    source_symbol_aliases: SOURCE_SYMBOL_ALIASES,
                }
            }

            fn deliver_any(&mut self, effect: &dyn ::std::any::Any) -> ::eyre::Result<bool> {
                let (handled, follow_ups) = self.__deliver_effect(effect)?;
                let handled_follow_ups = self.__continue_effects(follow_ups)?;
                Ok(handled || handled_follow_ups)
            }
        }

        #program_impl

        #scheduler_methods

        impl #ident {
            fn __continue_effect(
                &mut self,
                effect: Box<dyn ::std::any::Any>,
            ) -> ::eyre::Result<bool> {
                let (handled, follow_ups) = self.__deliver_effect(effect.as_ref())?;
                let handled_follow_ups = self.__continue_effects(follow_ups)?;
                Ok(handled || handled_follow_ups)
            }

            fn __continue_effects(
                &mut self,
                effects: impl ::std::iter::IntoIterator<Item = Box<dyn ::std::any::Any>>,
            ) -> ::eyre::Result<bool> {
                let mut handled_any = false;
                for effect in effects {
                    handled_any = self.__continue_effect(effect)? || handled_any;
                }
                Ok(handled_any)
            }

            fn __deliver_effect(
                &mut self,
                effect: &dyn ::std::any::Any,
            ) -> ::eyre::Result<(bool, Vec<Box<dyn ::std::any::Any>>)> {
                let mut handled = false;
                let mut follow_ups = Vec::new();
                #( #observe_calls )*
                Ok((handled, follow_ups))
            }

            pub fn dispatch_boxed(
                &mut self,
                device_code: u8,
                inst: Box<dyn ::std::any::Any>,
            ) -> ::eyre::Result<()> {
                let effects = match device_code {
                    #( #dispatch_arms )*
                    _ => return Err(::eyre::eyre!("unknown device code {}", device_code)),
                };
                self.__continue_effects(effects)?;

                Ok(())
            }

            #( #registry_blocks )*
        }
    }
    .into()
}

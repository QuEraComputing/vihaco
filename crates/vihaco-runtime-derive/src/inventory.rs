// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Hidden inventory transport. Producers send names and slots; Rust resolves types.
//!
//! The collector queries one child's `__enumerate!` macro at a time. That producer
//! forwards opaque continuation tokens and appends its inventory records:
//!
//! ```text
//! __inventory_collect! {
//!     resume {
//!         target [composite; ::vihaco; struct Machine<T> where T: Clone;]
//!         requests [(child_macro; Provider; [(cpu ["cpu"])] [Field] [7])]
//!         entries []
//!     }
//!     [([] arithmetic Add 0 [])]
//! }
//! ```
//!
//! `target` retains only the generation kind, framework path, and declaration
//! name/generics/bounds; the unit struct is a signature, never emitted as an item.
//! `requests` keeps the current query first, followed by remaining queries.
//! `entries` contains resolved responses from previous children. On `resume`,
//! the collector removes the current request and combines its provider, field
//! prefix, and device codes with each returned record. Once no requests remain,
//! it emits the inventory associations and, for composites, the flat enums and
//! dispatch implementation. `start` uses the same three sections without a response.
//!
//! Request rows are `(enumerator; provider; [segments] [field] [codes])`.
//! Response rows are `([segments] dialect leaf slot [codes])`; a segment is
//! `(identifier [source_names])`. Collected entries pair a response row with its
//! resolved instruction projection and optional field route. Slots select
//! `LeafAt`/`RouteAt` associations; producers never transport leaf type spellings.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::{
    Fields, Ident, ItemStruct, LitInt, LitStr, Path, Result, Token, Type, Visibility, braced,
    bracketed,
    ext::IdentExt,
    parenthesized,
    parse::{Parse, ParseStream},
};

/// Derive the fixed companion namespace used to query a component or composite inventory.
pub(crate) fn module_ident(name: &Ident) -> Ident {
    format_ident!("__vihaco_inventory_{}", name.unraw(), span = name.span())
}

#[derive(Clone)]
pub(crate) struct Segment {
    pub name: Ident,
    pub source: Vec<LitStr>,
}

impl Parse for Segment {
    /// Read a structural field identifier and its accepted source spellings.
    fn parse(input: ParseStream) -> Result<Self> {
        let body;
        parenthesized!(body in input);
        let name = body.parse()?;
        let source;
        bracketed!(source in body);
        let source = source
            .parse_terminated(<LitStr as Parse>::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { name, source })
    }
}

impl ToTokens for Segment {
    /// Serialize a field segment for inventory transport between macro expansions.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { name, source } = self;
        tokens.extend(quote!((#name [#(#source),*])));
    }
}

#[derive(Clone)]
pub(crate) struct Request {
    pub callback: Path,
    pub provider: Type,
    pub prefix: Vec<Segment>,
    pub field: Option<Type>,
    pub codes: Vec<LitInt>,
}

impl Parse for Request {
    /// Read the child enumerator, provider, and parent routing information for one query.
    fn parse(input: ParseStream) -> Result<Self> {
        let body;
        parenthesized!(body in input);
        let callback = body.parse()?;
        body.parse::<Token![;]>()?;
        let provider = body.parse()?;
        body.parse::<Token![;]>()?;
        let prefix;
        bracketed!(prefix in body);
        let mut segments = Vec::new();
        while !prefix.is_empty() {
            segments.push(prefix.parse()?);
        }
        let field;
        bracketed!(field in body);
        let field = if field.is_empty() {
            None
        } else {
            Some(field.parse()?)
        };
        let codes;
        bracketed!(codes in body);
        let codes = codes
            .parse_terminated(LitInt::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            callback,
            provider,
            prefix: segments,
            field,
            codes,
        })
    }
}

impl ToTokens for Request {
    /// Serialize a pending child query into the continuation state.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            callback,
            provider,
            prefix,
            field,
            codes,
        } = self;
        tokens.extend(quote!((#callback; #provider; [#(#prefix)*] [#field] [#(#codes),*])));
    }
}

struct Record {
    segments: Vec<Segment>,
    dialect: Ident,
    leaf: Ident,
    slot: usize,
    codes: Vec<LitInt>,
}

impl Parse for Record {
    /// Read a producer's structural leaf identity, slot, and device code path.
    fn parse(input: ParseStream) -> Result<Self> {
        let body;
        parenthesized!(body in input);
        let fields;
        bracketed!(fields in body);
        let mut segments = Vec::new();
        while !fields.is_empty() {
            segments.push(fields.parse()?);
        }
        let dialect = body.parse()?;
        let leaf = body.parse()?;
        let slot = body.parse::<LitInt>()?.base10_parse()?;
        let codes;
        bracketed!(codes in body);
        let codes = codes
            .parse_terminated(LitInt::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            segments,
            dialect,
            leaf,
            slot,
            codes,
        })
    }
}

struct Entry {
    record: Record,
    instruction: Type,
    field: Option<Type>,
}

impl Parse for Entry {
    /// Read a collected record with its resolved instruction type and field route.
    fn parse(input: ParseStream) -> Result<Self> {
        let body;
        parenthesized!(body in input);
        let record = body.parse()?;
        let instruction = body.parse()?;
        let field;
        bracketed!(field in body);
        let field = if field.is_empty() {
            None
        } else {
            Some(field.parse()?)
        };
        Ok(Self {
            record,
            instruction,
            field,
        })
    }
}

impl ToTokens for Record {
    /// Serialize leaf metadata without exposing the actual leaf type spelling.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            segments,
            dialect,
            leaf,
            slot,
            codes,
        } = self;
        let slot = LitInt::new(&slot.to_string(), leaf.span());
        tokens.extend(quote!(([#(#segments)*] #dialect #leaf #slot [#(#codes),*])));
    }
}

impl ToTokens for Entry {
    /// Carry a resolved leaf and destination through subsequent child queries.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            record,
            instruction,
            field,
        } = self;
        tokens.extend(quote!((#record #instruction [#field])));
    }
}

struct Target {
    kind: Ident,
    root: Path,
    item: ItemStruct,
}

impl Parse for Target {
    /// Read the generation kind, framework path, and reduced declaration signature.
    fn parse(input: ParseStream) -> Result<Self> {
        let kind = input.parse()?;
        input.parse::<Token![;]>()?;
        let root = input.parse()?;
        input.parse::<Token![;]>()?;
        let item = input.parse()?;
        Ok(Self { kind, root, item })
    }
}

impl ToTokens for Target {
    /// Serialize the information needed to generate the final declaration companions.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { kind, root, item } = self;
        tokens.extend(quote!(#kind; #root; #item));
    }
}

mod section {
    syn::custom_keyword!(target);
    syn::custom_keyword!(requests);
    syn::custom_keyword!(entries);
}

struct Collection {
    target: Target,
    requests: Vec<Request>,
    entries: Vec<Entry>,
}

impl Parse for Collection {
    /// Read the three named continuation sections: target, requests, and entries.
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<section::target>()?;
        let signature;
        bracketed!(signature in input);
        let target = signature.parse()?;
        input.parse::<section::requests>()?;
        let pending;
        bracketed!(pending in input);
        let mut requests = Vec::new();
        while !pending.is_empty() {
            requests.push(pending.parse()?);
        }
        input.parse::<section::entries>()?;
        let collected;
        bracketed!(collected in input);
        let mut entries = Vec::new();
        while !collected.is_empty() {
            entries.push(collected.parse()?);
        }
        Ok(Self {
            target,
            requests,
            entries,
        })
    }
}

impl ToTokens for Collection {
    /// Serialize collector state using the named callback protocol.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            target,
            requests,
            entries,
        } = self;
        tokens.extend(quote!(target [#target] requests [#(#requests)*] entries [#(#entries)*]));
    }
}

/// Begin inventory collection with only the declaration signature and pending child queries.
pub(crate) fn start(
    kind: &str,
    root: &Path,
    item: &ItemStruct,
    requests: Vec<Request>,
) -> TokenStream {
    let kind = Ident::new(kind, item.ident.span());
    // Retain Rust's generic signature, including defaults and where bounds,
    // without carrying authored fields, visibility, or attributes through callbacks.
    let mut signature = item.clone();
    signature.attrs.clear();
    signature.vis = Visibility::Inherited;
    signature.fields = Fields::Unit;
    signature.semi_token = Some(Default::default());
    let collection = Collection {
        target: Target {
            kind,
            root: root.clone(),
            item: signature,
        },
        requests,
        entries: Vec::new(),
    };
    quote!(#root::__private::__inventory_collect! { start {#collection} })
}

/// Resume child inventory queries, resolve leaf and route projections, and emit when complete.
pub(crate) fn collect(input: TokenStream) -> Result<TokenStream> {
    struct Input {
        mode: Ident,
        collection: Collection,
        records: Vec<Record>,
    }

    impl Parse for Input {
        /// Read the collection mode, continuation state, and optional child response.
        fn parse(input: ParseStream) -> Result<Self> {
            let mode = input.parse()?;
            let context;
            braced!(context in input);
            let collection = context.parse()?;
            let mut records = Vec::new();
            if !input.is_empty() {
                let rows;
                bracketed!(rows in input);
                while !rows.is_empty() {
                    records.push(rows.parse()?);
                }
            }
            Ok(Self {
                mode,
                collection,
                records,
            })
        }
    }
    let Input {
        mode,
        mut collection,
        records,
    } = syn::parse2(input)?;
    if mode == "resume" {
        if collection.requests.is_empty() {
            return Err(syn::Error::new(
                mode.span(),
                "inventory response without a pending request",
            ));
        }
        // The first request remains in the continuation until its response arrives.
        let current = collection.requests.remove(0);
        let root = &collection.target.root;
        let provider = &current.provider;
        for mut record in records {
            let slot = LitInt::new(&record.slot.to_string(), record.leaf.span());
            let instruction =
                syn::parse_quote!(<#provider as #root::__private::LeafAt<#slot>>::Instruction);
            let child_route: Option<Type> = if record.segments.is_empty() {
                None
            } else {
                Some(syn::parse_quote!(<#provider as #root::__private::RouteAt<#slot>>::Field))
            };
            let field = match (&current.field, child_route) {
                (Some(head), Some(tail)) => {
                    Some(syn::parse_quote!(#root::__private::FieldPath<#head, #tail>))
                }
                (Some(head), None) => Some(head.clone()),
                (None, tail) => tail,
            };
            record.segments = current
                .prefix
                .iter()
                .cloned()
                .chain(record.segments)
                .collect();
            record.codes = current.codes.iter().cloned().chain(record.codes).collect();
            collection.entries.push(Entry {
                record,
                instruction,
                field,
            });
        }
    } else if mode != "start" {
        return Err(syn::Error::new(
            mode.span(),
            "unknown inventory collection mode",
        ));
    }
    if let Some(request) = collection.requests.first() {
        let callback = &request.callback;
        let root = &collection.target.root;
        Ok(quote!(#callback! { #root::__private::__inventory_collect, {#collection} }))
    } else {
        emit(collection)
    }
}

/// Generate a companion enumerator that returns records while forwarding opaque collector state.
pub(crate) fn producer(module: &Ident, records: TokenStream, provider: bool) -> TokenStream {
    static NEXT_EXPORT: AtomicUsize = AtomicUsize::new(0);
    let suffix = NEXT_EXPORT.fetch_add(1, Ordering::Relaxed);
    let exported = format_ident!("__vihaco_inventory_export_{suffix}");
    let declaration = provider.then(|| {
        quote!(
            #[doc(hidden)]
            pub struct Provider;
        )
    });
    quote! {
        #[doc(hidden)]
        #[macro_export]
        #[allow(non_local_definitions)]
        macro_rules! #exported {
            // The producer only forwards continuation state; the collector owns
            // its grammar and resolves the returned slots against the provider.
            ($callback:path, {$($context:tt)*}) => {
                $callback! { resume {$($context)*} [#records] }
            };
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #module {
            #declaration
            #[doc(hidden)]
            pub use #exported as __enumerate;
        }
    }
}

/// Encode the field, dialect, and leaf names into a readable, unambiguous flat variant name.
fn variant(record: &Record) -> Ident {
    let names = record
        .segments
        .iter()
        .map(|segment| &segment.name)
        .chain([&record.dialect, &record.leaf]);
    let name = names
        .map(|name| name.unraw().to_string().replace('_', "_u"))
        .collect::<Vec<_>>()
        .join("__");
    format_ident!("V_{name}", span = record.leaf.span())
}

/// Publish collected leaf and route associations and generate composite instruction machinery.
fn emit(collection: Collection) -> Result<TokenStream> {
    let Collection {
        target: Target { kind, root, item },
        entries,
        ..
    } = collection;
    let name = &item.ident;
    let module = module_ident(name);
    let provider = quote!(#module::Provider);
    let records = entries
        .iter()
        .enumerate()
        .map(|(slot, entry)| {
            let Record {
                segments,
                dialect,
                leaf,
                codes,
                ..
            } = &entry.record;
            let slot = LitInt::new(&slot.to_string(), leaf.span());
            quote!(([#(#segments)*] #dialect #leaf #slot [#(#codes),*]))
        })
        .collect::<TokenStream>();
    let producer = producer(&module, records, true);
    let associations = entries.iter().enumerate().map(|(slot, entry)| {
        let instruction = &entry.instruction;
        let codes = &entry.record.codes;
        let slot = LitInt::new(&slot.to_string(), entry.record.leaf.span());
        let route = entry.field.as_ref().map(|field| {
            quote! {
                impl #root::__private::RouteAt<#slot> for #provider { type Field = #field; }
            }
        });
        quote! {
            impl #root::__private::LeafAt<#slot> for #provider {
                type Instruction = #instruction;
                const DEVICE_CODES: &'static [u8] = &[#(#codes),*];
            }
            #route
        }
    });
    let aggregate = if kind == "composite" {
        aggregate(&root, &item, &entries)?
    } else {
        TokenStream::new()
    };
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    Ok(quote! {
        #producer
        impl #impl_generics #root::__private::HasInventory for #name #type_generics #where_clause {
            type Provider = #provider;
        }
        #(#associations)* #aggregate
    })
}

/// Generate a phase-aware flat enum, lowering, qualified parsing, and runtime Step dispatch.
fn aggregate(root: &Path, item: &ItemStruct, entries: &[Entry]) -> Result<TokenStream> {
    let name = &item.ident;
    let runtime = format_ident!("__vihaco_{}Instruction", name.unraw());
    let phase = format_ident!("__vihaco_{}InstructionPhase", name.unraw());
    let provider = module_ident(name);
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    let variants: Vec<_> = entries.iter().map(|entry| variant(&entry.record)).collect();
    let types: Vec<Type> = entries
        .iter()
        .enumerate()
        .map(|(slot, entry)| {
            let slot = LitInt::new(&slot.to_string(), entry.record.leaf.span());
            syn::parse_quote!(<#provider::Provider as #root::__private::LeafAt<#slot>>::Instruction)
        })
        .collect();
    let authored = quote!(#root #item #(#types)*);
    let phase_parameter = fresh_aggregate_ident("__VihacoPhase", authored.clone());
    let context_parameter = fresh_aggregate_ident("__VihacoLoader", authored);
    let execution_context = fresh_aggregate_ident("__VihacoExecutionContext", quote!(#root #item));
    let mut dispatch_generics = item.generics.clone();
    dispatch_generics
        .params
        .push(syn::parse_quote!(#execution_context: ?Sized));
    // Use the same nominal route spelling for every instruction at one field.
    // Distinct per-leaf RouteAt projections can hide their equal child types
    // from Rust's solver when combined with context-dependent Step bounds.
    let field_module = crate::field::module_ident(name);
    let fields = entries
        .iter()
        .map(|entry| {
            let mut namespace = quote!(#field_module);
            let mut markers = Vec::new();
            for segment in &entry.record.segments {
                let field = &segment.name;
                markers.push(quote!(#namespace::__markers::#field));
                namespace = quote!(#namespace::__children::#field);
            }
            let mut route = markers.pop().expect("composite destination");
            while let Some(head) = markers.pop() {
                route = quote!(#root::__private::FieldPath<#head, #route>);
            }
            route
        })
        .collect::<Vec<_>>();
    for (field, ty) in fields.iter().zip(&types) {
        dispatch_generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(Self: #root::Step<#field, #ty, #execution_context>));
        dispatch_generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(<Self as #root::Field<#field>>::Child: #root::Execute<#ty>));
    }
    let (dispatch_impl_generics, _, dispatch_where_clause) = dispatch_generics.split_for_impl();
    let mut arms = Vec::new();
    for ((field, variant), ty) in fields.iter().zip(&variants).zip(&types) {
        arms.push(quote!(#runtime::#variant(instruction) => <Self as #root::Step<#field, #ty, #execution_context>>::step(self, instruction, context)));
    }
    let members: Vec<_> = (0..entries.len())
        .map(|i| format_ident!("Instruction{i}"))
        .collect();
    let syntax_types: Vec<_> = types
        .iter()
        .map(|ty| quote!(<#ty as #root::__private::InstructionSpec>::Syntax))
        .collect();
    let (declaration, syntax, parser_impl, lowering) = if entries.is_empty() {
        (
            quote!(pub enum #runtime {}),
            quote!(#runtime),
            quote! {
                impl<'src> #root::Parse<'src> for #runtime {
                    fn parser() -> impl #root::chumsky::Parser<'src, &'src str, Self,
                        #root::chumsky::extra::Err<#root::chumsky::error::Simple<'src, char>>> {
                        use #root::chumsky::Parser as _;
                        #root::chumsky::primitive::empty().try_map(|_, span| -> ::core::result::Result<Self, #root::chumsky::error::Simple<'src, char>> {
                            Err(#root::chumsky::error::Simple::new(None, span))
                        })
                    }
                }
            },
            quote! {
                impl<#context_parameter: ?Sized> #root::Lower<#context_parameter> for #runtime {
                    type Runtime = Self;
                    fn lower(self, _: &mut #context_parameter) -> #root::eyre::Result<Self> { match self {} }
                }
            },
        )
    } else {
        let parsers = entries.iter().zip(&variants).zip(&members).map(|((entry, variant), member)| {
            let mut prefix = quote!(#root::chumsky::primitive::empty());
            for segment in &entry.record.segments {
                let source = &segment.source;
                let branches = source.iter().map(|source| quote!(#root::chumsky::primitive::just(#source).then_ignore(#root::chumsky::primitive::just("::"))));
                prefix = quote!(#prefix.ignore_then(#root::chumsky::primitive::choice(vec![#(#branches),*])));
            }
            let dialect = LitStr::new(&entry.record.dialect.unraw().to_string(), entry.record.dialect.span());
            quote!(#prefix.ignore_then(#root::chumsky::primitive::just(#dialect))
                .then_ignore(#root::chumsky::primitive::just("::"))
                .ignore_then(<#phase_parameter::#member as #root::Parse<'src>>::parser()).map(Self::#variant).boxed())
        }).collect::<Vec<_>>();
        let lower_arms = variants.iter().zip(&syntax_types).map(|(variant, ty)| quote! {
            Self::#variant(instruction) => Ok(#runtime::#variant(<#ty as #root::Lower<#context_parameter>>::lower(instruction, context)?))
        });
        (
            quote! {
                pub enum #runtime<#phase_parameter: #phase = #root::Runtime> { #(#variants(#phase_parameter::#members)),* }
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                pub trait #phase: ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq {
                    #(type #members: ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq;)*
                }
                impl #phase for #root::Syntax { #(type #members = #syntax_types;)* }
                impl #phase for #root::Runtime { #(type #members = #types;)* }
            },
            quote!(#runtime<#root::Syntax>),
            quote! {
                impl<'src, #phase_parameter: #phase + 'src> #root::Parse<'src> for #runtime<#phase_parameter>
                where #(#phase_parameter::#members: #root::Parse<'src> + 'src),* {
                    fn parser() -> impl #root::chumsky::Parser<'src, &'src str, Self,
                        #root::chumsky::extra::Err<#root::chumsky::error::Simple<'src, char>>> {
                        use #root::chumsky::Parser as _;
                        #root::chumsky::primitive::choice(vec![#(#parsers),*])
                    }
                }
            },
            quote! {
                impl<#context_parameter: ?Sized> #root::Lower<#context_parameter> for #runtime<#root::Syntax>
                where #(#syntax_types: #root::Lower<#context_parameter, Runtime = #types>),* {
                    type Runtime = #runtime;
                    fn lower(self, context: &mut #context_parameter) -> #root::eyre::Result<Self::Runtime> {
                        match self { #(#lower_arms),* }
                    }
                }
            },
        )
    };
    let dispatch_body = if entries.is_empty() {
        quote!(match *instruction {})
    } else {
        quote!(match instruction { #(#arms),* })
    };
    Ok(quote! {
        #[doc(hidden)] #[allow(non_camel_case_types, clippy::enum_variant_names)] #[derive(Clone, Debug, PartialEq)]
        #declaration

        impl #impl_generics #root::HasInstructionSet for #name #type_generics #where_clause {
            type Runtime = #runtime;
            type Syntax = #syntax;
        }
        #parser_impl
        #lowering
        impl #root::SurfaceInstruction for #syntax {}

        impl #dispatch_impl_generics #root::Dispatch<#execution_context> for #name #type_generics #dispatch_where_clause {
            type Instruction = #runtime;
            #[inline(always)]
            fn dispatch(&mut self, instruction: &#runtime, context: &#execution_context) -> #root::eyre::Result<()> {
                #dispatch_body
            }
        }
    })
}

/// Keep generated aggregate parameters distinct from authored framework paths.
pub(crate) fn fresh_aggregate_ident(base: &str, tokens: TokenStream) -> Ident {
    fn collect(tokens: TokenStream, names: &mut std::collections::HashSet<String>) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(ident) => {
                    names.insert(ident.to_string());
                }
                proc_macro2::TokenTree::Group(group) => collect(group.stream(), names),
                _ => {}
            }
        }
    }
    let mut names = std::collections::HashSet::new();
    collect(tokens, &mut names);
    let mut candidate = base.to_owned();
    while names.contains(&candidate) {
        candidate.push('_');
    }
    Ident::new(&candidate, proc_macro2::Span::call_site())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_preserves_original_type_arguments_and_adds_execution_context() {
        let root = syn::parse_quote!(framework);
        let item: ItemStruct = syn::parse_quote!(
            struct Machine<T: Clone>
            where
                T: Default,
            {
                state: T,
            }
        );
        let entry = Entry {
            record: Record {
                segments: vec![Segment {
                    name: syn::parse_quote!(cpu),
                    source: vec![],
                }],
                dialect: syn::parse_quote!(arithmetic),
                leaf: syn::parse_quote!(Add),
                slot: 0,
                codes: vec![],
            },
            instruction: syn::parse_quote!(arithmetic::Add),
            field: Some(syn::parse_quote!(CpuField)),
        };
        let output: syn::File = syn::parse2(aggregate(&root, &item, &[entry]).unwrap()).unwrap();
        let implementation = output
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Impl(implementation) = item else {
                    return None;
                };
                implementation
                    .trait_
                    .as_ref()
                    .filter(|(_, path, _)| path.segments.last().unwrap().ident == "Dispatch")
                    .map(|_| implementation)
            })
            .expect("direct Dispatch implementation");
        let Type::Path(target) = implementation.self_ty.as_ref() else {
            panic!("named target")
        };
        assert_eq!(target.path.segments.len(), 1);
        assert_eq!(target.path.segments[0].ident, "Machine");
        let syn::PathArguments::AngleBracketed(arguments) = &target.path.segments[0].arguments
        else {
            panic!("original type arguments")
        };
        assert_eq!(arguments.args.len(), 1);
        assert!(
            matches!(&arguments.args[0], syn::GenericArgument::Type(Type::Path(ty)) if ty.path.is_ident("T"))
        );
        assert_eq!(implementation.generics.params.len(), 2);
        let syn::GenericParam::Type(parameter) = &implementation.generics.params[0] else {
            panic!("original parameter")
        };
        assert_eq!(parameter.ident, "T");
        assert_eq!(parameter.bounds.len(), 1);
        assert!(
            matches!(&parameter.bounds[0], syn::TypeParamBound::Trait(bound) if bound.path.is_ident("Clone"))
        );
        let predicates = &implementation
            .generics
            .where_clause
            .as_ref()
            .unwrap()
            .predicates;
        assert_eq!(predicates.len(), 3);
        let syn::WherePredicate::Type(predicate) = &predicates[0] else {
            panic!("original where predicate")
        };
        assert!(matches!(&predicate.bounded_ty, Type::Path(ty) if ty.path.is_ident("T")));
        assert_eq!(predicate.bounds.len(), 1);
        assert!(
            matches!(&predicate.bounds[0], syn::TypeParamBound::Trait(bound) if bound.path.is_ident("Default"))
        );
    }

    #[test]
    fn readable_names_preserve_segment_boundaries_and_underscores() {
        fn name(fields: &[&str]) -> String {
            variant(&Record {
                segments: fields
                    .iter()
                    .map(|field| Segment {
                        name: syn::parse_str(field).unwrap(),
                        source: vec![],
                    })
                    .collect(),
                dialect: syn::parse_quote!(arithmetic),
                leaf: syn::parse_quote!(Add),
                slot: 0,
                codes: vec![],
            })
            .to_string()
        }
        assert_eq!(name(&["cpu"]), "V_cpu__arithmetic__Add");
        assert_eq!(name(&["cpu_2"]), "V_cpu_u2__arithmetic__Add");
        assert_ne!(name(&["a", "bc"]), name(&["ab", "c"]));
        assert_ne!(name(&["a__b"]), name(&["a", "b"]));
        assert_eq!(name(&["r#type", "Δ"]), "V_type__Δ__arithmetic__Add");
    }
}

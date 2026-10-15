//! Deliberately bounded Step 1 expansion probes, never exported by vihaco.
#![cfg(feature = "typed-declarations")]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Item, ItemEnum, ItemMod, Type, ext::IdentExt, parse::Parse, parse::ParseStream};

fn root() -> syn::Path {
    match proc_macro_crate::crate_name("proof-support").expect("probe support dependency") {
        proc_macro_crate::FoundCrate::Itself => syn::parse_quote!(crate),
        proc_macro_crate::FoundCrate::Name(name) => {
            let ident = format_ident!("{name}");
            syn::parse_quote!(::#ident)
        }
    }
}

#[proc_macro_attribute]
pub fn dialect(args: TokenStream, item: TokenStream) -> TokenStream {
    let module = syn::parse_macro_input!(item as ItemMod);
    expand_dialect(args.into(), module)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
fn expand_dialect(
    args: proc_macro2::TokenStream,
    mut module: ItemMod,
) -> syn::Result<proc_macro2::TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "dialect takes no arguments"));
    }
    let root = root();
    let head = module.ident.unraw().to_string();
    let (_, items) = module.content.as_mut().ok_or_else(|| {
        syn::Error::new_spanned(&module.ident, "dialect requires an inline module")
    })?;
    let occupied: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(s) => Some(s.ident.to_string()),
            Item::Mod(m) => Some(m.ident.to_string()),
            Item::Type(t) => Some(t.ident.to_string()),
            _ => None,
        })
        .collect();
    if occupied.iter().any(|name| name == "__VihacoDialect") {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "reserved dialect identity name",
        ));
    }
    let mut generated = Vec::new();
    let mut leaves = Vec::new();
    let mut records = Vec::new();
    for item in items.iter_mut() {
        if let Item::Enum(e) = item {
            if e.attrs.iter().any(|a| a.path().is_ident("instruction")) {
                return Err(syn::Error::new_spanned(
                    e,
                    "instruction requires a unit or tuple struct",
                ));
            }
        }
        let Item::Struct(s) = item else { continue };
        let markers: Vec<_> = s
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("instruction"))
            .collect();
        if markers.is_empty() {
            continue;
        }
        if markers.len() != 1 || !matches!(markers[0].meta, syn::Meta::Path(_)) {
            return Err(syn::Error::new_spanned(
                markers[0],
                "expected exactly one bare instruction marker",
            ));
        }
        if !s.generics.params.is_empty() || matches!(s.fields, syn::Fields::Named(_)) {
            return Err(syn::Error::new_spanned(
                s,
                "instruction requires a nongeneric unit or tuple struct",
            ));
        }
        if s.attrs
            .iter()
            .any(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        {
            return Err(syn::Error::new_spanned(
                s,
                "conditional instruction membership is unsupported",
            ));
        }
        if s.ident.to_string().starts_with("r#") {
            return Err(syn::Error::new_spanned(
                &s.ident,
                "raw instruction names are initially unsupported",
            ));
        }
        if !matches!(s.vis, syn::Visibility::Public(_)) {
            return Err(syn::Error::new_spanned(
                &*s,
                "instruction requires unrestricted pub visibility",
            ));
        }
        let name = s.ident.clone();
        let surface = format_ident!("{}Surface", name);
        if occupied.iter().any(|n| n == &surface.to_string()) {
            return Err(syn::Error::new_spanned(
                &name,
                "generated surface name collides with a module item",
            ));
        }
        let mut surface_struct = s.clone();
        surface_struct.ident = surface.clone();
        surface_struct
            .attrs
            .retain(|a| a.path().is_ident("doc") || a.path().is_ident("pattern"));
        if !surface_struct
            .attrs
            .iter()
            .any(|a| a.path().is_ident("pattern"))
        {
            let mnemonic = name.to_string().to_lowercase();
            let fields = (0..s.fields.len())
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(" `,` ");
            let pattern = if fields.is_empty() {
                format!("'{mnemonic}")
            } else {
                format!("'{mnemonic} {fields}")
            };
            surface_struct
                .attrs
                .push(syn::parse_quote!(#[pattern = #pattern]));
        }
        // Only inspect a well-formed first token; binding validation stays with Parse.
        let pattern = surface_struct
            .attrs
            .iter()
            .find(|a| a.path().is_ident("pattern"))
            .unwrap();
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(pattern),
            ..
        }) = &pattern.meta.require_name_value()?.value
        else {
            return Err(syn::Error::new_spanned(
                pattern,
                "expected a literal pattern",
            ));
        };
        let mnemonic = pattern
            .value()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .strip_prefix('\'')
            .unwrap_or("")
            .to_owned();
        s.attrs
            .retain(|a| !a.path().is_ident("instruction") && !a.path().is_ident("pattern"));
        generated.push(quote! {
            #[derive(Clone, Debug, PartialEq, #root::Parse)]
            #[syntax_class(instruction, head = #head)]
            #surface_struct
            impl #root::InstructionSpec for #name {
                type Surface = #surface;
                type Dialect = __VihacoDialect;
                const MNEMONIC: &'static str = #mnemonic;
            }
        });
        records.push(quote!(#root::Leaf { head: #head, name: #mnemonic }));
        leaves.push(name);
    }
    if leaves.is_empty() {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "dialect needs a marked instruction",
        ));
    }
    generated.push(quote! {
        #[doc(hidden)] pub struct __VihacoDialect;
        impl #root::DialectSpec for __VihacoDialect {
            const LEAVES: &'static [#root::Leaf] = &[#(#records),*];
        }
        impl<C> #root::Coverage<C> for __VihacoDialect
        where C: #root::Component #( + #root::Execute<#leaves> )* {}
    });
    for tokens in generated {
        items.extend(syn::parse2::<syn::File>(tokens)?.items);
    }
    Ok(quote!(#module))
}

struct SetArgs {
    machine: Type,
}
impl Parse for SetArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: syn::Ident = input.parse()?;
        if key != "composite" {
            return Err(syn::Error::new_spanned(key, "expected composite = Type"));
        }
        input.parse::<syn::Token![=]>()?;
        let machine = input.parse()?;
        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("unexpected instruction_set option"));
        }
        Ok(Self { machine })
    }
}

#[proc_macro_attribute]
pub fn instruction_set(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(args as SetArgs);
    let item = syn::parse_macro_input!(item as ItemEnum);
    expand_set(args, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
fn expand_set(args: SetArgs, mut item: ItemEnum) -> syn::Result<proc_macro2::TokenStream> {
    let root = root();
    let machine = args.machine;
    let name = &item.ident;
    let vis = &item.vis;
    let unraw = name.unraw().to_string();
    let surface = format_ident!(
        "{}SurfaceInstruction",
        unraw.strip_suffix("Instruction").unwrap_or(&unraw)
    );
    let mut checks = Vec::new();
    let mut pairs = Vec::new();
    let mut variants = Vec::new();
    for variant in &mut item.variants {
        let helpers: Vec<_> = variant
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("target"))
            .collect();
        if helpers.len() != 1 {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "expected one target",
            ));
        }
        let path: syn::Path = helpers[0].parse_args()?;
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                &variant.fields,
                "expected one instruction payload",
            ));
        };
        if fields.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(fields, "expected one payload"));
        }
        let ty = &fields.unnamed[0].ty;
        let mut target = quote!(#machine);
        let mut keys = Vec::new();
        for segment in &path.segments {
            let constant = format_ident!(
                "__vihaco_field_{}",
                segment.ident.unraw(),
                span = segment.ident.span(),
            );
            let index = quote!(<#target>::#constant);
            target = quote!(<#target as #root::Field<{ #index }>>::Child);
            // A tuple of standard unit-array types records the exact index path
            // for coherence checks without another runtime marker contract.
            keys.push(quote!([(); #index]));
        }
        let route = quote!((#(#keys,)*));
        checks.push(quote!(#root::require_available::<#target, #ty>();));
        pairs.push(quote!(impl SelectedPair<#route, #ty> for #machine {}));
        let ident = &variant.ident;
        variants.push(quote!(#ident(<#ty as #root::InstructionSpec>::Surface)));
        variant.attrs.retain(|a| !a.path().is_ident("target"));
    }
    Ok(quote! {
        #item
        #[derive(Clone, Debug, PartialEq)]
        #vis enum #surface { #(#variants),* }
        impl #root::HasInstructionSet for #machine {
            type Runtime = #name;
            type Syntax = #surface;
        }
        const _: () = {
            #root::validate(<#machine as #root::Capabilities>::DESCRIPTION);
            fn check() { #(#checks)* }
            trait SelectedPair<Route, Instruction> {}
            #(#pairs)*
        };
    })
}

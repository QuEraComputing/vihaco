// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Nongeneric dialect validation and phase-aware instruction parsing.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Fields, Item, ItemMod, ItemStruct, Meta, Path, Result, Token, Visibility,
    ext::IdentExt,
    parse_quote,
    punctuated::Punctuated,
    visit::{self, Visit},
};

use crate::common::{resolve_root, strip_attrs};

struct Dialect {
    module: ItemMod,
    parent_root: Path,
    root: Path,
    leaves: Vec<ItemStruct>,
}

/// Validate an inline dialect declaration and generate instruction syntax parsers.
pub(crate) fn expand(options: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !options.is_empty() {
        return Err(syn::Error::new_spanned(
            options,
            "#[dialect] takes no arguments",
        ));
    }
    let module = syn::parse2(item)?;
    let declaration = validate(module)?;
    emit(declaration)
}

/// Resolve the framework root and collect valid leaves after checking reserved names.
fn validate(mut module: ItemMod) -> Result<Dialect> {
    let parent_root = resolve_root(&module.attrs)?;
    let root = resolve_dialect_root(parent_root.clone());
    strip_attrs(&mut module.attrs, &["vihaco"]);
    reject_helpers(&module.attrs)?;
    let (_, items) = module.content.as_mut().ok_or_else(|| {
        syn::Error::new(module.ident.span(), "#[dialect] requires an inline module")
    })?;
    let mut names = HashSet::new();
    for item in items.iter() {
        collect_names(item, &mut names);
    }
    if names.contains("__VihacoDialect") {
        return Err(syn::Error::new(
            module.ident.span(),
            "reserved dialect identity name `__VihacoDialect`",
        ));
    }
    if names.contains("__vihaco_inventory") {
        return Err(syn::Error::new(
            module.ident.span(),
            "generated inventory module collides with an authored item",
        ));
    }
    let leaves = collect_instruction_leaves(items)?;
    if leaves.is_empty() {
        return Err(syn::Error::new(
            module.ident.span(),
            "#[dialect] requires at least one #[instruction] struct",
        ));
    }
    Ok(Dialect {
        module,
        parent_root,
        root,
        leaves,
    })
}

/// Adjust the parent-authored framework path for references inside the dialect module.
fn resolve_dialect_root(mut root: Path) -> Path {
    // The root helper is authored on the module in its parent scope. All
    // generated references live one module deeper; operand paths stay untouched.
    if root.leading_colon.is_none() {
        if root.segments[0].ident == "self" {
            root.segments[0].ident = syn::Ident::new("super", root.segments[0].ident.span());
        } else if root.segments[0].ident != "crate" {
            root.segments.insert(
                0,
                syn::PathSegment {
                    ident: syn::Ident::new("super", proc_macro2::Span::call_site()),
                    arguments: syn::PathArguments::None,
                },
            );
        }
    }
    root
}

/// Collect immediate marked instruction structs and reject misplaced helpers elsewhere.
fn collect_instruction_leaves(items: &mut [Item]) -> Result<Vec<ItemStruct>> {
    let mut leaves = Vec::new();
    let mut leaf_names = HashSet::new();
    for item in items {
        if let Item::Struct(leaf) = item
            && has_instruction_marker(&leaf.attrs)?
        {
            leaves.push(validate_instruction(leaf, &mut leaf_names)?);
        } else {
            check_item(item, HelperValidationMode::UnmarkedItem)?;
        }
    }
    Ok(leaves)
}

/// Recognize one argument-free instruction marker.
fn has_instruction_marker(attrs: &[Attribute]) -> Result<bool> {
    let mut markers = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("instruction"));
    let Some(marker) = markers.next() else {
        return Ok(false);
    };
    if let Some(repeated) = markers.next() {
        return Err(syn::Error::new_spanned(
            repeated,
            "repeated #[instruction] marker",
        ));
    }
    if !matches!(marker.meta, Meta::Path(_)) {
        return Err(syn::Error::new_spanned(
            marker,
            "#[instruction] takes no arguments",
        ));
    }
    Ok(true)
}

/// Validate a runtime leaf and prepare its parser metadata.
fn validate_instruction(
    leaf: &mut ItemStruct,
    leaf_names: &mut HashSet<String>,
) -> Result<ItemStruct> {
    validate_instruction_shape(leaf)?;
    let name = leaf.ident.to_string();
    if name.starts_with("r#") {
        return Err(syn::Error::new(
            leaf.ident.span(),
            "raw instruction names are unsupported",
        ));
    }
    if !leaf_names.insert(name.clone()) {
        return Err(syn::Error::new(
            leaf.ident.span(),
            "duplicate instruction name",
        ));
    }
    validate_instruction_attrs(&leaf.attrs)?;
    strip_attrs(&mut leaf.attrs, &["instruction"]);
    let mut checked = leaf.clone();
    strip_attrs(&mut checked.attrs, &["pattern"]);
    check_item(&Item::Struct(checked), HelperValidationMode::Instruction)?;
    add_derives(leaf)?;
    let instruction = leaf.clone();
    strip_attrs(&mut leaf.attrs, &["pattern"]);
    Ok(instruction)
}

/// Require public, nongeneric unit or tuple structs for instruction leaves.
fn validate_instruction_shape(leaf: &ItemStruct) -> Result<()> {
    if !matches!(leaf.vis, Visibility::Public(_)) {
        return Err(syn::Error::new(
            leaf.ident.span(),
            "instruction structs require unrestricted pub visibility",
        ));
    }
    if !leaf.generics.params.is_empty() || leaf.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &leaf.generics,
            "generic instruction structs are unsupported",
        ));
    }
    if matches!(leaf.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &leaf.fields,
            "instructions require unit or tuple structs",
        ));
    }
    Ok(())
}

/// Check parser helper placement and reject conditional instruction membership.
fn validate_instruction_attrs(attrs: &[Attribute]) -> Result<()> {
    let mut pattern_seen = false;
    for attr in attrs {
        if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
            return Err(syn::Error::new_spanned(
                attr,
                "conditional instruction membership is unsupported; gate the whole dialect",
            ));
        }
        if attr.path().is_ident("pattern") {
            if pattern_seen {
                return Err(syn::Error::new_spanned(
                    attr,
                    "repeated #[pattern] attribute",
                ));
            }
            pattern_seen = true;
        } else if attr.path().is_ident("syntax_class") || attr.path().is_ident("vihaco") {
            return Err(syn::Error::new_spanned(
                attr,
                "unsupported instruction helper placement",
            ));
        }
    }
    Ok(())
}

/// Add the aggregate-required Clone, Debug, and PartialEq derives when absent.
fn add_derives(leaf: &mut ItemStruct) -> Result<()> {
    let mut derives = HashSet::new();
    for attr in &leaf.attrs {
        if attr.path().is_ident("derive") {
            for path in attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)? {
                if let Some(segment) = path.segments.last() {
                    derives.insert(segment.ident.to_string());
                }
            }
        }
    }
    let missing: Vec<Path> = ["Clone", "Debug", "PartialEq"]
        .into_iter()
        .filter(|name| !derives.contains(*name))
        .map(syn::parse_str)
        .collect::<Result<_>>()?;
    if !missing.is_empty() {
        leaf.attrs.push(parse_quote!(#[derive(#(#missing),*)]));
    }
    Ok(())
}

/// Reject instruction and parser helpers where the dialect cannot consume them.
fn reject_helpers(attrs: &[Attribute]) -> Result<()> {
    for attr in attrs {
        if ["instruction", "pattern", "syntax_class"]
            .iter()
            .any(|name| attr.path().is_ident(name))
        {
            return Err(syn::Error::new_spanned(
                attr,
                "instruction helpers are only supported on immediate marked structs",
            ));
        }
    }
    Ok(())
}

enum HelperValidationMode {
    Instruction,
    UnmarkedItem,
}

/// Walk an item for misplaced or conditional helpers according to its instruction status.
fn check_item(item: &Item, mode: HelperValidationMode) -> Result<()> {
    struct HelperValidator {
        error: Option<syn::Error>,
        mode: HelperValidationMode,
    }

    impl<'ast> Visit<'ast> for HelperValidator {
        fn visit_attribute(&mut self, attr: &'ast Attribute) {
            if self.error.is_some() {
                return;
            }
            let owned = |path: &Path| {
                path.is_ident("instruction")
                    || (matches!(self.mode, HelperValidationMode::Instruction)
                        && (path.is_ident("pattern") || path.is_ident("syntax_class")))
            };
            if owned(attr.path()) {
                self.error = Some(syn::Error::new_spanned(
                    attr,
                    "instruction helpers are only supported on immediate marked structs",
                ));
            } else if attr.path().is_ident("cfg_attr") {
                // Conditional markers cannot be consumed. Unmarked parser
                // helpers belong to their own derive and remain untouched.
                if let Ok(args) =
                    attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                    && args.iter().skip(1).any(|meta| owned(meta.path()))
                {
                    self.error = Some(syn::Error::new_spanned(
                        attr,
                        "conditional instruction helpers are unsupported",
                    ));
                }
            }
            visit::visit_attribute(self, attr);
        }
    }
    let mut visitor = HelperValidator { error: None, mode };
    visitor.visit_item(item);
    match visitor.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Collect authored names and explicit import bindings to detect generated-name collisions.
fn collect_names(item: &Item, names: &mut HashSet<String>) {
    let ident = match item {
        Item::Struct(i) => Some(&i.ident),
        Item::Enum(i) => Some(&i.ident),
        Item::Union(i) => Some(&i.ident),
        Item::Type(i) => Some(&i.ident),
        Item::Mod(i) => Some(&i.ident),
        Item::Trait(i) => Some(&i.ident),
        Item::Const(i) => Some(&i.ident),
        Item::Static(i) => Some(&i.ident),
        Item::Fn(i) => Some(&i.sig.ident),
        Item::ExternCrate(i) => Some(i.rename.as_ref().map_or(&i.ident, |(_, name)| name)),
        _ => None,
    };
    if let Some(ident) = ident {
        names.insert(ident.unraw().to_string());
    }

    fn imports(tree: &syn::UseTree, names: &mut HashSet<String>) {
        match tree {
            syn::UseTree::Name(i) => {
                names.insert(i.ident.unraw().to_string());
            }
            syn::UseTree::Rename(i) => {
                names.insert(i.rename.unraw().to_string());
            }
            syn::UseTree::Path(i) => imports(&i.tree, names),
            syn::UseTree::Group(i) => {
                for item in &i.items {
                    imports(item, names);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
    if let Item::Use(item) = item {
        imports(&item.tree, names);
    }
}

/// Emit runtime parsers, dialect identity, coverage, and the leaf inventory producer.
fn emit(dialect: Dialect) -> Result<TokenStream> {
    let Dialect {
        mut module,
        parent_root,
        root,
        leaves,
    } = dialect;
    let head = module.ident.unraw().to_string();
    let mut instructions = Vec::new();
    let mut runtime_names = Vec::new();
    for leaf in &leaves {
        let runtime = &leaf.ident;
        runtime_names.push(runtime);
        let mut instruction = leaf.clone();
        instruction
            .attrs
            .push(parse_quote!(#[syntax_class(instruction)]));
        if !instruction
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("pattern"))
        {
            let mut pattern = format!("'{}", runtime.to_string().to_lowercase());
            if !leaf.fields.is_empty() {
                pattern.push(' ');
                pattern.push_str(
                    &(0..leaf.fields.len())
                        .map(|i| format!("${i}"))
                        .collect::<Vec<_>>()
                        .join(" `,` "),
                );
            }
            let pattern = syn::LitStr::new(&pattern, runtime.span());
            instruction.attrs.push(parse_quote!(#[pattern = #pattern]));
        }
        instruction
            .attrs
            .push(parse_quote!(#[__vihaco_runtime(#runtime)]));
        instructions.push(quote!(#instruction));
    }
    // A coverage parameter must not shadow an authored instruction type.
    let mut component = format_ident!("__VihacoComponent");
    let mut suffix = 0usize;
    while runtime_names.iter().any(|name| **name == component) {
        component = format_ident!("__VihacoComponent{suffix}");
        suffix += 1;
    }
    let slots = runtime_names.iter().enumerate().map(|(slot, runtime)| quote! {
        impl #root::__private::LeafAt<#slot> for __VihacoDialect { type Instruction = #runtime; }
    });
    let dialect_name = &module.ident;
    let records = runtime_names
        .iter()
        .enumerate()
        .map(|(slot, leaf)| quote!(([] #dialect_name #leaf #slot [])))
        .collect();
    let producer = crate::inventory::producer(&format_ident!("__vihaco_inventory"), records, false);
    let declarations = quote! {
        #producer
        #(#slots)*
        #[doc(hidden)]
        pub struct __VihacoDialect;

        impl #root::__private::DialectSpec for __VihacoDialect {
            const LEAVES: &'static [#root::__private::Leaf] = &[
                #(#root::__private::Leaf {
                    dialect: #head,
                    mnemonic: <#runtime_names as #root::__private::InstructionSpec>::MNEMONIC,
                }),*
            ];
        }

        impl<#component: #root::Component> #root::__private::Coverage<#component> for __VihacoDialect
        where #(#component: #root::Execute<#runtime_names>),* {}
    };
    // Ordinary items remain visible to expansion formatters. The internal
    // module attribute consumes parser markers and emits their parsers without
    // hiding the declarations inside an opaque function-like macro body.
    module.attrs.push(parse_quote!(
        #[#parent_root::__private::__dialect_parsers(#root)]
    ));
    let mut instructions = instructions.into_iter();
    for item in &mut module.content.as_mut().expect("validated inline module").1 {
        if let Item::Struct(leaf) = item
            && runtime_names.iter().any(|name| **name == leaf.ident)
        {
            *item = syn::parse2(instructions.next().expect("one parser per runtime leaf"))?;
        }
    }
    let generated = quote!(#declarations);
    // Append ordinary items to the authored module, retaining item order/scope.
    let generated: syn::File = syn::parse2(generated)?;
    module
        .content
        .as_mut()
        .expect("validated inline module")
        .1
        .extend(generated.items);
    Ok(quote!(#module))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_exposes_declarations_to_rust_formatters() {
        let expanded = expand(
            TokenStream::new(),
            quote! {
                #[vihaco(crate = crate)]
                mod control {
                    #[instruction]
                    pub struct Halt;
                }
            },
        )
        .unwrap();
        let file: syn::File = syn::parse2(expanded).unwrap();
        let formatted = prettyplease::unparse(&file);
        assert!(
            formatted.contains("#[__vihaco_runtime(Halt)]"),
            "{formatted}"
        );
        assert!(formatted.contains("pub struct Halt;"), "{formatted}");
        assert!(
            formatted.contains("\n    pub struct __VihacoDialect;\n"),
            "{formatted}"
        );
        assert!(
            formatted.contains("\n    impl crate::__private::DialectSpec for __VihacoDialect {\n"),
            "{formatted}"
        );
        assert!(!formatted.contains("__dialect_parsers!"), "{formatted}");
    }
}

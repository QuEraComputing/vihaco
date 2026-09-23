// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use syn::{Expr, ExprLit, Lit, Result};

use super::parse::{Declaration, Instruction};

pub(super) fn validate(declaration: &Declaration) -> Result<()> {
    if declaration.instructions.is_empty() {
        return Err(syn::Error::new_spanned(
            &declaration.name,
            "dialect instruction list must not be empty",
        ));
    }

    for attr in &declaration.attrs {
        if !attr.path().is_ident("doc") && !attr.path().is_ident("vihaco") {
            return Err(syn::Error::new_spanned(
                attr,
                "unsupported dialect attribute; expected documentation or `#[vihaco(crate = ...)]`",
            ));
        }
    }

    let mut names = BTreeSet::new();
    let mut mnemonics = BTreeSet::new();
    for instruction in &declaration.instructions {
        if !names.insert(instruction.name.to_string()) {
            return Err(syn::Error::new_spanned(
                &instruction.name,
                "duplicate dialect instruction",
            ));
        }

        for attr in &instruction.attrs {
            if !attr.path().is_ident("pattern") && !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "unsupported dialect instruction attribute; expected `#[pattern = ...]`",
                ));
            }
        }

        let Some(mnemonic) = instruction_mnemonic(instruction) else {
            continue;
        };
        if !mnemonics.insert(mnemonic.clone()) {
            let message = format!("duplicate instruction name `{mnemonic}`");
            let error = instruction
                .attrs
                .iter()
                .rev()
                .find(|attr| attr.path().is_ident("pattern"))
                .map_or_else(
                    || syn::Error::new_spanned(&instruction.name, &message),
                    |attr| syn::Error::new_spanned(attr, &message),
                );
            return Err(error);
        }
    }

    Ok(())
}

pub(super) fn instruction_mnemonic(instruction: &Instruction) -> Option<String> {
    let pattern = instruction
        .attrs
        .iter()
        .rev()
        .find(|attr| attr.path().is_ident("pattern"));

    let Some(pattern) = pattern else {
        return Some(instruction.name.to_string().to_lowercase());
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(pattern),
        ..
    }) = &pattern.meta.require_name_value().ok()?.value
    else {
        return None;
    };

    let pattern = pattern.value();
    if pattern.split(' ').any(str::is_empty) {
        return None;
    }

    pattern
        .split(' ')
        .next()
        .and_then(|atom| atom.strip_prefix('\''))
        .filter(|mnemonic| !mnemonic.is_empty())
        .map(str::to_owned)
}

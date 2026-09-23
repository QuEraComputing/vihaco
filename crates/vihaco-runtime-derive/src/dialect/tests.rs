// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use quote::ToTokens;

use super::parse::Declaration;
use super::validation::validate;

#[test]
fn parses_syntax_and_runtime_payload_mappings() {
    let declaration: Declaration = syn::parse_quote! {
        arith {
            Add(u32 => i64, bool),
            Halt,
        }
    };

    assert_eq!(declaration.name, "arith");
    assert_eq!(declaration.instructions.len(), 2);
    let fields = &declaration.instructions[0].fields;
    assert_eq!(fields[0].syntax.to_token_stream().to_string(), "u32");
    assert_eq!(fields[0].runtime.to_token_stream().to_string(), "i64");
    assert_eq!(fields[1].syntax.to_token_stream().to_string(), "bool");
    assert_eq!(fields[1].runtime.to_token_stream().to_string(), "bool");
}

#[test]
fn rejects_duplicate_instruction_names() {
    let declaration: Declaration = syn::parse_quote! {
        arith { Add, Add }
    };

    let error = validate(&declaration).expect_err("duplicate instructions must be rejected");
    assert_eq!(error.to_string(), "duplicate dialect instruction");
}

#[test]
fn rejects_duplicate_instruction_mnemonics() {
    let declaration: Declaration = syn::parse_quote! {
        arith {
            Add,
            #[pattern = "'add $0"]
            AddImmediate(u32),
        }
    };

    let error = validate(&declaration).expect_err("duplicate mnemonics must be rejected");
    assert_eq!(error.to_string(), "duplicate instruction name `add`");
}

#[test]
fn leaves_malformed_patterns_to_parser_derive() {
    let declaration: Declaration = syn::parse_quote! {
        arith {
            Add,
            #[pattern = "'add  $0"]
            AddImmediate(u32),
        }
    };

    assert!(validate(&declaration).is_ok());
}

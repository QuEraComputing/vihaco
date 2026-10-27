// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use crate::ParsedModule;
use vihaco_abi::Type;
use vihaco_bytecode::{NoContext, SstFile};

// No Clone implementation: parsing and iteration must not require one.
#[derive(Debug, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "device")]
enum Declaration {
    #[pattern = "gain $0"]
    Gain(u64),
    #[pattern = "name $0"]
    Name(String),
}

#[test]
fn declarations_preserve_order_and_support_comments_and_owned_iteration() -> eyre::Result<()> {
    let headers = HeaderDeclarations::<Declaration>::from_text(
        "// configuration\ndevice gain 2; device name \"controller\"; // end\n",
    )?;
    assert_eq!(
        headers.into_iter().collect::<Vec<_>>(),
        [Declaration::Gain(2), Declaration::Name("controller".into())]
    );
    Ok(())
}

#[test]
fn empty_headers_are_valid_collections() -> eyre::Result<()> {
    assert!(
        HeaderDeclarations::<Declaration>::from_text(" // no declarations\n")?
            .0
            .is_empty()
    );
    Ok(())
}

#[test]
fn invalid_declarations_and_missing_terminators_are_rejected() {
    for source in [
        "device gain 2",
        "device unknown 2;",
        "device gain invalid;",
        "device gain 2; trailing",
    ] {
        assert!(HeaderDeclarations::<Declaration>::from_text(source).is_err());
    }
}

#[test]
fn collections_are_parsed_through_the_section_header_interface() -> eyre::Result<()> {
    #[derive(vihaco_parser_derive::Parse)]
    #[syntax_class(instruction, head = "example")]
    enum Instruction {
        Halt,
    }

    let file = SstFile::<NoContext>::from_text(
        "sst v1\n.section(root):\n.header(root):\ndevice gain 2;\n.header(root).\n\
         .text(root):\nfn @main() { example.halt }\n.text(root).\n.section(root).\n",
    )?;
    let parsed = ParsedModule::<Instruction, Type, HeaderDeclarations<Declaration>>::parse_section(
        file.root(),
    )?;
    assert_eq!(parsed.header.0, [Declaration::Gain(2)]);
    assert_eq!(parsed.functions[0].body.len(), 1);
    Ok(())
}

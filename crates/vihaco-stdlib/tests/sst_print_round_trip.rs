// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Section-level round trip of `examples/loadable.sst`: every header
//! declaration and instruction prints, reparses, and compares equal.

#![cfg(feature = "program-image-execution")]

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_module::module::ProgramMetadata;
use vihaco_parser::{Parse, Print};
use vihaco_runtime::{HasInstructionSet, chumsky::Parser as _, composite};
use vihaco_syntax::{HeaderDeclarations, ParsedModule};

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

type SourceInstruction = <Machine as HasInstructionSet>::Syntax;

/// The header vocabulary of `examples/loadable.rs`.
#[derive(Debug, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum HeaderDeclaration {
    #[pattern = "$0"]
    Metadata(ProgramMetadata<Value, Type>),
    #[pattern = "experiment.description $0"]
    Description(String),
}

fn reparse<T>(value: &T) -> String
where
    T: Print + PartialEq + std::fmt::Debug + for<'src> Parse<'src>,
{
    let printed = value.to_source().unwrap();
    let parsed = T::parser().parse(printed.as_str()).into_result();
    assert_eq!(parsed.as_ref(), Ok(value), "reparsing {printed:?}");
    printed
}

/// Instruction lines of the `.text` block, without symbol definitions.
fn instruction_lines(source: &str) -> Vec<&str> {
    let text = source.split(".text").nth(1).expect(".text block");
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('@'))
        .collect()
}

#[test]
fn loadable_example_header_and_code_round_trip() {
    let source = include_str!("../examples/loadable.sst");
    let file = SSTFile::<NoContext>::from_text(source).unwrap();
    let section = file.section("controller").unwrap();
    let parsed =
        ParsedModule::<SourceInstruction, HeaderDeclarations<HeaderDeclaration>>::parse_section(
            section,
        )
        .unwrap();

    let printed: Vec<_> = parsed.header.0.iter().map(reparse).collect();
    assert_eq!(
        printed,
        [
            r#"experiment.description "Loadable SST example: 21 * 2""#,
            concat!(
                "metadata { functions: [",
                "{ name: 1, signature: { params: [], ret: [u64] }, local_count: 0, file: 2 }, ",
                "{ name: 0, signature: { params: [], ret: [u64] }, local_count: 0, file: 2 }",
                r#"], constants: [], strings: ["main", "helper", "loadable.sst"], "#,
                "main_function: 1, source_symbols: [], extra: {} }",
            ),
        ]
    );

    // The example is written canonically, so each line prints back verbatim.
    let code: Vec<_> = parsed.code.iter().map(reparse).collect();
    assert_eq!(code, instruction_lines(source));
}

#[test]
fn metadata_constants_and_symbols_round_trip() {
    let source = concat!(
        "metadata {\n",
        "    strings: [\"a\\tb\"], functions: [],\n",
        "    constants: [u64 18446744073709551615, i64 -1, f64 -0.5, f64_bits 9221120237041090561,\n",
        "                f32 1.5, str 0, fn 2, heap 3, bool false, undefined],\n",
        "    main_function: null,\n",
        "    source_symbols: [{ index: 4, name: \"loop\" }],\n",
        "    extra: {},\n",
        "}",
    );
    let parsed = HeaderDeclaration::parser()
        .parse(source)
        .into_result()
        .unwrap();
    let printed = parsed.to_source().unwrap();
    assert!(
        printed.contains("f64_bits 9221120237041090561"),
        "{printed}"
    );
    assert!(printed.contains("main_function: null"), "{printed}");
    // NaN compares unequal, so compare the canonical text after a reparse.
    let reparsed = HeaderDeclaration::parser()
        .parse(printed.as_str())
        .into_result()
        .unwrap();
    assert_eq!(reparsed.to_source().unwrap(), printed);
}

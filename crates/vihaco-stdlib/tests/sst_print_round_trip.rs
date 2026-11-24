// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Canonical SST printing and parse round trips for headers and instructions.

use std::collections::BTreeSet;

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_module::module::ProgramDirective;
use vihaco_parser::{Parse, Print};
use vihaco_runtime::{__private::capabilities, HasInstructionSet, chumsky::Parser as _, composite};
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
    Standard(ProgramDirective<Value, Type>),
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

    // The example is written canonically, so each directive prints back as
    // its line without the comment.
    let printed: Vec<_> = parsed.header.0.iter().map(reparse).collect();
    assert_eq!(
        printed,
        [
            r#"experiment.description "Loadable SST example: 21 * 2""#,
            r#".file "loadable.sst""#,
            ".func @helper() -> u64, locals 0",
            ".func @main() -> u64, locals 0",
            ".entry @main",
        ]
    );

    // The example is written canonically, so each line prints back verbatim.
    let code: Vec<_> = parsed.code.iter().map(reparse).collect();
    assert_eq!(code, instruction_lines(source));
}

#[test]
fn constant_and_symbol_directives_round_trip() {
    for (source, canonical) in [
        (
            ".const u64 18446744073709551615",
            ".const u64 18446744073709551615",
        ),
        (".const i64 -1", ".const i64 -1"),
        (".const f64 -0.5", ".const f64 -0.5"),
        (
            ".const f64_bits 9221120237041090561",
            ".const f64_bits 9221120237041090561",
        ),
        (".const f32 1.5", ".const f32 1.5"),
        (".const str \"a\\tb\"", ".const str \"a\\tb\""),
        (".const fn @main", ".const fn @main"),
        (".const heap 3", ".const heap 3"),
        (".const bool false", ".const bool false"),
        (".const undefined", ".const undefined"),
        (".source_symbol 4 \"loop\"", ".source_symbol 4 \"loop\""),
        (
            ".func @f(a: u64, b: f64) -> (u64, bool), locals 3",
            ".func @f(a: u64, b: f64) -> (u64, bool), locals 3",
        ),
    ] {
        let parsed = HeaderDeclaration::parser()
            .parse(source)
            .into_result()
            .unwrap();
        let printed = parsed.to_source().unwrap();
        assert_eq!(printed, canonical);
        // NaN compares unequal, so compare the canonical text after a reparse.
        let reparsed = HeaderDeclaration::parser()
            .parse(printed.as_str())
            .into_result()
            .unwrap();
        assert_eq!(reparsed.to_source().unwrap(), printed);
    }
}

#[composite]
struct Controller {
    #[device(7, alias = "host")]
    cpu: vihaco_cpu::CPU,
}

#[composite]
struct RoutedMachine {
    #[device(2, alias = "ctl")]
    controller: Controller,
    #[device(3, alias = "state")]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(42)]
    spare: vihaco_cpu::CPU,
}

const CPU_LEAVES: &[&str] = &[
    "heap_alloc 8",
    "get_item",
    "heap_dealloc",
    "heap_reserve",
    "heap_push",
    "heap_len",
    "add_i32",
    "add_f32",
    "add_i64",
    "add_u32",
    "add_u64",
    "add_f64",
    "sub_i32",
    "sub_i64",
    "sub_u32",
    "sub_u64",
    "sub_f32",
    "sub_f64",
    "mul_i32",
    "mul_i64",
    "mul_u32",
    "mul_u64",
    "mul_f32",
    "mul_f64",
    "div_i32",
    "div_i64",
    "div_u32",
    "div_u64",
    "div_f32",
    "div_f64",
    "rem_i32",
    "rem_i64",
    "rem_u32",
    "rem_u64",
    "rem_f32",
    "rem_f64",
    "neg_i32",
    "neg_i64",
    "neg_f32",
    "neg_f64",
    "shl_i32",
    "shl_i64",
    "shl_u32",
    "shl_u64",
    "shr_i32",
    "shr_i64",
    "shr_u32",
    "shr_u64",
    "rol_i32",
    "rol_i64",
    "rol_u32",
    "rol_u64",
    "ror_i32",
    "ror_i64",
    "ror_u32",
    "ror_u64",
    "bitand_i32",
    "bitand_i64",
    "bitand_u32",
    "bitand_u64",
    "bitor_i32",
    "bitor_i64",
    "bitor_u32",
    "bitor_u64",
    "bitxor_i32",
    "bitxor_i64",
    "bitxor_u32",
    "bitxor_u64",
    "not",
    "and",
    "or",
    "xor",
    "eq_i32",
    "eq_i64",
    "eq_u32",
    "eq_u64",
    "eq_f32",
    "eq_f64",
    "ne_i32",
    "ne_i64",
    "ne_u32",
    "ne_u64",
    "ne_f32",
    "ne_f64",
    "lt_i32",
    "lt_i64",
    "lt_u32",
    "lt_u64",
    "lt_f32",
    "lt_f64",
    "gt_i32",
    "gt_i64",
    "gt_u32",
    "gt_u64",
    "gt_f32",
    "gt_f64",
    "le_i32",
    "le_i64",
    "le_u32",
    "le_u64",
    "le_f32",
    "le_f64",
    "ge_i32",
    "ge_i64",
    "ge_u32",
    "ge_u64",
    "ge_f32",
    "ge_f64",
];

const PROGRAM_LEAVES: &[&str] = &[
    "br 0",
    "br @loop",
    "cond_br @then, 4",
    "cond_br 1, @else",
    "ret 2",
    "call_indirect",
    "call 0, @helper",
    "call 3, 17",
    "halt",
    "load_i32 0",
    "load_i64 1",
    "load_u32 2",
    "load_u64 3",
    "load_f32 4",
    "load_f64 5",
    "load_bool 6",
    "store_i32 0",
    "store_i64 1",
    "store_u32 2",
    "store_u64 3",
    "store_f32 4",
    "store_f64 5",
    "store_bool 6",
    "dup",
    "const_i32 -2147483648",
    "const_i64 -9223372036854775808",
    "const_u32 4294967295",
    "const_u64 18446744073709551615",
    "const_f32 -1.5",
    "const_f32 3.4028235e38",
    "const_f64 -1.9999999999998004e-6",
    "const_f64 0.1",
    "const_bool true",
    "const_bool false",
    "const_string 3",
    r#"const_string "tab\there \"quoted\" back\\slash\n""#,
    "const_fn_ref @main",
    "const_fn_ref 1",
    "const_heap_ref 0",
];

#[test]
fn every_instruction_on_every_route_prints_canonically_and_reparses() {
    type RoutedSource = <RoutedMachine as HasInstructionSet>::Syntax;

    let routes = [
        ("controller::cpu::cpu", "controller::cpu::cpu", CPU_LEAVES),
        ("ctl::cpu::cpu", "controller::cpu::cpu", CPU_LEAVES),
        ("ctl::host::cpu", "controller::cpu::cpu", CPU_LEAVES),
        ("controller::host::cpu", "controller::cpu::cpu", CPU_LEAVES),
        ("spare::cpu", "spare::cpu", CPU_LEAVES),
        ("execution::program", "execution::program", PROGRAM_LEAVES),
        ("state::program", "execution::program", PROGRAM_LEAVES),
    ];
    let mut tested = BTreeSet::new();
    for (route, canonical, leaves) in routes {
        for leaf in leaves {
            let source = format!("{route}::{leaf}");
            let parsed = RoutedSource::parser()
                .parse(source.as_str())
                .into_result()
                .unwrap_or_else(|errors| panic!("parse({source:?}): {errors:?}"));
            assert_eq!(reparse(&parsed), format!("{canonical}::{leaf}"), "{source}");

            let mnemonic = leaf.split_whitespace().next().unwrap();
            tested.insert(format!("{canonical}::{mnemonic}"));
        }
    }

    let inventory: BTreeSet<_> = capabilities::<RoutedMachine>()
        .map(|capability| {
            format!(
                "{}::{}::{}",
                capability.target.join("::"),
                capability.dialect,
                capability.instruction
            )
        })
        .collect();
    assert_eq!(tested, inventory);
}

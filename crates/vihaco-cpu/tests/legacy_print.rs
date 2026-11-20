// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Legacy composites print `field::component.mnemonic operands`, using the
//! field name rather than an alias, and parse back to the same value.

#![cfg(not(feature = "typed-declarations"))]

use vihaco_runtime::{HasInstructionSet, Parse, Print, chumsky::Parser as _, composite};

#[composite]
#[allow(dead_code)]
struct Machine {
    #[device(0x01, alias = "host")]
    cpu: vihaco_cpu::CPU,
}

type Syntax = <Machine as HasInstructionSet>::Syntax;

/// Canonical local spellings covering every operand shape of the legacy CPU.
const LEAVES: &[&str] = &[
    "span 0 12 3",
    "label @loop",
    "func_start",
    "func_end",
    "breakpoint",
    "br @loop",
    "cond_br @then, @else",
    "ret 2",
    "call_indirect",
    "call 0, helper",
    "halt",
    "print",
    "load_i64 1",
    "store_f64 5",
    "dup",
    "heap_alloc 8",
    "get_item",
    "heap_push",
    "heap_len",
    "const_i64 -42",
    "const_f64 -1.5e-7",
    "const_bool true",
    r#"const_string "line\nbreak \"quoted\"""#,
    "const_fn_ref @main",
    "add_i64",
    "neg_f64",
    "bitxor_u32",
    "not",
    "ge_f64",
];

fn parse(source: &str) -> Syntax {
    Syntax::parser()
        .parse(source)
        .into_result()
        .unwrap_or_else(|errors| panic!("parse({source:?}): {errors:?}"))
}

#[test]
fn legacy_composite_instructions_print_canonically_and_reparse() {
    for leaf in LEAVES {
        for route in ["cpu::cpu.", "host::cpu."] {
            let parsed = parse(&format!("{route}{leaf}"));
            let printed = parsed.to_source().unwrap();
            assert_eq!(printed, format!("cpu::cpu.{leaf}"));
            assert_eq!(parse(&printed), parsed, "reparsing {printed:?}");
        }
    }
}

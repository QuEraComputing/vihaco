// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Every leaf of a nested, aliased composite prints as
//! `device::dialect::mnemonic operands` and parses back to the same value.

#![cfg(all(
    feature = "typed-declarations",
    not(feature = "program-image-execution")
))]

use std::collections::BTreeSet;

use vihaco_runtime::{HasInstructionSet, Parse, Print, chumsky::Parser as _, composite};

pub use vihaco_cpu::{__vihaco_fields_CPU, __vihaco_inventory_CPU};

#[composite]
struct Controller {
    #[device(7, alias = "host")]
    cpu: vihaco_cpu::CPU,
}

#[composite]
struct Machine {
    #[device(2, alias = "ctl")]
    controller: Controller,
    #[device(42)]
    spare: vihaco_cpu::CPU,
}

type Syntax = <Machine as HasInstructionSet>::Syntax;

/// Local spellings of every CPU leaf, already in canonical form, with operands
/// exercising symbols, negative numbers, floats, and string escapes.
const LEAVES: &[&str] = &[
    "span 0 12 3",
    "breakpoint",
    "br 0",
    "br @loop",
    "cond_br @then, 4",
    "cond_br 1, @else",
    "ret 2",
    "call_indirect",
    "call 0, @helper",
    "call 3, 17",
    "halt",
    "print",
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
    "heap_alloc 8",
    "get_item",
    "heap_dealloc",
    "heap_reserve",
    "heap_push",
    "heap_len",
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

fn parse(source: &str) -> Syntax {
    Syntax::parser()
        .parse(source)
        .into_result()
        .unwrap_or_else(|errors| panic!("parse({source:?}): {errors:?}"))
}

#[test]
fn every_leaf_on_every_route_prints_canonically_and_reparses() {
    // Aliased routes print with the canonical field names.
    let routes = [
        ("controller::cpu::cpu::", "controller::cpu::cpu::"),
        ("ctl::host::cpu::", "controller::cpu::cpu::"),
        ("controller::host::cpu::", "controller::cpu::cpu::"),
        ("spare::cpu::", "spare::cpu::"),
    ];
    let mut leaves = BTreeSet::new();
    for leaf in LEAVES {
        for (route, canonical) in routes {
            let parsed = parse(&format!("{route}{leaf}"));
            let printed = parsed.to_source().unwrap();
            assert_eq!(printed, format!("{canonical}{leaf}"));
            assert_eq!(parse(&printed), parsed, "reparsing {printed:?}");
            let mnemonic = leaf.split(' ').next().unwrap();
            leaves.insert(format!("{canonical}{mnemonic}"));
        }
    }
    assert_eq!(
        leaves.len(),
        vihaco_runtime::__private::capabilities::<Machine>().count(),
        "every executable leaf is covered"
    );
}

#[test]
fn non_canonical_operand_spellings_normalize() {
    let cases = [
        ("spare::cpu::const_f64 1e2", "spare::cpu::const_f64 100.0"),
        ("spare::cpu::const_f64 -0", "spare::cpu::const_f64 -0.0"),
        ("spare::cpu::const_f32 2", "spare::cpu::const_f32 2.0"),
        ("spare::cpu::call   1,   @f", "spare::cpu::call 1, @f"),
        (
            "ctl::host::cpu::cond_br    @a,  @b",
            "controller::cpu::cpu::cond_br @a, @b",
        ),
    ];
    for (source, canonical) in cases {
        let parsed = parse(source);
        assert_eq!(parsed.to_source().unwrap(), canonical);
        assert_eq!(parse(canonical), parsed);
    }
}

#[test]
fn printing_writes_into_any_formatter() {
    struct Source<'a>(&'a Syntax);
    impl std::fmt::Display for Source<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.print(f)
        }
    }
    let parsed = parse("ctl::host::cpu::br @top");
    assert_eq!(
        format!("    {}", Source(&parsed)),
        "    controller::cpu::cpu::br @top"
    );
}

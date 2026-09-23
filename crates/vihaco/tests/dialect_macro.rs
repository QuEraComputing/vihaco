// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{Parse as _, dialect};

dialect! {
    arith {
        /// Adds a signed amount when enabled.
        #[pattern = "'add $0 `,` $1"]
        Add(u32 => i64, bool),
        Return(u32),
        Halt,
        #[pattern = "'swap $1 $0"]
        Swap(u32, u32),
    }
}

macro_rules! capture_instruction_descriptors {
    (
        dialect: { $($dialect:tt)+ },
        context: { $component:ty },
        instructions: {
            $(
                {
                    ident: { $ident:ident },
                    runtime: { $runtime:ty },
                    syntax: { $syntax:ty },
                    mnemonic: { $mnemonic:literal },
                    ordinal: { $ordinal:expr },
                },
            )*
        },
    ) => {
        const ARITH_INSTRUCTION_METADATA: &[(&str, &str, usize)] = &[
            $((stringify!($ident), $mnemonic, $ordinal)),*
        ];

        fn assert_arith_instruction_descriptor_types() {
            let _: Option<$component> = None;
            $(
                let _: Option<$runtime> = None;
                let _: Option<$syntax> = None;
            )*
        }
    };
}

arith::__vihaco_instructions! {
    callback: capture_instruction_descriptors,
    dialect: { arith },
    context: { arith::Add },
}

pub mod payload_types {
    pub type RuntimeWord = i64;
    pub type SurfaceWord = u32;
}

dialect! {
    payloads {
        Convert(
            payload_types::SurfaceWord => payload_types::RuntimeWord,
            Vec<u32> => Box<[i64]>,
        ),
        Empty(),
    }
}

mod nested {
    use vihaco::dialect;

    dialect! {
        control {
            Start,
        }
    }
}

use nested::control as control_alias;

macro_rules! capture_aliased_instruction_descriptor {
    (
        dialect: { $($dialect:tt)+ },
        context: {},
        instructions: {
            {
                ident: { Start },
                runtime: { $runtime:ty },
                syntax: { $syntax:ty },
                mnemonic: { "start" },
                ordinal: { $ordinal:expr },
            },
        },
    ) => {
        fn assert_aliased_instruction_descriptor_types() {
            let _: Option<$runtime> = None;
            let _: Option<$syntax> = None;
            let _: Option<$($dialect)+::Start> = None;
            let _: [(); 0] = [(); $ordinal];
        }
    };
}

control_alias::__vihaco_instructions! {
    callback: capture_aliased_instruction_descriptor,
    dialect: { control_alias },
    context: {},
}

#[test]
fn generates_canonical_instruction_structs_with_runtime_payloads() {
    let add = arith::Add(-4_i64, true);
    assert_eq!(add, arith::Add(-4, true));

    let returned = arith::Return(7_u32);
    assert_eq!(returned.0, 7);

    assert_eq!(arith::Halt, arith::Halt);
    assert_eq!(arith::Swap(1, 2), arith::Swap(1, 2));
}

#[test]
fn exports_structured_instruction_descriptors_to_callbacks() {
    assert_arith_instruction_descriptor_types();
    assert_eq!(
        ARITH_INSTRUCTION_METADATA,
        &[
            ("Add", "add", 0),
            ("Return", "return", 1),
            ("Halt", "halt", 2),
            ("Swap", "swap", 3),
        ]
    );
}

#[test]
fn qualifies_instruction_enumerator_by_aliased_nested_dialect_module() {
    assert_aliased_instruction_descriptor_types();
}

#[test]
fn parses_explicit_and_generated_instruction_patterns() {
    let add = arith::syntax::Add::parser()
        .parse("arith.add 4, true")
        .into_result()
        .unwrap();
    assert_eq!(add, arith::syntax::Add(4_u32, true));

    let returned = arith::syntax::Return::parser()
        .parse("arith.return 7")
        .into_result()
        .unwrap();
    assert_eq!(returned, arith::syntax::Return(7));

    let halt = arith::syntax::Halt::parser()
        .parse("arith.halt")
        .into_result()
        .unwrap();
    assert_eq!(halt, arith::syntax::Halt);

    let swap = arith::syntax::Swap::parser()
        .parse("arith.swap 2 1")
        .into_result()
        .unwrap();
    assert_eq!(swap, arith::syntax::Swap(1, 2));
}

#[test]
fn each_instruction_parser_rejects_other_or_malformed_inputs() {
    assert!(
        arith::syntax::Add::parser()
            .parse("arith.halt")
            .has_errors()
    );
    assert!(
        arith::syntax::Add::parser()
            .parse("other.add 4, true")
            .has_errors()
    );
    assert!(
        arith::syntax::Add::parser()
            .parse("add 4, true")
            .has_errors()
    );
    assert!(
        arith::syntax::Add::parser()
            .parse("arith.add 4, true trailing")
            .has_errors()
    );
}

#[test]
fn supports_mapped_qualified_generic_and_trailing_comma_payloads() {
    let parsed = payloads::syntax::Convert::parser()
        .parse("payloads.convert 9, [1, 2]")
        .into_result()
        .unwrap();
    assert_eq!(parsed, payloads::syntax::Convert(9, vec![1, 2]));

    let runtime = payloads::Convert(-9, vec![1_i64, 2].into_boxed_slice());
    assert_eq!(runtime.0, -9);
    assert_eq!(&*runtime.1, &[1, 2]);

    assert_eq!(
        payloads::syntax::Empty::parser()
            .parse("payloads.empty")
            .into_result()
            .unwrap(),
        payloads::syntax::Empty
    );
    assert_eq!(payloads::Empty, payloads::Empty);
}

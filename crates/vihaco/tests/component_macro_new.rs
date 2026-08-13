// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{Parse, SurfaceInstruction, component};
use vihaco_parser::Ident;

#[derive(Clone, Debug, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(type)]
pub enum SurfaceType {
    #[pattern = "`u32`"]
    U32,
}

#[derive(Clone, Debug, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(value)]
pub enum SurfaceValue {
    #[pattern = "$0"]
    Bare(vihaco_parser::BareToken),
}

component! {
    #[derive(Default, Debug)]
    pub component Demo {
        state: u8,
    }

    type Type = vihaco::Type;
    value Value = vihaco::Value;

    instruction {
        #[pattern = "'branch `@` $0"]
        Branch(Ident => u32),
        Return(u32),
        Add(SurfaceType => Type),
        Const(SurfaceType => Type, SurfaceValue => Value),
        Halt,
    }
}

fn require_surface_instruction<T: SurfaceInstruction>() {}

#[test]
fn generates_distinct_syntax_and_runtime_enums() {
    require_surface_instruction::<demo::syntax::Instruction>();

    let component = demo::Demo::default();
    assert_eq!(format!("{component:?}"), "Demo { state: 0 }");

    let syntax = demo::syntax::Instruction::parser()
        .parse("demo::branch @target")
        .into_result()
        .unwrap();
    assert_eq!(
        syntax,
        demo::syntax::Instruction::Branch(Ident("target".into()))
    );

    let _: demo::runtime::Instruction = demo::runtime::Instruction::Branch(7);
    let _: demo::runtime::Instruction = demo::runtime::Instruction::Return(0);
    let _: demo::runtime::Type = vihaco::Type::U32;
    let _: demo::runtime::Value = vihaco::Value::U64(1);
}

#[test]
fn infers_patterns_for_unit_and_payload_variants() {
    assert_eq!(
        demo::syntax::Instruction::parser()
            .parse("demo::halt")
            .into_result(),
        Ok(demo::syntax::Instruction::Halt)
    );
}

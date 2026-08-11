// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{InstructionSet, Parse, SurfaceInstruction};

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(value)]
enum LocalValue {
    Number(i64),
}

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(type)]
enum LocalType {
    #[pattern = "`i64`"]
    I64,
}

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(instruction)]
enum LocalInstruction {
    #[pattern = "'local::add $0"]
    Add(i64),
    #[pattern = "'local::reset"]
    Reset,
}

#[allow(dead_code)]
struct LocalInstructionSet;

impl InstructionSet for LocalInstructionSet {
    type Instruction = LocalInstruction;
    type Value = LocalValue;
    type Type = LocalType;
}

fn require_surface_instruction<T: SurfaceInstruction>() {}

#[test]
fn local_syntax_is_parser_complete_without_mounting_context() {
    require_surface_instruction::<LocalInstruction>();
    assert_eq!(
        LocalInstruction::parser()
            .parse("local::add 7")
            .into_result(),
        Ok(LocalInstruction::Add(7))
    );
    assert_eq!(
        LocalValue::parser().parse("42").into_result(),
        Ok(LocalValue::Number(42))
    );
    assert_eq!(
        LocalType::parser().parse("i64").into_result(),
        Ok(LocalType::I64)
    );
}

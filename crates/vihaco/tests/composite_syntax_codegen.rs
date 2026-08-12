// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{FromText, InstructionSet, ModuleSyntax, Parse, SstHeader, composite};

#[derive(Clone, Debug, PartialEq, vihaco::Parse)]
#[syntax_class(instruction)]
enum LocalInstruction {
    #[pattern = "'step $0"]
    Step(u32),
}

#[derive(Clone, Debug, PartialEq, vihaco::Parse)]
#[syntax_class(type)]
enum LocalType {
    #[pattern = "`unit`"]
    Unit,
}

#[derive(Clone, Debug, PartialEq, vihaco::Parse)]
#[syntax_class(value)]
enum LocalValue {
    #[pattern = "`zero`"]
    Zero,
}

struct SyntaxComponent;

impl InstructionSet for SyntaxComponent {
    type Instruction = LocalInstruction;
    type Value = LocalValue;
    type Type = LocalType;
}

struct RuntimeOnly;

#[derive(Clone, Debug, PartialEq)]
pub struct MachineHeader;

impl FromText for MachineHeader {
    fn from_text(_text: &str) -> eyre::Result<Self> {
        Ok(Self)
    }
}

impl SstHeader for MachineHeader {}

composite! {
    #[allow(dead_code)]
    pub composite MountedSyntax {
        #[device(0x01)]
        #[syntax("left", "alias")]
        left: SyntaxComponent,

        #[device(0x02)]
        #[syntax("right")]
        right: SyntaxComponent,

        #[device(0x03)]
        runtime_only: RuntimeOnly,
    }
    syntax {
        header MachineHeader => resolve_header;
    }
}

fn require_module_syntax<S: ModuleSyntax>() {}
fn require_machine_header<S: ModuleSyntax<Header = MachineHeader>>() {}

#[test]
fn generated_syntax_wraps_mounts_and_aliases() {
    require_module_syntax::<mounted_syntax::syntax::Module>();
    require_machine_header::<mounted_syntax::syntax::Module>();

    let left = mounted_syntax::syntax::Instruction::parser()
        .parse("left::step 7")
        .into_result()
        .unwrap();
    let alias = mounted_syntax::syntax::Instruction::parser()
        .parse("alias::step 8")
        .into_result()
        .unwrap();
    let right = mounted_syntax::syntax::Instruction::parser()
        .parse("right::step 9")
        .into_result()
        .unwrap();

    assert!(matches!(
        left,
        mounted_syntax::syntax::Instruction::Left(LocalInstruction::Step(7))
    ));
    assert!(matches!(
        alias,
        mounted_syntax::syntax::Instruction::Left(LocalInstruction::Step(8))
    ));
    assert!(matches!(
        right,
        mounted_syntax::syntax::Instruction::Right(LocalInstruction::Step(9))
    ));
}

#[test]
fn runtime_only_mount_does_not_require_instruction_set() {
    let _: MountedSyntax = MountedSyntax {
        left: SyntaxComponent,
        right: SyntaxComponent,
        runtime_only: RuntimeOnly,
    };
}

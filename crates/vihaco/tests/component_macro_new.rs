// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{
    Component, Composite, HasInstructionSet, Parse, SurfaceInstruction, component, composite,
};
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

component! {
    pub component Simple {}

    instruction {
        #[pattern = "'halt"]
        Halt,
    }
}

#[derive(Clone, Debug, PartialEq, vihaco::Instruction)]
enum RawRuntimeInstruction {
    Halt,
}

struct Raw;

impl vihaco::HasInstructionSet for Raw {
    type Runtime = RawRuntimeInstruction;
    type Syntax = simple::syntax::Instruction;
}

impl vihaco::Component for Raw {}

impl vihaco::GeneratedComponent for Raw {
    type Instruction = RawRuntimeInstruction;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<vihaco::Effects<Self::Effect>> {
        Ok(vihaco::Effects::none())
    }
}

fn require_surface_instruction<T: SurfaceInstruction>() {}

#[composite]
#[allow(dead_code)]
struct DemoComposite {
    #[device(0x01, alias = "alias")]
    raw: Raw,
}

#[test]
fn generates_distinct_syntax_and_runtime_enums() {
    fn require_component<
        T: Component<Runtime = demo::runtime::Instruction, Syntax = demo::syntax::Instruction>,
    >() {
    }
    require_component::<demo::Demo>();
    require_surface_instruction::<demo::syntax::Instruction>();

    let component = demo::Demo::default();
    assert_eq!(format!("{component:?}"), "Demo { state: 0 }");

    let syntax = demo::syntax::Instruction::parser()
        .parse("demo.branch @target")
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
fn composite_generates_component_instruction_modules() {
    let _composite = DemoComposite { raw: Raw };
    fn require_runtime<T>() {}
    require_runtime::<demo_composite::runtime::Instruction>();

    fn require_composite<T>()
    where
        T: Composite
            + HasInstructionSet<
                Runtime = demo_composite::runtime::Instruction,
                Syntax = demo_composite::syntax::Instruction,
            >,
    {
    }
    require_composite::<DemoComposite>();

    let parsed = demo_composite::syntax::Instruction::parser()
        .parse("raw::simple.halt")
        .into_result()
        .unwrap();
    assert_eq!(
        parsed,
        demo_composite::syntax::Instruction::Raw(simple::syntax::Instruction::Halt)
    );

    let aliased = demo_composite::syntax::Instruction::parser()
        .parse("alias::simple.halt")
        .into_result()
        .unwrap();
    assert_eq!(
        aliased,
        demo_composite::syntax::Instruction::Raw(simple::syntax::Instruction::Halt)
    );
}

#[test]
fn infers_patterns_for_unit_and_payload_variants() {
    assert_eq!(
        demo::syntax::Instruction::parser()
            .parse("demo.halt")
            .into_result(),
        Ok(demo::syntax::Instruction::Halt)
    );
}

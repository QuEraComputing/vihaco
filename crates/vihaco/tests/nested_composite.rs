// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco::{Component, Parse, SurfaceInstruction, component, composite};

component! {
    pub component LeftClock {}

    instruction {
        #[pattern = "'tick"]
        Tick,
    }
}

component! {
    pub component LeftWaveform {}

    instruction {
        #[pattern = "'sample"]
        Sample,
    }
}

component! {
    pub component RightDrive {}

    instruction {
        #[pattern = "'drive"]
        Drive,
    }
}

component! {
    pub component RightReadout {}

    instruction {
        #[pattern = "'read"]
        Read,
    }
}

#[composite]
#[allow(dead_code)]
struct LeftComposite {
    #[device(0x01)]
    clock: left_clock::LeftClock,

    #[device(0x02)]
    waveform: left_waveform::LeftWaveform,
}

impl Component for LeftComposite {
    type Runtime = left_composite::runtime::Instruction;
    type Syntax = left_composite::syntax::Instruction;
}

#[composite]
#[allow(dead_code)]
struct RightComposite {
    #[device(0x01)]
    drive: right_drive::RightDrive,

    #[device(0x02)]
    readout: right_readout::RightReadout,
}

impl Component for RightComposite {
    type Runtime = right_composite::runtime::Instruction;
    type Syntax = right_composite::syntax::Instruction;
}

#[composite]
#[allow(dead_code)]
struct ControlComposite {
    #[device(0x10)]
    left: LeftComposite,

    #[device(0x20)]
    right: RightComposite,
}

#[test]
fn composites_sum_component_instruction_sets_and_can_be_nested() {
    fn require_surface_instruction<T: SurfaceInstruction>() {}
    require_surface_instruction::<left_composite::syntax::Instruction>();
    require_surface_instruction::<right_composite::syntax::Instruction>();
    require_surface_instruction::<control_composite::syntax::Instruction>();

    let left_parser = left_composite::syntax::Instruction::parser();
    assert_eq!(
        left_parser.parse("clock::tick").into_result(),
        Ok(left_composite::syntax::Instruction::Clock(
            left_clock::syntax::Instruction::Tick,
        ))
    );
    assert_eq!(
        left_composite::syntax::Instruction::parser()
            .parse("waveform::sample")
            .into_result(),
        Ok(left_composite::syntax::Instruction::Waveform(
            left_waveform::syntax::Instruction::Sample,
        ))
    );

    assert_eq!(
        right_composite::syntax::Instruction::parser()
            .parse("drive::drive")
            .into_result(),
        Ok(right_composite::syntax::Instruction::Drive(
            right_drive::syntax::Instruction::Drive,
        ))
    );
    assert_eq!(
        right_composite::syntax::Instruction::parser()
            .parse("readout::read")
            .into_result(),
        Ok(right_composite::syntax::Instruction::Readout(
            right_readout::syntax::Instruction::Read,
        ))
    );

    let _: left_composite::runtime::Instruction =
        left_composite::runtime::Instruction::Clock(left_clock::runtime::Instruction::Tick);
    let _: left_composite::runtime::Instruction =
        left_composite::runtime::Instruction::Waveform(left_waveform::runtime::Instruction::Sample);
    let _: right_composite::runtime::Instruction =
        right_composite::runtime::Instruction::Drive(right_drive::runtime::Instruction::Drive);
    let _: right_composite::runtime::Instruction =
        right_composite::runtime::Instruction::Readout(right_readout::runtime::Instruction::Read);

    assert_eq!(
        control_composite::syntax::Instruction::parser()
            .parse("left::waveform::sample")
            .into_result(),
        Ok(control_composite::syntax::Instruction::Left(
            left_composite::syntax::Instruction::Waveform(
                left_waveform::syntax::Instruction::Sample,
            ),
        ))
    );
    assert_eq!(
        control_composite::syntax::Instruction::parser()
            .parse("right::readout::read")
            .into_result(),
        Ok(control_composite::syntax::Instruction::Right(
            right_composite::syntax::Instruction::Readout(right_readout::syntax::Instruction::Read,),
        ))
    );
}

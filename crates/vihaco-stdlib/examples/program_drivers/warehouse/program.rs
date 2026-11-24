// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Encode Warehouse bytecode and load it into the checked runner's module.

use super::{__vihaco_WarehouseInstruction, Instruction, Module, ops, records};
use eyre::Result;
use vihaco_abi::{Type, instruction::WriteBytes};
use vihaco_bytecode::decode_instruction_stream;
use vihaco_module::module::{FunctionInfo, Signature};
use vihaco_stdlib::program::program;

#[derive(vihaco_abi::Instruction)]
pub(super) enum Bytecode {
    PrintLabel(u32),
    VerifyLabel(u32),
    Halt,
}

impl From<Bytecode> for Instruction {
    fn from(instruction: Bytecode) -> Self {
        use __vihaco_WarehouseInstruction::*;
        match instruction {
            Bytecode::PrintLabel(parcel) => V_printer__ops__PrintLabel(ops::PrintLabel(parcel)),
            Bytecode::VerifyLabel(parcel) => {
                V_parcels__records__VerifyLabel(records::VerifyLabel(parcel))
            }
            Bytecode::Halt => V_execution__program__Halt(program::Halt),
        }
    }
}

pub(super) fn load(instructions: &[Bytecode]) -> Result<Module<Instruction>> {
    let mut bytes = Vec::new();
    for instruction in instructions {
        instruction.write_bytes(&mut bytes)?;
    }
    let code = decode_instruction_stream::<Bytecode>(&bytes)?
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();
    Ok(Module {
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature::<Type> {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            start_address: 0,
            end_address: u32::try_from(code.len())?,
            file: 0,
        }],
        code,
        strings: vec!["main".into()],
        main_function: Some(0),
        ..Default::default()
    })
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Encode and decode warehouse bytecode.

use super::{__vihaco_WarehouseInstruction, Warehouse, ops, records};
use vihaco_runtime::HasInstructionSet;

pub(super) type Instruction = <Warehouse as HasInstructionSet>::Runtime;

#[derive(vihaco_abi::Instruction)]
pub(super) enum Bytecode {
    PrintLabel(u32),
    VerifyLabel(u32),
}

impl From<Bytecode> for Instruction {
    fn from(instruction: Bytecode) -> Self {
        match instruction {
            Bytecode::PrintLabel(parcel) => {
                __vihaco_WarehouseInstruction::V_printer__ops__PrintLabel(ops::PrintLabel(parcel))
            }
            Bytecode::VerifyLabel(parcel) => {
                __vihaco_WarehouseInstruction::V_parcels__records__VerifyLabel(
                    records::VerifyLabel(parcel),
                )
            }
        }
    }
}

pub(super) type Program = super::program_driver::Program<Instruction>;

#[cfg(test)]
mod tests {
    use super::super::program_driver::{ProgramDriver, encode};
    use super::*;
    use crate::framework::ChannelDriver;
    use eyre::Result;

    #[test]
    fn route_failure_keeps_the_pc_and_stops_subsequent_dispatch() -> Result<()> {
        let bytes = encode(&[Bytecode::VerifyLabel(42), Bytecode::PrintLabel(42)])?;
        let program = Program::load::<Bytecode>(&bytes)?;
        let mut machine = Warehouse::default();
        let backend = ChannelDriver::new();
        let mut driver = ProgramDriver::new(&program, &backend);
        let error = driver.run(&mut machine, |_, _| unreachable!()).unwrap_err();
        assert_eq!(error.to_string(), "parcel 42 has no label");
        assert_eq!(driver.pc(), 0);
        assert_eq!(machine.printer.submitted, 0);
        assert!(driver.run(&mut machine, |_, _| unreachable!()).is_err());
        Ok(())
    }
}

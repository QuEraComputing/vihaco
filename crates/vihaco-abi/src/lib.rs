// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco_abi;

pub mod effect;
pub mod frame;
pub mod instruction_syntax;
pub mod metadata;
pub mod program;
#[doc(hidden)]
pub mod traits;

/// Bytecode-instruction traits, mounted at a stable `instruction` path so that
/// derive-generated code can resolve `::<root>::instruction::OpCode` whether the
/// resolved root is `vihaco_abi` (direct dependency) or the `vihaco` facade.
pub mod instruction {
    pub use crate::traits::{FromBytes, FromBytesWithOpcode, Instruction, OpCode, WriteBytes};
}

pub use effect::Effects;
pub use program::{Type, Value};

#[cfg(feature = "derive")]
pub use vihaco_abi_derive::Instruction;

// Exercises the `FoundCrate::Itself` branch of the derive's root resolver: used
// inside `vihaco-abi` itself, `#[derive(Instruction)]` must root generated code
// at `crate`, resolving `crate::instruction::{OpCode, ..}` via the shim above.
#[cfg(all(test, feature = "derive"))]
mod derive_root_self_tests {
    use crate::instruction::{FromBytes, OpCode, WriteBytes};

    #[derive(Debug, PartialEq, crate::Instruction)]
    enum Isa {
        Nop,
        PushI64(i64),
    }

    #[test]
    fn round_trips_with_crate_root() {
        for inst in [Isa::Nop, Isa::PushI64(-7)] {
            let opcode = inst.opcode();
            let mut buf = Vec::new();
            inst.write_bytes(&mut buf).unwrap();
            assert_eq!(buf[0], opcode);

            let mut cursor = std::io::Cursor::new(buf);
            assert_eq!(Isa::from_bytes(&mut cursor).unwrap(), inst);
        }
    }
}

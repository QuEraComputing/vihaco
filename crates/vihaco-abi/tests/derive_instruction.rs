// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Smoke test for `#[derive(vihaco_abi::Instruction)]`.
//!
//! Deriving from within `vihaco-abi`'s own test target exercises the
//! `FoundCrate::Itself` branch of the root resolver (root == `crate`), proving
//! the derive + the abi `instruction` shim resolve together and round-trip.

use vihaco_abi::instruction::{FromBytes, OpCode, WriteBytes};

#[derive(Debug, PartialEq, vihaco_abi::Instruction)]
enum Isa {
    Nop,
    PushI64(i64),
    PushBool(bool),
}

#[test]
fn opcodes_are_assigned_by_declaration_order() {
    assert_eq!(Isa::Nop.opcode(), 0);
    assert_eq!(Isa::PushI64(0).opcode(), 1);
    assert_eq!(Isa::PushBool(false).opcode(), 2);
}

#[test]
fn round_trips_through_bytes() {
    for inst in [Isa::Nop, Isa::PushI64(-42), Isa::PushBool(true)] {
        let mut buf = Vec::new();
        inst.write_bytes(&mut buf).unwrap();
        assert_eq!(buf.len(), Isa::width() as usize);

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = Isa::from_bytes(&mut cursor).unwrap();
        assert_eq!(decoded, inst);
    }
}

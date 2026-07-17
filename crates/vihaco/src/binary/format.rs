// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::traits::{FromBytes, FromText, Instruction, OpCode, WriteBytes};

pub use vihaco_parser_core::container::bytecode::{FLAGS, MAGIC, VERSION};

pub trait BytecodeHeader: Sized + FromBytes {}

impl<T> BytecodeHeader for T where T: Sized + FromBytes {}

pub trait SstHeader: Sized + FromText {}

pub trait WriteBytecodeHeader: WriteBytes {}

impl<T> WriteBytecodeHeader for T where T: WriteBytes {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstantId(pub u32);

impl From<u32> for ConstantId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<ConstantId> for u32 {
    fn from(value: ConstantId) -> Self {
        value.0
    }
}

impl FromBytes for ConstantId {
    fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
        Ok(Self(bytes.read_u32::<LittleEndian>()?))
    }
}

impl WriteBytes for ConstantId {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        use byteorder::WriteBytesExt;

        io.write_u32::<LittleEndian>(self.0)?;
        Ok(())
    }
}

impl OpCode for ConstantId {
    fn width() -> u32 {
        4
    }

    fn opcode(&self) -> u8 {
        0
    }
}

impl std::fmt::Display for ConstantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

pub fn decode_instruction_stream<I: Instruction>(bytes: &[u8]) -> eyre::Result<Vec<I>> {
    let width = I::width() as usize;
    if width == 0 {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        return Err(eyre::eyre!(
            "cannot decode {} byte(s) into zero-width instructions",
            bytes.len()
        ));
    }
    if !bytes.len().is_multiple_of(width) {
        return Err(eyre::eyre!(
            "bytecode length {} is not a multiple of instruction width {}",
            bytes.len(),
            width
        ));
    }

    let mut cursor = Cursor::new(bytes);
    let mut code = Vec::with_capacity(bytes.len() / width);
    while cursor.position() as usize != bytes.len() {
        code.push(I::from_bytes(&mut cursor)?);
    }
    Ok(code)
}

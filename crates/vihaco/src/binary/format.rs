// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::traits::{FromBytes, Instruction, OpCode, WriteBytes};

pub trait CompositeHeader: Sized + FromBytes + WriteBytes {}

impl<T> CompositeHeader for T where T: Sized + FromBytes + WriteBytes {}

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

/// The vihaco bytecode file header.
///
/// The file header only frames the shared [`ProgramContext`], with the root section
/// starting immediately after the context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BytecodeHeader {
    pub(super) context_len: usize,
}

impl BytecodeHeader {
    const MAGIC: &'static [u8; 4] = b"VHBC";
    const VERSION: u16 = 1;

    /// Currently unused in version 1, and we will reject any binary
    /// that has flags set
    const FLAGS: u16 = 0;

    const CONTEXT_LEN_SIZE: usize = 8;

    pub(super) const ENCODED_LEN: usize = 4 + 2 + 2 + Self::CONTEXT_LEN_SIZE;

    pub(super) fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if &magic != Self::MAGIC {
            return Err(eyre::eyre!("invalid bytecode magic"));
        }

        let version = reader.read_u16::<LittleEndian>()?;
        if version != Self::VERSION {
            return Err(eyre::eyre!(
                "unsupported bytecode version {} (expected {})",
                version,
                VERSION
            ));
        }

        let flags = reader.read_u16::<LittleEndian>()?;
        if flags != Self::FLAGS {
            return Err(eyre::eyre!("unsupported bytecode flags 0x{flags:04X}"));
        }

        Ok(Self {
            context_len: read_usize_u64(reader, "program context length")?,
        })
    }
}

pub const MAGIC: &[u8; 4] = BytecodeHeader::MAGIC;
pub const VERSION: u16 = BytecodeHeader::VERSION;
pub const FLAGS: u16 = BytecodeHeader::FLAGS;

/// The frame data for a section in a bytecode file.
///
/// The section length describes the length of the entire section, starting
/// from the beginning of the section frame to the end of the last child
/// section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SectionFrame {
    pub(super) section_len: usize,
    pub(super) composite_header_len: usize,
}

impl SectionFrame {
    pub(super) const SECTION_LEN_SIZE: usize = 8;
    pub(super) const HEADER_LEN_SIZE: usize = 8;

    pub(super) const ENCODED_LEN: usize = Self::SECTION_LEN_SIZE + Self::HEADER_LEN_SIZE;

    pub(super) fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            section_len: read_usize_u64(reader, "section length")?,
            composite_header_len: read_usize_u64(reader, "composite header length")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SectionBytecodeHeader {
    pub(super) bytecode_len: usize,
}

impl SectionBytecodeHeader {
    pub(super) const BYTECODE_LEN_SIZE: usize = 8;

    pub(super) const ENCODED_LEN: usize = Self::BYTECODE_LEN_SIZE;

    pub(super) fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            bytecode_len: read_usize_u64(reader, "bytecode length")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChildSectionTableHeader {
    pub(super) child_count: usize,
}

impl ChildSectionTableHeader {
    // Note: this is stored as a `usize` within the `ChildSectionTableHeader`
    // struct but will be 4 bytes (a `u32`) in the actual binary.
    pub(super) const CHILD_COUNT_SIZE: usize = 4;

    pub(super) const ENCODED_LEN: usize = Self::CHILD_COUNT_SIZE;

    pub(super) fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            child_count: reader.read_u32::<LittleEndian>()? as usize,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChildSectionTableEntry {
    pub(super) local_name_string: u32,
    pub(super) section_offset: usize,
}

impl ChildSectionTableEntry {
    pub(super) const LOCAL_NAME_STRING_SIZE: usize = 4;
    pub(super) const SECTION_OFFSET_SIZE: usize = 8;

    pub(super) const ENCODED_LEN: usize = Self::LOCAL_NAME_STRING_SIZE + Self::SECTION_OFFSET_SIZE;

    pub(super) fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            local_name_string: reader.read_u32::<LittleEndian>()?,
            section_offset: read_usize_u64(reader, "child section offset")?,
        })
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
    if bytes.len() % width != 0 {
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

fn read_usize_u64<R: Read>(reader: &mut R, label: &str) -> eyre::Result<usize> {
    usize::try_from(reader.read_u64::<LittleEndian>()?)
        .map_err(|_| eyre::eyre!("{label} does not fit in usize"))
}

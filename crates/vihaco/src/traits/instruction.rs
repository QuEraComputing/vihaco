// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub trait FromBytes {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized;
}

pub trait FromText {
    fn from_text<R: std::io::Read>(text: &mut R) -> eyre::Result<Self>
    where
        Self: Sized;
}

pub trait FromBytesWithOpcode: Sized {
    fn from_bytes_with_opcode<R: std::io::Read>(bytes: &mut R, opcode: u8) -> eyre::Result<Self>;
}

impl<T> FromBytes for T
where
    T: FromBytesWithOpcode,
{
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let mut buf = [0u8; 1];
        bytes.read_exact(&mut buf)?;
        let opcode = buf[0];
        T::from_bytes_with_opcode(bytes, opcode)
    }
}

pub trait WriteBytes {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()>;
}

pub trait OpCode {
    fn width() -> u32;
    fn opcode(&self) -> u8;
}

pub trait Instruction: Sized + OpCode + FromBytes + WriteBytes {}

impl<T> Instruction for T where T: Sized + OpCode + FromBytes + WriteBytes {}

macro_rules! impl_scalar_instruction_traits {
    ($ty:ty, $read:ident, $write:ident, $width:expr) => {
        impl FromBytes for $ty {
            fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
            where
                Self: Sized,
            {
                use byteorder::{LittleEndian, ReadBytesExt};
                Ok(bytes.$read::<LittleEndian>()?)
            }
        }

        impl WriteBytes for $ty {
            fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
                use byteorder::{LittleEndian, WriteBytesExt};
                io.$write::<LittleEndian>(*self)?;
                Ok(())
            }
        }

        impl OpCode for $ty {
            fn width() -> u32 {
                $width
            }

            fn opcode(&self) -> u8 {
                0
            }
        }
    };
}

impl_scalar_instruction_traits!(u32, read_u32, write_u32, 4);
impl_scalar_instruction_traits!(u64, read_u64, write_u64, 8);
impl_scalar_instruction_traits!(i64, read_i64, write_i64, 8);
impl_scalar_instruction_traits!(f64, read_f64, write_f64, 8);

impl FromBytes for bool {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        use byteorder::ReadBytesExt;
        Ok(bytes.read_u8()? != 0)
    }
}

impl WriteBytes for bool {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        io.write_all(&[u8::from(*self)])?;
        Ok(())
    }
}

impl OpCode for bool {
    fn width() -> u32 {
        1
    }

    fn opcode(&self) -> u8 {
        0
    }
}

impl FromBytes for () {
    fn from_bytes<R: std::io::Read>(_bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl WriteBytes for () {
    fn write_bytes<W: std::io::Write>(&self, _io: &mut W) -> eyre::Result<()> {
        Ok(())
    }
}

impl OpCode for () {
    fn width() -> u32 {
        0
    }

    fn opcode(&self) -> u8 {
        0
    }
}

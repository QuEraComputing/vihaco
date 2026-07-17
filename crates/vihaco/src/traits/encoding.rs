// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub trait FromBytes {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized;
}

pub trait FromText {
    fn from_text(text: &str) -> eyre::Result<Self>
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

macro_rules! impl_scalar_encoding_traits {
    ($ty:ty, $read:ident, $write:ident) => {
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
    };
}

impl_scalar_encoding_traits!(u32, read_u32, write_u32);
impl_scalar_encoding_traits!(u64, read_u64, write_u64);
impl_scalar_encoding_traits!(i64, read_i64, write_i64);
impl_scalar_encoding_traits!(f64, read_f64, write_f64);

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

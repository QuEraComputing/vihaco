// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chumsky::{error::Simple, extra, prelude::*};
use eyre::Result;

type ValueParseExtra<'src> = extra::Err<Simple<'src, char>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Undefined,
    /// a string value from the string interner
    String(u32),
    /// a boolean value
    Bool(bool),
    /// a signed 64-bit integer
    I64(i64),
    /// an unsigned 32-bit integer
    U32(u32),
    /// an unsigned 64-bit integer
    U64(u64),
    /// a 64-bit floating point number
    F64(f64),
    /// a reference to a function by its index in the function table
    FunctionRef(u32),
    /// a reference to an immutable heap object by its index in the heap table
    HeapRef(u32),
}

impl Value {
    pub fn get_function_ref(&self) -> Result<u32> {
        if let Value::FunctionRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected FunctionRef, found {:?}", self))
        }
    }

    pub fn get_heap_ref(&self) -> Result<u32> {
        if let Value::HeapRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected HeapRef, found {:?}", self))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    Undefined,
    String,
    Bool,
    I64,
    U32,
    U64,
    F64,
    FunctionRef,
    HeapRef,
}

impl AsRef<Type> for Type {
    fn as_ref(&self) -> &Type {
        self
    }
}

impl Value {
    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn type_of(&self) -> Type {
        match self {
            Value::Undefined => Type::Undefined,
            Value::String(_) => Type::String,
            Value::Bool(_) => Type::Bool,
            Value::I64(_) => Type::I64,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::F64(_) => Type::F64,
            Value::FunctionRef(_) => Type::FunctionRef,
            Value::HeapRef(_) => Type::HeapRef,
        }
    }

    pub fn cast(&self, to: impl AsRef<Type>) -> Result<Value> {
        match (self, to.as_ref()) {
            (Value::I64(v), Type::U64) => Ok(Value::U64(*v as u64)),
            (Value::I64(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::U32(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::U32(v), Type::U64) => Ok(Value::U64(*v as u64)),
            (Value::U32(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::U64(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::U64(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::F64(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::F64(v), Type::U64) => Ok(Value::U64(*v as u64)),
            _ if self.type_of() == *to.as_ref() => Ok(*self),
            _ => Err(eyre::eyre!(
                "Cannot cast value {:?} to type {:?}",
                self,
                to.as_ref()
            )),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::String(s) => write!(f, "0x{:X}", s),
            Value::Bool(b) => b.fmt(f),
            Value::I64(i) => i.fmt(f),
            Value::U32(u) => u.fmt(f),
            Value::U64(u) => u.fmt(f),
            Value::F64(fl) => fl.fmt(f),
            Value::FunctionRef(id) => write!(f, "<fn {}>", id),
            Value::HeapRef(id) => write!(f, "<heap {}>", id),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Undefined => write!(f, "undefined"),
            Type::String => write!(f, "str"),
            Type::Bool => write!(f, "bool"),
            Type::I64 => write!(f, "i64"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::F64 => write!(f, "f64"),
            Type::FunctionRef => write!(f, "fn"),
            Type::HeapRef => write!(f, "heap"),
        }
    }
}

macro_rules! impl_from_for_value {
    ($t:ty => $variant:ident) => {
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Value::$variant(v)
            }
        }
    };
}

impl_from_for_value!(bool => Bool);
impl_from_for_value!(i64 => I64);
impl_from_for_value!(u32 => U32);
impl_from_for_value!(u64 => U64);
impl_from_for_value!(f64 => F64);

macro_rules! impl_try_from_for_rust {
    ($variant:ident => $t:ty) => {
        impl TryFrom<Value> for $t {
            type Error = eyre::Report;
            fn try_from(v: Value) -> Result<Self, Self::Error> {
                if let Value::$variant(v) = v {
                    Ok(v)
                } else {
                    Err(eyre::eyre!("Cannot convert {:?} to {}", v, stringify!($t)))
                }
            }
        }
    };
}

impl_try_from_for_rust!(Bool => bool);
impl_try_from_for_rust!(I64 => i64);
impl_try_from_for_rust!(U32 => u32);
impl_try_from_for_rust!(U64 => u64);
impl_try_from_for_rust!(F64 => f64);

impl crate::traits::FromBytes for Type {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let type_byte = bytes.read_u8()?;
        match type_byte {
            0x00 => Ok(Type::Undefined),
            0x01 => Ok(Type::String),
            0x02 => Ok(Type::Bool),
            0x03 => Ok(Type::I64),
            0x04 => Ok(Type::U32),
            0x05 => Ok(Type::U64),
            0x06 => Ok(Type::F64),
            0x07 => Ok(Type::FunctionRef),
            0x08 => Ok(Type::HeapRef),
            _ => Err(eyre::eyre!("Unknown type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for Type {
    fn from_text<R: std::io::Read>(text: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let mut buffer = String::new();
        text.read_to_string(&mut buffer)?;
        type_text_parser()
            .then_ignore(end())
            .parse(buffer.trim())
            .into_result()
            .map_err(format_value_parse_errors)
    }
}

impl crate::traits::WriteBytes for Type {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        let type_byte = match self {
            Type::Undefined => 0x00,
            Type::String => 0x01,
            Type::Bool => 0x02,
            Type::I64 => 0x03,
            Type::U32 => 0x04,
            Type::U64 => 0x05,
            Type::F64 => 0x06,
            Type::FunctionRef => 0x07,
            Type::HeapRef => 0x08,
        };
        io.write_u8(type_byte)?;
        Ok(())
    }
}

impl crate::traits::FromBytes for Value {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let type_byte = bytes.read_u8()?;
        match type_byte {
            0x00 => Ok(Value::Undefined),
            0x01 => {
                let str_id = bytes.read_u32::<LittleEndian>()?;
                Ok(Value::String(str_id))
            }
            0x02 => {
                let b = bytes.read_u8()?;
                Ok(Value::Bool(b != 0))
            }
            0x03 => {
                let i = bytes.read_i64::<LittleEndian>()?;
                Ok(Value::I64(i))
            }
            0x04 => {
                let u = bytes.read_u32::<LittleEndian>()?;
                Ok(Value::U32(u))
            }
            0x05 => {
                let u = bytes.read_u64::<LittleEndian>()?;
                Ok(Value::U64(u))
            }
            0x06 => {
                let f = bytes.read_f64::<LittleEndian>()?;
                Ok(Value::F64(f))
            }
            0x07 => {
                let addr = bytes.read_u32::<LittleEndian>()?;
                Ok(Value::FunctionRef(addr))
            }
            0x08 => {
                let addr = bytes.read_u32::<LittleEndian>()?;
                Ok(Value::HeapRef(addr))
            }
            _ => Err(eyre::eyre!("Unknown value type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for Value {
    fn from_text<R: std::io::Read>(text: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let mut buffer = String::new();
        text.read_to_string(&mut buffer)?;
        value_text_parser()
            .then_ignore(end())
            .parse(buffer.trim())
            .into_result()
            .map_err(format_value_parse_errors)
    }
}

impl crate::traits::WriteBytes for Value {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        match self {
            Value::Undefined => {
                io.write_u8(0x00)?;
            }
            Value::String(s) => {
                io.write_u8(0x01)?;
                io.write_all(&s.to_le_bytes())?;
            }
            Value::Bool(b) => {
                io.write_u8(0x02)?;
                io.write_u8(if *b { 1 } else { 0 })?;
            }
            Value::I64(i) => {
                io.write_u8(0x03)?;
                io.write_all(&i.to_le_bytes())?;
            }
            Value::U32(u) => {
                io.write_u8(0x04)?;
                io.write_all(&u.to_le_bytes())?;
            }
            Value::U64(u) => {
                io.write_u8(0x05)?;
                io.write_all(&u.to_le_bytes())?;
            }
            Value::F64(f) => {
                io.write_u8(0x06)?;
                io.write_all(&f.to_le_bytes())?;
            }
            Value::FunctionRef(addr) => {
                io.write_u8(0x07)?;
                io.write_all(&addr.to_le_bytes())?;
            }
            Value::HeapRef(addr) => {
                io.write_u8(0x08)?;
                io.write_all(&addr.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

fn type_text_parser<'src>() -> impl Parser<'src, &'src str, Type, ValueParseExtra<'src>> + Clone {
    choice((
        just("undefined").to(Type::Undefined),
        just("string").to(Type::String),
        just("str").to(Type::String),
        just("bool").to(Type::Bool),
        just("i64").to(Type::I64),
        just("u32").to(Type::U32),
        just("u64").to(Type::U64),
        just("f64").to(Type::F64),
        just("fn").to(Type::FunctionRef),
        just("heap").to(Type::HeapRef),
    ))
}

fn value_text_parser<'src>() -> impl Parser<'src, &'src str, Value, ValueParseExtra<'src>> + Clone {
    let ws = one_of(" \t").repeated().at_least(1).ignored();
    choice((
        just("undefined").to(Value::Undefined),
        just("string")
            .or(just("str"))
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(Value::String),
        just("bool")
            .ignore_then(ws)
            .ignore_then(choice((just("true").to(true), just("false").to(false))))
            .map(Value::Bool),
        just("i64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<i64>())
            .map(Value::I64),
        just("u32")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(Value::U32),
        just("u64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u64>())
            .map(Value::U64),
        just("f64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<f64>())
            .map(Value::F64),
        just("fn")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(Value::FunctionRef),
        just("heap")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(Value::HeapRef),
    ))
}

fn scalar_text<'src, T>() -> impl Parser<'src, &'src str, T, ValueParseExtra<'src>> + Clone
where
    T: std::str::FromStr,
{
    any()
        .filter(|ch: &char| !ch.is_whitespace())
        .repeated()
        .at_least(1)
        .to_slice()
        .try_map(|text: &str, span| text.parse::<T>().map_err(|_| Simple::new(None, span)))
}

fn format_value_parse_errors(errors: Vec<Simple<'_, char>>) -> eyre::Report {
    let error = errors
        .into_iter()
        .next()
        .map(|error| format!("{error:?}"))
        .unwrap_or_else(|| "unknown parse error".to_string());
    eyre::eyre!("failed to parse value text: {error}")
}

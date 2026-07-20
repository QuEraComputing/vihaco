// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chumsky::{error::Simple, extra, prelude::*};
use eyre::Result;
use vihaco_parser::Parse;

use crate::{
    traits::ByteWidth,
    value::{ResolutionContext, Type, Value},
};

/// The resolution contract for an architecture that uses the CPU component.
pub trait CPUResolutionContext: ResolutionContext {
    fn intern_string(str: impl Into<String>) -> u32;
    fn function_index(fun: impl Into<String>) -> u32;
}

#[derive(Debug, Clone, PartialEq, Parse)]
#[syntax_class(value)]
pub enum CPUValueSyntax {
    Symbol(String),
    U32(u32),
    Const(u32),
}

// the idea here will be to allow for vihaco_parser::Parse to derive
// a parser from a struct using a declarative form
// inspo: MLIR `assemblyFormat`
#[derive(Parse)]
#[syntax_class(value)]
#[pattern = "$ty $value"]
pub struct CPUConstSyntax {
    pub ty: CPUType,
    pub value: CPUConstValueSyntax,
}

#[derive(Parse)]
#[syntax_class(value)]
pub struct CPUConstValueSyntax(pub String);

impl<T> From<T> for CPUConstValueSyntax
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

type ValueParseExtra<'src> = extra::Err<Simple<'src, char>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CPUValue {
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

impl ByteWidth for CPUValue {
    fn width() -> u32 {
        1 + u64::width()
    }
}

impl Value for CPUValue {
    type Type = CPUType;

    fn type_of(&self) -> Self::Type {
        CPUValue::type_of(self)
    }
}

impl CPUValue {
    pub fn get_function_ref(&self) -> Result<u32> {
        if let CPUValue::FunctionRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected FunctionRef, found {:?}", self))
        }
    }

    pub fn get_heap_ref(&self) -> Result<u32> {
        if let CPUValue::HeapRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected HeapRef, found {:?}", self))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Parse)]
#[syntax_class(type)]
pub enum CPUType {
    #[pattern = "`undef`"]
    Undefined,

    #[pattern = "`str`"]
    String,

    #[pattern = "`bool`"]
    Bool,

    #[pattern = "`i64`"]
    I64,

    #[pattern = "`u32`"]
    U32,

    #[pattern = "`u64`"]
    U64,

    #[pattern = "`f64`"]
    F64,

    #[pattern = "`@` `func_ref`"]
    FunctionRef,

    #[pattern = "`@` `heap_ref`"]
    HeapRef,
}

impl AsRef<CPUType> for CPUType {
    fn as_ref(&self) -> &CPUType {
        self
    }
}

impl ByteWidth for CPUType {
    fn width() -> u32 {
        1
    }
}

impl Type for CPUType {}

impl CPUValue {
    pub fn is_undefined(&self) -> bool {
        matches!(self, CPUValue::Undefined)
    }

    pub fn type_of(&self) -> CPUType {
        match self {
            CPUValue::Undefined => CPUType::Undefined,
            CPUValue::String(_) => CPUType::String,
            CPUValue::Bool(_) => CPUType::Bool,
            CPUValue::I64(_) => CPUType::I64,
            CPUValue::U32(_) => CPUType::U32,
            CPUValue::U64(_) => CPUType::U64,
            CPUValue::F64(_) => CPUType::F64,
            CPUValue::FunctionRef(_) => CPUType::FunctionRef,
            CPUValue::HeapRef(_) => CPUType::HeapRef,
        }
    }

    pub fn cast(&self, to: impl AsRef<CPUType>) -> Result<CPUValue> {
        match (self, to.as_ref()) {
            (CPUValue::I64(v), CPUType::U64) => Ok(CPUValue::U64(*v as u64)),
            (CPUValue::I64(v), CPUType::F64) => Ok(CPUValue::F64(*v as f64)),
            (CPUValue::U32(v), CPUType::I64) => Ok(CPUValue::I64(*v as i64)),
            (CPUValue::U32(v), CPUType::U64) => Ok(CPUValue::U64(*v as u64)),
            (CPUValue::U32(v), CPUType::F64) => Ok(CPUValue::F64(*v as f64)),
            (CPUValue::U64(v), CPUType::I64) => Ok(CPUValue::I64(*v as i64)),
            (CPUValue::U64(v), CPUType::F64) => Ok(CPUValue::F64(*v as f64)),
            (CPUValue::F64(v), CPUType::I64) => Ok(CPUValue::I64(*v as i64)),
            (CPUValue::F64(v), CPUType::U64) => Ok(CPUValue::U64(*v as u64)),
            _ if self.type_of() == *to.as_ref() => Ok(*self),
            _ => Err(eyre::eyre!(
                "Cannot cast value {:?} to type {:?}",
                self,
                to.as_ref()
            )),
        }
    }
}

impl std::fmt::Display for CPUValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CPUValue::Undefined => write!(f, "undefined"),
            CPUValue::String(s) => write!(f, "0x{:X}", s),
            CPUValue::Bool(b) => b.fmt(f),
            CPUValue::I64(i) => i.fmt(f),
            CPUValue::U32(u) => u.fmt(f),
            CPUValue::U64(u) => u.fmt(f),
            CPUValue::F64(fl) => fl.fmt(f),
            CPUValue::FunctionRef(id) => write!(f, "<fn {}>", id),
            CPUValue::HeapRef(id) => write!(f, "<heap {}>", id),
        }
    }
}

impl std::fmt::Display for CPUType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CPUType::Undefined => write!(f, "undefined"),
            CPUType::String => write!(f, "str"),
            CPUType::Bool => write!(f, "bool"),
            CPUType::I64 => write!(f, "i64"),
            CPUType::U32 => write!(f, "u32"),
            CPUType::U64 => write!(f, "u64"),
            CPUType::F64 => write!(f, "f64"),
            CPUType::FunctionRef => write!(f, "fn"),
            CPUType::HeapRef => write!(f, "heap"),
        }
    }
}

macro_rules! impl_from_for_value {
    ($t:ty => $variant:ident) => {
        impl From<$t> for CPUValue {
            fn from(v: $t) -> Self {
                CPUValue::$variant(v)
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
        impl TryFrom<CPUValue> for $t {
            type Error = eyre::Report;
            fn try_from(v: CPUValue) -> Result<Self, Self::Error> {
                if let CPUValue::$variant(v) = v {
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

impl crate::traits::FromBytes for CPUType {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let type_byte = bytes.read_u8()?;
        match type_byte {
            0x00 => Ok(CPUType::Undefined),
            0x01 => Ok(CPUType::String),
            0x02 => Ok(CPUType::Bool),
            0x03 => Ok(CPUType::I64),
            0x04 => Ok(CPUType::U32),
            0x05 => Ok(CPUType::U64),
            0x06 => Ok(CPUType::F64),
            0x07 => Ok(CPUType::FunctionRef),
            0x08 => Ok(CPUType::HeapRef),
            _ => Err(eyre::eyre!("Unknown type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for CPUType {
    fn from_text(text: &str) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        type_text_parser()
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(format_value_parse_errors)
    }
}

impl crate::traits::WriteBytes for CPUType {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        let type_byte = match self {
            CPUType::Undefined => 0x00,
            CPUType::String => 0x01,
            CPUType::Bool => 0x02,
            CPUType::I64 => 0x03,
            CPUType::U32 => 0x04,
            CPUType::U64 => 0x05,
            CPUType::F64 => 0x06,
            CPUType::FunctionRef => 0x07,
            CPUType::HeapRef => 0x08,
        };
        io.write_u8(type_byte)?;
        Ok(())
    }
}

impl crate::traits::FromBytes for CPUValue {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let type_byte = bytes.read_u8()?;
        match type_byte {
            0x00 => Ok(CPUValue::Undefined),
            0x01 => {
                let str_id = bytes.read_u32::<LittleEndian>()?;
                Ok(CPUValue::String(str_id))
            }
            0x02 => {
                let b = bytes.read_u8()?;
                Ok(CPUValue::Bool(b != 0))
            }
            0x03 => {
                let i = bytes.read_i64::<LittleEndian>()?;
                Ok(CPUValue::I64(i))
            }
            0x04 => {
                let u = bytes.read_u32::<LittleEndian>()?;
                Ok(CPUValue::U32(u))
            }
            0x05 => {
                let u = bytes.read_u64::<LittleEndian>()?;
                Ok(CPUValue::U64(u))
            }
            0x06 => {
                let f = bytes.read_f64::<LittleEndian>()?;
                Ok(CPUValue::F64(f))
            }
            0x07 => {
                let addr = bytes.read_u32::<LittleEndian>()?;
                Ok(CPUValue::FunctionRef(addr))
            }
            0x08 => {
                let addr = bytes.read_u32::<LittleEndian>()?;
                Ok(CPUValue::HeapRef(addr))
            }
            _ => Err(eyre::eyre!("Unknown value type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for CPUValue {
    fn from_text(text: &str) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        value_text_parser()
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(format_value_parse_errors)
    }
}

impl crate::traits::WriteBytes for CPUValue {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        match self {
            CPUValue::Undefined => {
                io.write_u8(0x00)?;
            }
            CPUValue::String(s) => {
                io.write_u8(0x01)?;
                io.write_all(&s.to_le_bytes())?;
            }
            CPUValue::Bool(b) => {
                io.write_u8(0x02)?;
                io.write_u8(if *b { 1 } else { 0 })?;
            }
            CPUValue::I64(i) => {
                io.write_u8(0x03)?;
                io.write_all(&i.to_le_bytes())?;
            }
            CPUValue::U32(u) => {
                io.write_u8(0x04)?;
                io.write_all(&u.to_le_bytes())?;
            }
            CPUValue::U64(u) => {
                io.write_u8(0x05)?;
                io.write_all(&u.to_le_bytes())?;
            }
            CPUValue::F64(f) => {
                io.write_u8(0x06)?;
                io.write_all(&f.to_le_bytes())?;
            }
            CPUValue::FunctionRef(addr) => {
                io.write_u8(0x07)?;
                io.write_all(&addr.to_le_bytes())?;
            }
            CPUValue::HeapRef(addr) => {
                io.write_u8(0x08)?;
                io.write_all(&addr.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

fn type_text_parser<'src>() -> impl Parser<'src, &'src str, CPUType, ValueParseExtra<'src>> + Clone
{
    choice((
        just("undefined").to(CPUType::Undefined),
        just("string").to(CPUType::String),
        just("str").to(CPUType::String),
        just("bool").to(CPUType::Bool),
        just("i64").to(CPUType::I64),
        just("u32").to(CPUType::U32),
        just("u64").to(CPUType::U64),
        just("f64").to(CPUType::F64),
        just("fn").to(CPUType::FunctionRef),
        just("heap").to(CPUType::HeapRef),
    ))
}

fn value_text_parser<'src>() -> impl Parser<'src, &'src str, CPUValue, ValueParseExtra<'src>> + Clone
{
    let ws = one_of(" \t").repeated().at_least(1).ignored();
    choice((
        just("undefined").to(CPUValue::Undefined),
        just("string")
            .or(just("str"))
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(CPUValue::String),
        just("bool")
            .ignore_then(ws)
            .ignore_then(choice((just("true").to(true), just("false").to(false))))
            .map(CPUValue::Bool),
        just("i64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<i64>())
            .map(CPUValue::I64),
        just("u32")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(CPUValue::U32),
        just("u64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u64>())
            .map(CPUValue::U64),
        just("f64")
            .ignore_then(ws)
            .ignore_then(scalar_text::<f64>())
            .map(CPUValue::F64),
        just("fn")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(CPUValue::FunctionRef),
        just("heap")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(CPUValue::HeapRef),
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

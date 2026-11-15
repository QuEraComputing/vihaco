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
    /// a signed 32-bit integer
    I32(i32),
    /// an unsigned 32-bit integer
    U32(u32),
    /// an unsigned 64-bit integer
    U64(u64),
    /// a 64-bit floating point number
    F64(f64),
    /// a 32-bit floating point number
    F32(f32),
    /// a reference to a function by its index in the function table
    FunctionRef(u32),
    /// a heap reference: an opaque 64-bit key issued by the heap
    HeapRef(u64),
}

impl Value {
    pub fn get_function_ref(&self) -> Result<u32> {
        if let Value::FunctionRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected FunctionRef, found {:?}", self))
        }
    }

    pub fn get_heap_ref(&self) -> Result<u64> {
        if let Value::HeapRef(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre::eyre!("Expected HeapRef, found {:?}", self))
        }
    }
}

/// `Debug` shows the source spelling, as `Display` does (`struct 0`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Undefined,
    String,
    Bool,
    I64,
    I32,
    U32,
    U64,
    F64,
    F32,
    FunctionRef,
    HeapRef,
    /// A heap reference to a struct, by index into the module's struct table.
    /// At runtime the value is an ordinary heap reference.
    Struct(u32),
    /// A heap reference to an array whose elements all have the given type.
    /// Written `array <elem>`, e.g. `array f64` or `array array u64`.
    Array(Box<Type>),
    /// A named type, by index into the module's named-type table. Written
    /// `type N`. Named types are nominal: at runtime a value is the same word
    /// as the table entry's scalar representation, but for verification it
    /// satisfies only itself.
    Named(u32),
}

impl Type {
    /// The deepest array nesting accepted when a type is decoded from bytecode
    /// or parsed from text: `array` may be applied at most this many times.
    ///
    /// Untrusted input could otherwise nest types deeply enough to overflow the
    /// stack while decoding, comparing, or dropping them.
    pub const MAX_ARRAY_DEPTH: usize = 32;

    /// An array type with elements of type `elem`.
    pub fn array(elem: Type) -> Type {
        Type::Array(Box::new(elem))
    }

    /// How many `array` levels wrap this type (`0` for a non-array type).
    pub fn array_depth(&self) -> usize {
        let mut depth = 0;
        let mut ty = self;
        while let Type::Array(elem) = ty {
            depth += 1;
            ty = elem;
        }
        depth
    }

    /// Wrap `elem` in `depth` array levels, failing past [`Type::MAX_ARRAY_DEPTH`].
    pub fn nested_array(depth: usize, elem: Type) -> Result<Type> {
        let total = depth + elem.array_depth();
        eyre::ensure!(total <= Self::MAX_ARRAY_DEPTH, Self::nesting_message(total));
        Ok((0..depth).fold(elem, |ty, _| Type::array(ty)))
    }

    /// Explain a failed type parse as too-deep nesting, if it is one.
    ///
    /// The type parsers reject a type nested past [`Type::MAX_ARRAY_DEPTH`]
    /// with a position-only parse error, which reads like truncated input.
    /// Pass the source text and the start offset of a parse error's span: if
    /// the text there is a run of more than `MAX_ARRAY_DEPTH` `array`
    /// prefixes, this returns the message to report instead.
    ///
    /// ```
    /// # use vihaco_abi::Type;
    /// let text = format!("{}i64", "array ".repeat(40));
    /// assert_eq!(
    ///     Type::nesting_error_at(&text, 0).unwrap(),
    ///     "array type nested 40 levels deep; at most 32 are allowed",
    /// );
    /// assert_eq!(Type::nesting_error_at("array array i64", 0), None);
    /// ```
    pub fn nesting_error_at(text: &str, start: usize) -> Option<String> {
        let mut rest = text.get(start..)?.trim_start();
        let mut depth = 0usize;
        while let Some(after) = rest.strip_prefix("array") {
            let trimmed = after.trim_start();
            if trimmed.len() == after.len() {
                break;
            }
            depth += 1;
            rest = trimmed;
        }
        (depth > Self::MAX_ARRAY_DEPTH).then(|| Self::nesting_message(depth))
    }

    /// Explain a failed type parse as too-deep nesting, wherever it is.
    ///
    /// Like [`Type::nesting_error_at`], but scans the whole `text` for a run
    /// of more than [`Type::MAX_ARRAY_DEPTH`] `array` prefixes instead of
    /// looking only where a parse error starts. Use it as the fallback when
    /// no error position matches: a parser can report the error at an
    /// enclosing token (the `(` of a return list, say) rather than at the run.
    ///
    /// ```
    /// # use vihaco_abi::Type;
    /// let text = format!("fn @main() -> (u64, {}i64) {{ }}", "array ".repeat(33));
    /// assert_eq!(
    ///     Type::nesting_error_in(&text).unwrap(),
    ///     "array type nested 33 levels deep; at most 32 are allowed",
    /// );
    /// assert_eq!(Type::nesting_error_in("(array array i64, array i64)"), None);
    /// ```
    pub fn nesting_error_in(text: &str) -> Option<String> {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let (mut run, mut deepest) = (0usize, 0usize);
        let mut rest = text;
        while let Some(first) = rest.chars().next() {
            let word_len = rest.find(|c: char| !is_word(c)).unwrap_or(rest.len());
            if word_len == 0 {
                // Whitespace keeps a run of prefixes going; punctuation ends it.
                if !first.is_whitespace() {
                    run = 0;
                }
                rest = &rest[first.len_utf8()..];
                continue;
            }
            let (word, after) = rest.split_at(word_len);
            if word == "array" && after.starts_with(char::is_whitespace) {
                run += 1;
                deepest = deepest.max(run);
            } else {
                run = 0;
            }
            rest = after;
        }
        (deepest > Self::MAX_ARRAY_DEPTH).then(|| Self::nesting_message(deepest))
    }

    fn nesting_message(depth: usize) -> String {
        format!(
            "array type nested {depth} levels deep; at most {} are allowed",
            Self::MAX_ARRAY_DEPTH
        )
    }

    /// Visit every struct-table index this type refers to, including struct
    /// types nested inside array element types.
    pub fn visit_struct_refs(&self, visit: &mut dyn FnMut(u32)) {
        match self {
            Type::Struct(index) => visit(*index),
            Type::Array(elem) => elem.visit_struct_refs(visit),
            _ => {}
        }
    }

    /// Visit every named-type-table index this type refers to, including named
    /// types nested inside array element types.
    pub fn visit_named_refs(&self, visit: &mut dyn FnMut(u32)) {
        match self {
            Type::Named(index) => visit(*index),
            Type::Array(elem) => elem.visit_named_refs(visit),
            _ => {}
        }
    }

    /// Whether this is a scalar word type that may represent a named type:
    /// `i32`, `i64`, `u32`, `u64`, `f32`, `f64` or `bool`.
    pub fn is_scalar_word(&self) -> bool {
        matches!(
            self,
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::F32 | Type::F64 | Type::Bool
        )
    }
}

/// A function type: parameter and result types, without names.
///
/// Written `(u64, f64) -> u64`: parameters in parentheses, then `->` and the
/// results, as one type or a parenthesized list (`-> ()` for none,
/// `-> (u64, u64)` for several). `call_indirect` carries one as its expected
/// signature, which the runtime compares with the target function's declared
/// signature before calling.
/// `Debug` shows the source spelling, as `Display` does (`(u64) -> f64`).
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct FunctionType {
    pub params: Vec<Type>,
    pub results: Vec<Type>,
}

impl FunctionType {
    /// A function type with the given parameter and result types.
    pub fn new(params: impl Into<Vec<Type>>, results: impl Into<Vec<Type>>) -> Self {
        Self {
            params: params.into(),
            results: results.into(),
        }
    }
}

impl std::fmt::Display for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::names::write_function_type(f, self, None)
    }
}

impl<'src> vihaco_parser::Parse<'src> for FunctionType {
    fn parser() -> impl Parser<'src, &'src str, Self, ValueParseExtra<'src>> {
        function_type_text_parser()
    }
}

impl crate::traits::FromText for FunctionType {
    fn from_text(text: &str) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        function_type_text_parser()
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(|errors| format_value_parse_errors(text, errors))
    }
}

/// Encoded as the parameter count (`u32` LE), the parameter types, the result
/// count, and the result types.
impl crate::traits::FromBytes for FunctionType {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let list = |bytes: &mut R| -> eyre::Result<Vec<Type>> {
            let count = bytes.read_u32::<LittleEndian>()?;
            // Grow as types decode, so a corrupt count cannot force a huge allocation.
            let mut types = Vec::new();
            for _ in 0..count {
                types.push(<Type as crate::traits::FromBytes>::from_bytes(bytes)?);
            }
            Ok(types)
        };
        let params = list(bytes)?;
        let results = list(bytes)?;
        Ok(Self { params, results })
    }
}

impl crate::traits::WriteBytes for FunctionType {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        for types in [&self.params, &self.results] {
            io.write_u32::<LittleEndian>(u32::try_from(types.len())?)?;
            for ty in types {
                crate::traits::WriteBytes::write_bytes(ty, io)?;
            }
        }
        Ok(())
    }
}

/// Parse type syntax directly into the runtime type vocabulary.
impl<'src> vihaco_parser::Parse<'src> for Type {
    fn parser() -> impl Parser<'src, &'src str, Self, ValueParseExtra<'src>> {
        type_text_parser()
    }
}

/// Typed metadata constants use the existing SST value vocabulary.
impl<'src> vihaco_parser::Parse<'src> for Value {
    fn parser() -> impl Parser<'src, &'src str, Self, ValueParseExtra<'src>> {
        value_text_parser()
    }
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
            Value::I32(_) => Type::I32,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::F64(_) => Type::F64,
            Value::F32(_) => Type::F32,
            Value::FunctionRef(_) => Type::FunctionRef,
            Value::HeapRef(_) => Type::HeapRef,
        }
    }

    pub fn cast(&self, to: impl AsRef<Type>) -> Result<Value> {
        match (self, to.as_ref()) {
            (Value::I64(v), Type::U64) => Ok(Value::U64(*v as u64)),
            (Value::I64(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::I32(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::I32(v), Type::U64) => Ok(Value::U64(*v as u64)),
            (Value::I32(v), Type::F32) => Ok(Value::F32(*v as f32)),
            (Value::I32(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::U32(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::U32(v), Type::U64) => Ok(Value::U64(*v as u64)),
            (Value::U32(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::U64(v), Type::I64) => Ok(Value::I64(*v as i64)),
            (Value::U64(v), Type::F64) => Ok(Value::F64(*v as f64)),
            (Value::F32(v), Type::F64) => Ok(Value::F64(*v as f64)),
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
            Value::I32(i) => i.fmt(f),
            Value::U32(u) => u.fmt(f),
            Value::U64(u) => u.fmt(f),
            Value::F64(fl) => fl.fmt(f),
            Value::F32(fl) => fl.fmt(f),
            Value::FunctionRef(id) => write!(f, "<fn {}>", id),
            Value::HeapRef(id) => write!(f, "<heap {}>", id),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::names::write_type(f, self, None)
    }
}

impl std::fmt::Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::names::write_type(f, self, None)
    }
}

impl std::fmt::Debug for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::names::write_function_type(f, self, None)
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
impl_from_for_value!(i32 => I32);
impl_from_for_value!(u32 => U32);
impl_from_for_value!(u64 => U64);
impl_from_for_value!(f64 => F64);
impl_from_for_value!(f32 => F32);

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
impl_try_from_for_rust!(I32 => i32);
impl_try_from_for_rust!(U32 => u32);
impl_try_from_for_rust!(U64 => u64);
impl_try_from_for_rust!(F64 => f64);
impl_try_from_for_rust!(F32 => f32);

impl crate::traits::FromBytes for Type {
    fn from_bytes<R: std::io::Read>(bytes: &mut R) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        // Array element types are read iteratively, so a long run of array
        // bytes is rejected instead of recursing once per level.
        let mut depth = 0;
        loop {
            let type_byte = bytes.read_u8()?;
            if type_byte != 0x0C {
                return Type::nested_array(depth, Type::from_scalar_byte(type_byte, bytes)?);
            }
            depth += 1;
            eyre::ensure!(
                depth <= Type::MAX_ARRAY_DEPTH,
                "array type nested more than {} levels deep",
                Type::MAX_ARRAY_DEPTH
            );
        }
    }
}

impl Type {
    /// Decode a non-array type from its type byte (and payload).
    fn from_scalar_byte<R: std::io::Read>(type_byte: u8, bytes: &mut R) -> Result<Type> {
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
            0x09 => Ok(Type::I32),
            0x0A => Ok(Type::F32),
            0x0B => Ok(Type::Struct(bytes.read_u32::<LittleEndian>()?)),
            0x0D => Ok(Type::Named(bytes.read_u32::<LittleEndian>()?)),
            _ => Err(eyre::eyre!("Unknown type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for Type {
    fn from_text(text: &str) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        type_text_parser()
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(|errors| format_value_parse_errors(text, errors))
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
            Type::I32 => 0x09,
            Type::F32 => 0x0A,
            Type::Struct(_) => 0x0B,
            Type::Array(_) => 0x0C,
            Type::Named(_) => 0x0D,
        };
        io.write_u8(type_byte)?;
        match self {
            Type::Struct(index) | Type::Named(index) => io.write_u32::<LittleEndian>(*index)?,
            Type::Array(elem) => elem.write_bytes(io)?,
            _ => {}
        }
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
                let addr = bytes.read_u64::<LittleEndian>()?;
                Ok(Value::HeapRef(addr))
            }
            0x09 => {
                let i = bytes.read_i32::<LittleEndian>()?;
                Ok(Value::I32(i))
            }
            0x0A => {
                let f = bytes.read_f32::<LittleEndian>()?;
                Ok(Value::F32(f))
            }
            _ => Err(eyre::eyre!("Unknown value type byte: {}", type_byte)),
        }
    }
}

impl crate::traits::FromText for Value {
    fn from_text(text: &str) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        value_text_parser()
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(|errors| format_value_parse_errors(text, errors))
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
            Value::I32(i) => {
                io.write_u8(0x09)?;
                io.write_all(&i.to_le_bytes())?;
            }
            Value::F32(f) => {
                io.write_u8(0x0A)?;
                io.write_all(&f.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

/// Parse any number of `array ` prefixes (without recursion) and return how
/// many there were. Callers wrap their element type with [`Type::nested_array`],
/// which enforces [`Type::MAX_ARRAY_DEPTH`].
fn array_prefixes<'src>() -> impl Parser<'src, &'src str, usize, ValueParseExtra<'src>> + Clone {
    just("array")
        .then(text::whitespace().at_least(1))
        .repeated()
        .count()
}

fn type_text_parser<'src>() -> impl Parser<'src, &'src str, Type, ValueParseExtra<'src>> + Clone {
    array_prefixes()
        .then(element_type_text_parser())
        .try_map(|(depth, elem), span| {
            Type::nested_array(depth, elem).map_err(|_| Simple::new(None, span))
        })
}

/// A type that is not itself written with an `array` prefix.
fn element_type_text_parser<'src>()
-> impl Parser<'src, &'src str, Type, ValueParseExtra<'src>> + Clone {
    {
        // `struct` must precede `str`, which would otherwise match its prefix.
        let struct_type = just("struct")
            .then_ignore(text::whitespace().at_least(1))
            .ignore_then(scalar_text::<u32>())
            .map(Type::Struct);
        let named_type = just("type")
            .then_ignore(text::whitespace().at_least(1))
            .ignore_then(scalar_text::<u32>())
            .map(Type::Named);
        choice((
            struct_type,
            named_type,
            just("undefined").to(Type::Undefined),
            just("undef").to(Type::Undefined),
            just("string").to(Type::String),
            just("str").to(Type::String),
            just("bool").to(Type::Bool),
            just("i64").to(Type::I64),
            just("i32").to(Type::I32),
            just("u32").to(Type::U32),
            just("u64").to(Type::U64),
            just("f64").to(Type::F64),
            just("f32").to(Type::F32),
            just("fn_ref").to(Type::FunctionRef),
            just("fn").to(Type::FunctionRef),
            just("heap_ref").to(Type::HeapRef),
            just("heap").to(Type::HeapRef),
        ))
    }
}

fn function_type_text_parser<'src>()
-> impl Parser<'src, &'src str, FunctionType, ValueParseExtra<'src>> + Clone {
    let ws = one_of(" \t").repeated();
    let list = type_text_parser()
        .separated_by(just(',').padded_by(ws))
        .collect::<Vec<_>>()
        .padded_by(ws)
        .delimited_by(just('('), just(')'));
    let results = list.clone().or(type_text_parser().map(|ty| vec![ty]));
    list.then_ignore(just("->").padded_by(ws))
        .then(results)
        .map(|(params, results)| FunctionType { params, results })
}

fn value_text_parser<'src>() -> impl Parser<'src, &'src str, Value, ValueParseExtra<'src>> + Clone {
    let bits = just("f64_bits")
        .then_ignore(text::whitespace().at_least(1))
        .ignore_then(scalar_text::<u64>())
        .map(|bits| Value::F64(f64::from_bits(bits)))
        .or(just("f32_bits")
            .then_ignore(text::whitespace().at_least(1))
            .ignore_then(scalar_text::<u32>())
            .map(|bits| Value::F32(f32::from_bits(bits))));

    let ws = one_of(" \t").repeated().at_least(1).ignored();
    choice((
        bits,
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
        just("i32")
            .ignore_then(ws)
            .ignore_then(scalar_text::<i32>())
            .map(Value::I32),
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
        just("f32")
            .ignore_then(ws)
            .ignore_then(scalar_text::<f32>())
            .map(Value::F32),
        just("fn")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u32>())
            .map(Value::FunctionRef),
        just("heap")
            .ignore_then(ws)
            .ignore_then(scalar_text::<u64>())
            .map(Value::HeapRef),
    ))
}

fn scalar_text<'src, T>() -> impl Parser<'src, &'src str, T, ValueParseExtra<'src>> + Clone
where
    T: std::str::FromStr,
{
    any()
        .filter(|ch: &char| !ch.is_whitespace() && !",;{}[]()/".contains(*ch))
        .repeated()
        .at_least(1)
        .to_slice()
        .try_map(|text: &str, span| text.parse::<T>().map_err(|_| Simple::new(None, span)))
}

fn format_value_parse_errors(text: &str, errors: Vec<Simple<'_, char>>) -> eyre::Report {
    let error = errors
        .into_iter()
        .next()
        .map(|error| {
            Type::nesting_error_at(text, error.span().start)
                .or_else(|| Type::nesting_error_in(text))
                .unwrap_or_else(|| format!("{error:?}"))
        })
        .unwrap_or_else(|| "unknown parse error".to_string());
    eyre::eyre!("failed to parse value text: {error}")
}

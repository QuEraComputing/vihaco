// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{
    io::{Cursor, Read},
    sync::Arc,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    module::{FunctionInfo, LabelInfo, Parameter, Signature, SourceSymbolInfo},
    traits::FromBytes,
    value::{Type, Value},
};

use super::format::ConstantId;

/// The global context for a given program.
///
/// This should include all context needed for an entire section tree.
/// Anything that should be shared across machines should be in a
/// [`BytecodeContext`].
pub trait BytecodeContext: Sized {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self>;

    fn section_name(&self, index: u32) -> Option<&str>;
}

pub trait ProgramGlobals {
    type Type;
    type Value;

    fn get_function(&self, index: usize) -> eyre::Result<FunctionInfo<Self::Type>>;
    fn get_string(&self, index: usize) -> eyre::Result<&String>;
    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value>;
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProgramContext<V = Value, Ty = Type> {
    pub constants: Vec<V>,
    pub strings: Vec<String>,
    pub functions: Vec<FunctionInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub main_function: Option<u32>,
    pub file: u32,
    pub source_symbols: Vec<SourceSymbolInfo>,
}

impl<V, Ty> ProgramContext<V, Ty>
where
    V: FromBytes,
    Ty: FromBytes,
{
    pub fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let context = Self::read_from(&mut cursor)?;
        if cursor.position() as usize != bytes.len() {
            return Err(eyre::eyre!(
                "program context has {} trailing bytes",
                bytes.len() - cursor.position() as usize
            ));
        }
        Ok(context)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        let constants = read_vec(reader, "constant", V::from_bytes)?;
        let strings = read_vec(reader, "string", read_string)?;
        let functions = read_vec(reader, "function", read_function_info)?;
        let labels = read_vec(reader, "label", read_label_info)?;
        let main_function = read_optional_u32(reader)?;
        let file = reader.read_u32::<LittleEndian>()?;
        let source_symbols = read_vec(reader, "source symbol", read_source_symbol_info)?;

        Ok(Self {
            constants,
            strings,
            functions,
            labels,
            main_function,
            file,
            source_symbols,
        })
    }
}

impl<V, Ty> BytecodeContext for ProgramContext<V, Ty>
where
    V: FromBytes,
    Ty: FromBytes,
{
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        ProgramContext::from_bytes(bytes)
    }

    fn section_name(&self, index: u32) -> Option<&str> {
        self.strings.get(index as usize).map(String::as_str)
    }
}

impl<V, Ty> ProgramGlobals for ProgramContext<V, Ty>
where
    Ty: Clone,
{
    type Type = Ty;
    type Value = V;

    fn get_function(&self, index: usize) -> eyre::Result<FunctionInfo<Self::Type>> {
        self.functions.get(index).cloned().ok_or_else(|| {
            eyre::eyre!(format!(
                "function index out of bounds: {} (max {})",
                index,
                self.functions.len()
            ))
        })
    }

    fn get_string(&self, index: usize) -> eyre::Result<&String> {
        self.strings.get(index).ok_or_else(|| {
            eyre::eyre!(format!(
                "string index out of bounds: {} (max {})",
                index,
                self.strings.len()
            ))
        })
    }

    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value> {
        self.constants.get(id.0 as usize).ok_or_else(|| {
            eyre::eyre!(format!(
                "constant index out of bounds: {} (max {})",
                id.0,
                self.constants.len()
            ))
        })
    }
}

/// The public handle for a bytecode context.
///
/// To avoid needing explicit lifetimes permeating throughout
/// machine definitions, we wrap the context in an [`Arc`] to drop it
/// automatically.
#[derive(Debug)]
pub struct ContextHandle<C:  = ProgramContext>(Arc<C>);

impl<C> Clone for ContextHandle<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C> ContextHandle<C> {
    pub fn new(context: C) -> Self {
        Self(Arc::new(context))
    }

    pub fn get(&self) -> &C {
        &self.0
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<C> std::ops::Deref for ContextHandle<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

fn read_vec<R, T, F>(reader: &mut R, label: &str, mut read_item: F) -> eyre::Result<Vec<T>>
where
    R: Read,
    F: FnMut(&mut R) -> eyre::Result<T>,
{
    let count = reader.read_u32::<LittleEndian>()? as usize;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(
            read_item(reader)
                .map_err(|err| eyre::eyre!("failed to read {label} table entry {index}: {err}"))?,
        );
    }
    Ok(values)
}

fn read_string<R: Read>(reader: &mut R) -> eyre::Result<String> {
    let len = reader.read_u32::<LittleEndian>()? as usize;
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn read_function_info<R, Ty>(reader: &mut R) -> eyre::Result<FunctionInfo<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let name = reader.read_u32::<LittleEndian>()?;
    let signature = read_signature(reader)?;
    let local_count = reader.read_u32::<LittleEndian>()?;
    let start_address = reader.read_u32::<LittleEndian>()?;
    let end_address = reader.read_u32::<LittleEndian>()?;
    let file = reader.read_u32::<LittleEndian>()?;

    Ok(FunctionInfo {
        name,
        signature,
        local_count,
        start_address,
        end_address,
        file,
    })
}

fn read_signature<R, Ty>(reader: &mut R) -> eyre::Result<Signature<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let params = read_vec(reader, "parameter", read_parameter)?;
    let ret = read_vec(reader, "return type", Ty::from_bytes)?;
    Ok(Signature { params, ret })
}

fn read_parameter<R, Ty>(reader: &mut R) -> eyre::Result<Parameter<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let name = reader.read_u32::<LittleEndian>()?;
    let ty = Ty::from_bytes(reader)?;
    Ok(Parameter { name, ty })
}

fn read_label_info<R: Read>(reader: &mut R) -> eyre::Result<LabelInfo> {
    let address = reader.read_u32::<LittleEndian>()?;
    let name = reader.read_u32::<LittleEndian>()?;
    Ok(LabelInfo { address, name })
}

fn read_source_symbol_info<R: Read>(reader: &mut R) -> eyre::Result<SourceSymbolInfo> {
    let index = reader.read_u32::<LittleEndian>()?;
    let name = read_string(reader)?;
    Ok(SourceSymbolInfo { index, name })
}

fn read_optional_u32<R: Read>(reader: &mut R) -> eyre::Result<Option<u32>> {
    match reader.read_u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.read_u32::<LittleEndian>()?)),
        other => Err(eyre::eyre!(
            "invalid optional u32 discriminant {} in program context",
            other
        )),
    }
}

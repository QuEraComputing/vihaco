// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Constant encoding and string interning used during resolution.

use eyre::{Result, ensure, eyre};
use std::collections::BTreeMap;
use vihaco::Type;
use vihaco_cpu::{SurfaceType, SurfaceValue, Word, word::*};

use super::Resolver;

impl Resolver {
    pub(super) fn intern(&mut self, value: &str) -> Result<u32> {
        if let Some(index) = self.strings.iter().position(|item| item == value) {
            return Ok(u32::try_from(index)?);
        }
        let index = u32::try_from(self.strings.len())?;
        self.strings.push(value.to_owned());
        Ok(index)
    }

    pub(super) fn value(
        &mut self,
        ty: SurfaceType,
        value: SurfaceValue,
        functions: &BTreeMap<String, u32>,
    ) -> Result<Word> {
        let text = match value {
            SurfaceValue::Bare(value) => value.0,
            SurfaceValue::Quoted(value) => {
                ensure!(
                    ty == SurfaceType::String,
                    "quoted constant requires string type"
                );
                value.as_str().to_owned()
            }
        };
        Ok(match ty {
            SurfaceType::Undefined => return Err(eyre!("undefined constant")),
            SurfaceType::String => encode_string_id(self.intern(&text)?),
            SurfaceType::Bool => encode_bool(text.parse()?),
            SurfaceType::I32 => encode_i32(text.parse()?),
            SurfaceType::I64 => encode_i64(text.parse()?),
            SurfaceType::U32 => encode_u32(text.parse()?),
            SurfaceType::U64 => encode_u64(text.parse()?),
            SurfaceType::F32 => encode_f32(text.parse()?),
            SurfaceType::F64 => encode_f64(text.parse()?),
            SurfaceType::FunctionRef => {
                let index = if let Some(name) = text.strip_prefix('@') {
                    *functions
                        .get(name)
                        .ok_or_else(|| eyre!("unknown function @{name}"))?
                } else {
                    text.parse()?
                };
                ensure!(
                    (index as usize) < functions.len(),
                    "function reference out of bounds"
                );
                encode_function_ref(index)
            }
            SurfaceType::HeapRef => encode_heap_ref(text.parse()?),
        })
    }
}

pub(super) fn runtime_type(ty: SurfaceType) -> Type {
    match ty {
        SurfaceType::Undefined => Type::Undefined,
        SurfaceType::String => Type::String,
        SurfaceType::Bool => Type::Bool,
        SurfaceType::I32 => Type::I32,
        SurfaceType::I64 => Type::I64,
        SurfaceType::U32 => Type::U32,
        SurfaceType::U64 => Type::U64,
        SurfaceType::F32 => Type::F32,
        SurfaceType::F64 => Type::F64,
        SurfaceType::FunctionRef => Type::FunctionRef,
        SurfaceType::HeapRef => Type::HeapRef,
    }
}

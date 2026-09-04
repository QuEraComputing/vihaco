// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Exhaustive surface-to-runtime instruction conversion.
//! Keep the opcode mapping together so CPU ISA additions cause compile errors.

use eyre::{Result, ensure, eyre};
use std::collections::BTreeMap;
use vihaco_cpu::{RuntimeInstruction as R, SurfaceInstruction as S, SurfaceType};

use super::Resolver;

impl Resolver {
    pub(super) fn lower(
        &mut self,
        instruction: S,
        labels: &BTreeMap<String, u32>,
        functions: &BTreeMap<String, u32>,
        starts: &BTreeMap<String, (u32, u32)>,
    ) -> Result<R> {
        let label = |name: &str| {
            labels
                .get(name)
                .copied()
                .ok_or_else(|| eyre!("unknown label @{name}"))
        };
        // Exhaustive conversion keeps the adapter aligned with the CPU ISA.
        // These variants have identical payloads in surface/runtime form.
        macro_rules! lower {
            (units [$($unit:ident),*], locals [$($local:ident),*], constants [$($constant:ident => $ty:ident),*]) => {
                match instruction {
                    $(S::$unit => R::$unit,)*
                    $(S::$local(index) => R::$local(index),)*
                    $(S::$constant(value) => R::$constant(self.value(SurfaceType::$ty, value, functions)?),)*
                    S::Span(file, start, end) => R::Span(file, start, end),
                    S::Label(name) => R::Label(name),
                    S::HeapAlloc(count) => {
                        eyre::ensure!(count <= 65_536, "benchmark heap allocation exceeds size limit");
                        R::HeapAlloc(count)
                    },
                    S::Return(count) => R::Return(count),
                    S::Branch(name) => R::Branch(label(name.as_str())?),
                    S::ConditionalBranch(yes, no) => R::ConditionalBranch(label(yes.as_str())?, label(no.as_str())?),
                    S::Call(arity, name) => {
                        let &(start, params) = starts.get(name.as_str()).ok_or_else(|| eyre!("unknown function @{name}"))?;
                        ensure!(arity == params, "call arity mismatch for @{name}");
                        R::Call(arity, start)
                    }
                }
            };
        }
        Ok(lower!(
            units [FunctionStart, FunctionEnd, Breakpoint, IndirectCall, Halt, Print,
                Dup, GetItem, HeapDealloc,
                AddI32, AddI64, AddU32, AddU64, AddF32, AddF64,
                SubI32, SubI64, SubU32, SubU64, SubF32, SubF64,
                MulI32, MulI64, MulU32, MulU64, MulF32, MulF64,
                DivI32, DivI64, DivU32, DivU64, DivF32, DivF64,
                RemI32, RemI64, RemU32, RemU64, RemF32, RemF64,
                NegI32, NegI64, NegF32, NegF64,
                ShlI32, ShlI64, ShlU32, ShlU64, ShrI32, ShrI64, ShrU32, ShrU64,
                RolI32, RolI64, RolU32, RolU64, RorI32, RorI64, RorU32, RorU64,
                BitAndI32, BitAndI64, BitAndU32, BitAndU64,
                BitOrI32, BitOrI64, BitOrU32, BitOrU64,
                BitXorI32, BitXorI64, BitXorU32, BitXorU64,
                Not, And, Or, Xor,
                EqI32, EqI64, EqU32, EqU64, EqF32, EqF64,
                NeI32, NeI64, NeU32, NeU64, NeF32, NeF64,
                LtI32, LtI64, LtU32, LtU64, LtF32, LtF64,
                GtI32, GtI64, GtU32, GtU64, GtF32, GtF64,
                LeI32, LeI64, LeU32, LeU64, LeF32, LeF64,
                GeI32, GeI64, GeU32, GeU64, GeF32, GeF64],
            locals [LoadI32, LoadI64, LoadU32, LoadU64, LoadF32, LoadF64, LoadBool,
                StoreI32, StoreI64, StoreU32, StoreU64, StoreF32, StoreF64, StoreBool],
            constants [ConstI32 => I32, ConstI64 => I64, ConstU32 => U32, ConstU64 => U64,
                ConstF32 => F32, ConstF64 => F64, ConstBool => Bool, ConstString => String,
                ConstFunctionRef => FunctionRef, ConstHeapRef => HeapRef]
        ))
    }
}

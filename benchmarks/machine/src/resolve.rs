// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Machine-specific lowering after vihaco has parsed a complete SST section.

use std::collections::{BTreeMap, BTreeSet};

use eyre::{Result, ensure, eyre};
use vihaco::{
    FromText, SstHeader, Type,
    module::{FunctionInfo, LabelInfo, LocalModule, Parameter, Signature},
    syntax::{ParsedFunction, ParsedModule, Resolve},
};
use vihaco_cpu::{SurfaceInstruction as S, SurfaceType, Word};

use crate::machine::fixture;

mod instructions;
mod values;
use values::runtime_type;

type FunctionIndices = BTreeMap<String, u32>;
type FunctionEntries = BTreeMap<String, (u32, u32)>;

type Syntax = fixture::syntax::Instruction;
pub(crate) type Module = LocalModule<fixture::runtime::Instruction, Word, Type>;

pub(crate) struct EmptyHeader;

impl FromText for EmptyHeader {
    fn from_text(text: &str) -> Result<Self> {
        ensure!(
            text.trim().is_empty(),
            "benchmark section header must be empty"
        );
        Ok(Self)
    }
}

impl SstHeader for EmptyHeader {}

#[derive(Default)]
pub(crate) struct Resolver {
    strings: Vec<String>,
}

impl Resolve<Syntax, SurfaceType, EmptyHeader> for Resolver {
    type Module = Module;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<Syntax, SurfaceType, EmptyHeader>,
    ) -> Result<Module> {
        self.strings.clear();
        let (functions, starts) = function_index(&parsed)?;
        let mut module = Module {
            main_function: functions.get("main").copied(),
            ..Module::default()
        };
        ensure!(module.main_function.is_some(), "missing @main function");
        for function in parsed.functions {
            self.resolve_function(function, &functions, &starts, &mut module)?;
        }
        module.strings = std::mem::take(&mut self.strings);
        Ok(module)
    }
}

/// First pass assigns all function indices and entry addresses, allowing
/// forward calls. Indices are used for function references; addresses for PCs.
fn function_index(
    parsed: &ParsedModule<Syntax, SurfaceType, EmptyHeader>,
) -> Result<(FunctionIndices, FunctionEntries)> {
    let mut functions = BTreeMap::new();
    let mut starts = BTreeMap::new();
    let mut address = 0_u32;
    for (index, function) in parsed.functions.iter().enumerate() {
        let name = function.name.as_str().to_owned();
        ensure!(
            functions
                .insert(name.clone(), u32::try_from(index)?)
                .is_none(),
            "duplicate function @{name}"
        );
        ensure!(!function.body.is_empty(), "empty function @{name}");
        starts.insert(name, (address, u32::try_from(function.params.len())?));
        address = address
            .checked_add(u32::try_from(function.body.len())?)
            .ok_or_else(|| eyre!("program too large"))?;
    }

    Ok((functions, starts))
}

impl Resolver {
    /// Second pass scopes labels and locals to a function before lowering its body.
    fn resolve_function(
        &mut self,
        function: ParsedFunction<Syntax, SurfaceType>,
        functions: &FunctionIndices,
        starts: &FunctionEntries,
        module: &mut Module,
    ) -> Result<()> {
        let name = function.name.as_str();
        let start = u32::try_from(module.code.len())?;
        let FunctionScope {
            labels,
            mut local_count,
        } = self.function_scope(&function, start, module)?;
        let signature = self.signature(&function)?;
        if name == "main" {
            ensure!(
                signature.params.len() == 2
                    && signature.params.iter().all(|p| p.ty == Type::U64)
                    && signature.ret == [Type::U64],
                "benchmark entry must be @main(iterations: u64, seed: u64) -> u64"
            );
            // Preserve the fixture's two scratch locals in addition to inputs.
            local_count = local_count.max(4);
        }
        for Syntax::Cpu(inst) in function.body {
            module.code.push(fixture::runtime::Instruction::Cpu(
                self.lower(inst, &labels, functions, starts)?,
            ));
        }
        module.functions.push(FunctionInfo {
            name: self.intern(name)?,
            signature,
            local_count,
            start_address: start,
            end_address: u32::try_from(module.code.len())?,
            file: 0,
        });
        Ok(())
    }
}

/// Labels are local to a function, even when several functions reuse a name.
struct FunctionScope {
    labels: BTreeMap<String, u32>,
    local_count: u32,
}

impl Resolver {
    fn function_scope(
        &mut self,
        function: &ParsedFunction<Syntax, SurfaceType>,
        start: u32,
        module: &mut Module,
    ) -> Result<FunctionScope> {
        let name = function.name.as_str();
        let mut labels = BTreeMap::new();
        let mut local_count = u32::try_from(function.params.len())?;
        for (offset, Syntax::Cpu(inst)) in function.body.iter().enumerate() {
            if let S::Label(label) = inst {
                let address = start + u32::try_from(offset)?;
                ensure!(
                    labels.insert(label.as_str().to_owned(), address).is_none(),
                    "duplicate label @{label} in @{name}"
                );
                module.labels.push(LabelInfo {
                    address,
                    name: self.intern(&format!("{name}::{}", label.as_str()))?,
                });
            }
            match inst {
                S::LoadI32(i)
                | S::LoadI64(i)
                | S::LoadU32(i)
                | S::LoadU64(i)
                | S::LoadF32(i)
                | S::LoadF64(i)
                | S::LoadBool(i)
                | S::StoreI32(i)
                | S::StoreI64(i)
                | S::StoreU32(i)
                | S::StoreU64(i)
                | S::StoreF32(i)
                | S::StoreF64(i)
                | S::StoreBool(i) => {
                    local_count = local_count.max(
                        i.checked_add(1)
                            .ok_or_else(|| eyre!("local index too large"))?,
                    );
                }
                _ => {}
            }
        }
        ensure!(
            local_count <= 4096,
            "benchmark function exceeds local slot limit"
        );
        Ok(FunctionScope {
            labels,
            local_count,
        })
    }

    fn signature(
        &mut self,
        function: &ParsedFunction<Syntax, SurfaceType>,
    ) -> Result<Signature<Type>> {
        let name = function.name.as_str();
        let mut params = Vec::new();
        let mut param_names = BTreeSet::new();
        for parameter in &function.params {
            ensure!(
                param_names.insert(parameter.name.as_str().to_owned()),
                "duplicate parameter in @{name}"
            );
            params.push(Parameter {
                name: self.intern(parameter.name.as_str())?,
                ty: runtime_type(parameter.ty),
            });
        }
        let ret: Vec<_> = function.return_ty.into_iter().map(runtime_type).collect();
        Ok(Signature { params, ret })
    }
}

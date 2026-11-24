// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard program instruction dialect and legacy section data.

mod dialect;

pub use dialect::program;

mod literal;

pub mod lower;

pub mod operand;

pub mod raise;

use vihaco_abi::{Type, Value};
use vihaco_bytecode::{ConstantId, ContextHandle};
use vihaco_module::{
    host::{GetProgramInfo, ProgramCounter},
    module::{LocalModule, NoInfo},
};

/// A loaded section, its shared context, and current program counter.
#[derive(Debug, Clone)]
pub struct ProgramImage<I, C, V = Value, Ty = Type, Info = NoInfo> {
    pub module: LocalModule<I, V, Ty, Info>,
    pub context: Option<ContextHandle<C>>,
    pub pc: u32,
}

impl<I, C, V, Ty, Info: Default> Default for ProgramImage<I, C, V, Ty, Info> {
    fn default() -> Self {
        Self {
            module: LocalModule::default(),
            context: None,
            pc: 0,
        }
    }
}

impl<I, C, V, Ty, Info> ProgramImage<I, C, V, Ty, Info> {
    pub fn new() -> Self
    where
        Info: Default,
    {
        Self::default()
    }

    pub fn with_extra(extra: Info) -> Self {
        Self {
            module: LocalModule {
                code: Vec::new(),
                functions: Vec::new(),
                symbols: Vec::new(),
                constants: Vec::new(),
                strings: Vec::new(),
                main_function: None,
                source_symbols: Vec::new(),
                extra,
            },
            context: None,
            pc: 0,
        }
    }

    pub fn context(&self) -> eyre::Result<&C> {
        self.context
            .as_ref()
            .map(ContextHandle::get)
            .ok_or_else(|| eyre::eyre!("program image has not been loaded with a context"))
    }
}

impl<I, C, V, Ty, Info> ProgramCounter for ProgramImage<I, C, V, Ty, Info> {
    type Instruction = I;

    fn pc(&self) -> u32 {
        self.pc
    }

    fn pc_mut(&mut self) -> &mut u32 {
        &mut self.pc
    }

    fn get_instruction(&self, pc: u32) -> eyre::Result<&Self::Instruction> {
        self.module.code.get(pc as usize).ok_or_else(|| {
            eyre::eyre!(format!(
                "program counter out of bounds: {} (max {})",
                pc,
                self.module.code.len()
            ))
        })
    }
}

impl<I, C, V, Ty, Info> GetProgramInfo for ProgramImage<I, C, V, Ty, Info>
where
    Ty: Clone,
{
    type Type = Ty;
    type Value = V;

    fn get_function(
        &self,
        index: usize,
    ) -> eyre::Result<vihaco_module::module::FunctionInfo<Self::Type>> {
        self.module.functions.get(index).cloned().ok_or_else(|| {
            eyre::eyre!(format!(
                "function index out of bounds: {} (max {})",
                index,
                self.module.functions.len()
            ))
        })
    }

    fn get_string(&self, index: usize) -> eyre::Result<&String> {
        self.module.strings.get(index).ok_or_else(|| {
            eyre::eyre!(format!(
                "string index out of bounds: {} (max {})",
                index,
                self.module.strings.len()
            ))
        })
    }

    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value> {
        self.module.constants.get(id.0 as usize).ok_or_else(|| {
            eyre::eyre!(format!(
                "constant index out of bounds: {} (max {})",
                id.0,
                self.module.constants.len()
            ))
        })
    }
}

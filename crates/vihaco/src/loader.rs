// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{
    BytecodeContext, BytecodeFile,
    binary::{
        ConstantId, ContextHandle, FileContents, SectionView, decode_instruction_stream,
        parse_instruction_stream,
    },
    module::{Module, NoInfo},
    program::{ProgramContext, ProgramGlobals, Type, Value},
    traits::{self, GetProgramGlobal, ProgramCounter},
};

/// The input given to a specific loadable machine.
pub struct LoadInput<'bc, F = Vec<u8>, C = ProgramContext>
where
    F: FileContents,
    C: BytecodeContext,
{
    pub section: SectionView<'bc, F, C>,
}

impl<'bc, F, C> Clone for LoadInput<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    fn clone(&self) -> Self {
        LoadInput {
            section: self.section.clone(),
        }
    }
}

impl<'bc, F, C> From<&'bc BytecodeFile<F, C>> for LoadInput<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    fn from(file: &'bc BytecodeFile<F, C>) -> Self {
        Self {
            section: file.root(),
        }
    }
}

impl<'bc, F, C> From<SectionView<'bc, F, C>> for LoadInput<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
{
    fn from(section: SectionView<'bc, F, C>) -> Self {
        Self { section }
    }
}

impl<'bc, F, C> std::fmt::Debug for LoadInput<'bc, F, C>
where
    F: FileContents,
    C: BytecodeContext,
    SectionView<'bc, F, C>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadInput")
            .field("section", &self.section)
            .finish_non_exhaustive()
    }
}

/// Allow a machine to load a section.
///
/// When used with the [`vihaco_derive::composite`] macro, any field marked
/// `#[program]` will automatically have the section routed to it;
/// that field should implement the logic for loading a section.
pub trait LoadSection<F = Vec<u8>, C = ProgramContext>
where
    F: FileContents,
    C: BytecodeContext,
{
    fn load_section<'bc>(&mut self, input: LoadInput<'bc, F, C>) -> eyre::Result<()>;
}

#[derive(Debug, Clone)]
pub struct ModuleProgramLoader<I, V = Value, Ty = Type, Info = NoInfo> {
    pub module: Module<I, V, Ty, Info>,
    pub pc: u32,
}

impl<I, V, Ty, Info: Default> Default for ModuleProgramLoader<I, V, Ty, Info> {
    fn default() -> Self {
        Self {
            module: Module::default(),
            pc: 0,
        }
    }
}

impl<I: traits::Instruction, V, Ty, Info> ProgramCounter for ModuleProgramLoader<I, V, Ty, Info> {
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

impl<I: traits::Instruction, V, Ty, Info> GetProgramGlobal for ModuleProgramLoader<I, V, Ty, Info>
where
    Ty: Clone,
{
    type Type = Ty;
    type Value = V;

    fn get_function(&self, index: usize) -> eyre::Result<crate::module::FunctionInfo<Self::Type>> {
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

#[derive(Debug, Clone)]
pub struct ProgramLoader<I, C = ProgramContext, Info = NoInfo> {
    pub code: Vec<I>,
    pub context: Option<ContextHandle<C>>,
    pub pc: u32,
    pub extra: Info,
}

impl<I, C, Info: Default> Default for ProgramLoader<I, C, Info> {
    fn default() -> Self {
        Self {
            code: Vec::new(),
            context: None,
            pc: 0,
            extra: Info::default(),
        }
    }
}

impl<I, C, Info> ProgramLoader<I, C, Info> {
    pub fn new() -> Self
    where
        Info: Default,
    {
        Self::default()
    }

    pub fn with_extra(extra: Info) -> Self {
        Self {
            code: Vec::new(),
            context: None,
            pc: 0,
            extra,
        }
    }

    pub fn context(&self) -> eyre::Result<&C> {
        self.context
            .as_ref()
            .map(ContextHandle::get)
            .ok_or_else(|| eyre::eyre!("bytecode program loader has not been loaded"))
    }
}

impl<I, C, Info> LoadSection<Vec<u8>, C> for ProgramLoader<I, C, Info>
where
    I: traits::Instruction,
    C: BytecodeContext,
{
    fn load_section<'bc>(&mut self, input: LoadInput<'bc, Vec<u8>, C>) -> eyre::Result<()> {
        self.code = decode_instruction_stream(input.section.bytecode())?;
        self.context = Some(input.section.context_handle());
        self.pc = 0;
        Ok(())
    }
}

impl<I, C, Info> LoadSection<String, C> for ProgramLoader<I, C, Info>
where
    I: traits::Instruction,
    C: BytecodeContext,
    for<'src> I: vihaco_parser_core::Parse<'src>,
{
    fn load_section<'bc>(&mut self, input: LoadInput<'bc, String, C>) -> eyre::Result<()> {
        self.code = parse_instruction_stream(input.section.text())?;
        self.context = Some(input.section.context_handle());
        self.pc = 0;
        Ok(())
    }
}

impl<I: traits::Instruction, C, Info> ProgramCounter for ProgramLoader<I, C, Info> {
    type Instruction = I;

    fn pc(&self) -> u32 {
        self.pc
    }

    fn pc_mut(&mut self) -> &mut u32 {
        &mut self.pc
    }

    fn get_instruction(&self, pc: u32) -> eyre::Result<&Self::Instruction> {
        self.code.get(pc as usize).ok_or_else(|| {
            eyre::eyre!(format!(
                "program counter out of bounds: {} (max {})",
                pc,
                self.code.len()
            ))
        })
    }
}

impl<I, C, Info> GetProgramGlobal for ProgramLoader<I, C, Info>
where
    C: ProgramGlobals,
{
    type Type = C::Type;
    type Value = C::Value;

    fn get_function(&self, index: usize) -> eyre::Result<crate::module::FunctionInfo<Self::Type>> {
        self.context()?.get_function(index)
    }

    fn get_string(&self, index: usize) -> eyre::Result<&String> {
        self.context()?.get_string(index)
    }

    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value> {
        self.context()?.get_constant(id)
    }
}

use crate::{
    module::{Module, NoInfo},
    traits::{self, GetProgramGlobal, ProgramCounter},
    value::{Type, Value},
};

#[derive(Debug, Clone)]
pub struct ProgramLoader<I, Info = NoInfo> {
    pub module: Module<I, Value, Type, Info>,
    pub pc: u32,
}

impl<I, Info: Default> Default for ProgramLoader<I, Info> {
    fn default() -> Self {
        Self {
            module: Module::default(),
            pc: 0,
        }
    }
}

impl<I: traits::Instruction, Info> ProgramCounter for ProgramLoader<I, Info> {
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

impl<I: traits::Instruction, Info> GetProgramGlobal for ProgramLoader<I, Info> {
    type Type = Type;

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
}

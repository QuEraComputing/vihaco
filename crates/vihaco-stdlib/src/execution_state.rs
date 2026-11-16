// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked invocation state and execution of an independently owned module.
//!
//! The host owns program data. `ExecutionState` owns frames, locals, operands,
//! the private PC, and execution flags. A runner borrows both the module and the
//! machine; instructions borrow the module while authored Steps mutate devices.
//! Component authors choose messages/effects, without implementing stack logic.
//!
//! ```
//! use vihaco_abi::{Type, Value};
//! use vihaco_module::module::{LocalModule, FunctionInfo, Signature};
//! use vihaco_runtime::{composite, field, Apply, BuildMessage, Execute, HasInstructionSet, Step};
//! use vihaco_stdlib::{ExecutionState, Image, execution_state::execute, program::program};
//! use vihaco_cpu::{CPU, cpu};
//!
//! #[composite]
//! struct Machine {
//!     #[device(0)] #[program] execution: vihaco_stdlib::ExecutionState,
//!     #[device(1)] cpu: vihaco_cpu::CPU,
//! }
//! type Instruction = <Machine as HasInstructionSet>::Runtime;
//! type Program = LocalModule<Instruction, Value, Type>;
//!
//! impl<I> Step<field!(Machine::execution), I, Program> for Machine
//! where
//!     ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
//!     Self: BuildMessage<field!(Machine::execution), I, Program>,
//! {
//!     fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
//!         let message = <Self as BuildMessage<field!(Machine::execution), I, Program>>
//!             ::build_message(self, instruction, module)?;
//!         self.execution.execute(instruction, message)
//!     }
//! }
//! impl<I> Step<field!(Machine::cpu), I, Program> for Machine
//! where
//!     CPU: Execute<I, Error = eyre::Report>,
//!     Self: BuildMessage<field!(Machine::cpu), I, Program>,
//!     ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
//! {
//!     fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
//!         let message = <Self as BuildMessage<field!(Machine::cpu), I, Program>>
//!             ::build_message(self, instruction, module)?;
//!         let effect = self.cpu.execute(instruction, message)?;
//!         self.execution.apply(effect);
//!         Ok(())
//!     }
//! }
//!
//! let module: Program = LocalModule {
//!     code: vec![
//!         Instruction::V_execution__program__ConstU64(program::ConstU64(20)),
//!         Instruction::V_execution__program__ConstU64(program::ConstU64(22)),
//!         Instruction::V_cpu__cpu__AddU64(cpu::AddU64),
//!         Instruction::V_execution__program__Return(program::Return(1)),
//!     ],
//!     main_function: Some(0),
//!     functions: vec![FunctionInfo {
//!         name: 0, signature: Signature { params: vec![], ret: vec![Type::U64] },
//!         local_count: 0, start_address: 0, end_address: 4, file: 0,
//!     }],
//!     ..Default::default()
//! };
//! // An ordinary outer owner keeps immutable program data beside its machine.
//! struct Loaded {
//!     image: Image<Instruction>,
//!     machine: Machine,
//! }
//! impl Loaded {
//!     fn run(&mut self) -> eyre::Result<()> {
//!         execute(&self.image.module, &mut self.machine, &[])
//!     }
//! }
//! let mut loaded = Loaded {
//!     image: Image::new(module),
//!     machine: Machine { execution: ExecutionState::new(), cpu: CPU::default() },
//! };
//! loaded.run()?;
//! assert_eq!(loaded.machine.execution.stack(), &[42]);
//! # Ok::<(), eyre::Report>(())
//! ```
use crate::program::program;
use vihaco_abi::{frame::Frame, word::Word};
use vihaco_module::module::LocalModule;

mod execution;
mod messages;
pub(crate) mod runner;
mod storage;

pub use runner::{execute, resume};

/// No custom execution state is available. By using this for the
/// default in [`RunState`], we can get exhaustive pattern matching
/// for `Runnable` and `Halted` by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoCustomState {}

/// Whether an invocation can execute its next instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState<S = NoCustomState> {
    Runnable,
    Halted,
    Other(S),
}

#[vihaco_runtime::component]
#[instructions { program }]
/// Frames, locals, operands, and private execution state for one invocation.
///
/// The module is supplied by reference at entry and is never stored here.
#[derive(Debug)]
pub struct ExecutionState<S = NoCustomState> {
    stack: Vec<Word>,
    frames: Vec<Frame>,
    pc: u32,
    run_state: RunState<S>,
    executing: bool,
}

impl<S> Default for ExecutionState<S> {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            pc: 0,
            run_state: RunState::Halted,
            executing: false,
        }
    }
}

impl ExecutionState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S: Clone> Clone for ExecutionState<S> {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            frames: self.frames.clone(),
            pc: self.pc,
            run_state: self.run_state.clone(),
            executing: false,
        }
    }
}

#[cold]
#[inline(never)]
fn instruction_out_of_bounds(pc: u32, len: usize) -> eyre::Report {
    eyre::eyre!("instruction index out of bounds: {} (max {})", pc, len)
}

fn function_entry<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    index: usize,
) -> eyre::Result<(usize, usize, u32)> {
    let function = module
        .functions
        .get(index)
        .ok_or_else(|| unknown_function(index))?;
    if (function.local_count as usize) < function.signature.params.len() {
        return Err(invalid_local_count());
    }
    if (function.start_address as usize) >= module.code.len() {
        return Err(invalid_call_target());
    }
    Ok((
        function.signature.params.len(),
        function.local_count as usize,
        function.start_address,
    ))
}

#[cold]
#[inline(never)]
fn unknown_function(index: usize) -> eyre::Report {
    eyre::eyre!("unknown function reference {index}")
}

#[cold]
#[inline(never)]
fn invalid_local_count() -> eyre::Report {
    eyre::eyre!("local count must be at least arity")
}

#[cold]
#[inline(never)]
fn invalid_call_target() -> eyre::Report {
    eyre::eyre!("call target out of bounds")
}

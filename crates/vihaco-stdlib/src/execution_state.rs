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
//! use vihaco_stdlib::{ExecutionState, Image, Verified, execution_state::execute, program::program};
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
//!     structs: vec![],
//!     types: vec![],
//!     functions: vec![FunctionInfo {
//!         name: 0, signature: Signature { params: vec![], ret: vec![Type::U64] },
//!         locals: vec![], start_address: 0, end_address: 4, file: 0,
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
//!         execute(self.image.module(), &mut self.machine, &[])
//!     }
//! }
//! let mut loaded = Loaded {
//!     image: Image::new(Verified::new(module)?),
//!     machine: Machine { execution: ExecutionState::new(), cpu: CPU::default() },
//! };
//! loaded.run()?;
//! assert_eq!(loaded.machine.execution.stack(), &[42]);
//! # Ok::<(), eyre::Report>(())
//! ```
use crate::program::program;
use vihaco_abi::{frame::Frame, word::Word};
use vihaco_module::module::{DEFAULT_MAX_CALL_DEPTH, LocalModule, MAX_LOCALS};

mod execution;
mod messages;
mod runner;
mod storage;

pub use runner::{StepError, execute, resume};

#[vihaco_runtime::component]
#[instructions { program }]
/// Frames, locals, operands, and private execution state for one invocation.
///
/// The module is supplied by reference at entry and is never stored here.
#[derive(Debug)]
pub struct ExecutionState {
    stack: Vec<Word>,
    frames: Vec<Frame>,
    pc: u32,
    halted: bool,
    executing: bool,
    max_call_depth: usize,
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            pc: 0,
            halted: true,
            executing: false,
            max_call_depth: DEFAULT_MAX_CALL_DEPTH,
        }
    }
}

impl ExecutionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The most call frames (the entry function's included) an invocation
    /// may have at once; [`DEFAULT_MAX_CALL_DEPTH`] unless changed.
    pub fn max_call_depth(&self) -> usize {
        self.max_call_depth
    }

    /// Limit nested call frames to `depth`. A call that would exceed it fails
    /// with an error naming the callee, before any frame storage is reserved.
    pub fn set_max_call_depth(&mut self, depth: usize) {
        self.max_call_depth = depth;
    }

    /// [`Self::set_max_call_depth`], by value.
    pub fn with_max_call_depth(mut self, depth: usize) -> Self {
        self.set_max_call_depth(depth);
        self
    }

    /// Fail if entering `function` would exceed the call-depth limit.
    fn check_call_depth<I, V, Ty, Info>(
        &self,
        module: &LocalModule<I, V, Ty, Info>,
        function: usize,
    ) -> eyre::Result<()> {
        if self.frames.len() >= self.max_call_depth {
            return Err(call_too_deep(module, function, self.max_call_depth));
        }
        Ok(())
    }
}

impl Clone for ExecutionState {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            frames: self.frames.clone(),
            pc: self.pc,
            halted: self.halted,
            executing: false,
            max_call_depth: self.max_call_depth,
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
    let locals = function.local_count();
    // Modules can be built without `ProgramMetadata::validate`, so re-check
    // the frame bound before any caller reserves or resizes storage.
    if locals > MAX_LOCALS {
        return Err(too_many_locals(module, index, locals));
    }
    if locals < function.signature.params.len() {
        return Err(invalid_local_count());
    }
    if (function.start_address as usize) >= module.code.len() {
        return Err(invalid_call_target());
    }
    Ok((
        function.signature.params.len(),
        locals,
        function.start_address,
    ))
}

#[cold]
#[inline(never)]
fn unknown_function(index: usize) -> eyre::Report {
    eyre::eyre!("unknown function reference {index}")
}

/// `@name (function N)`, or `function N` if the name is not in the string table.
fn describe_function<I, V, Ty, Info>(module: &LocalModule<I, V, Ty, Info>, index: usize) -> String {
    let name = module
        .functions
        .get(index)
        .and_then(|function| module.strings.get(function.name as usize));
    match name {
        Some(name) => format!("`@{name}` (function {index})"),
        None => format!("function {index}"),
    }
}

#[cold]
#[inline(never)]
fn too_many_locals<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    index: usize,
    locals: usize,
) -> eyre::Report {
    eyre::eyre!(
        "{} declares {locals} locals, more than the maximum of {MAX_LOCALS}",
        describe_function(module, index)
    )
}

#[cold]
#[inline(never)]
fn call_too_deep<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    index: usize,
    limit: usize,
) -> eyre::Report {
    eyre::eyre!(
        "call to {} exceeds the maximum call depth of {limit} frames",
        describe_function(module, index)
    )
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

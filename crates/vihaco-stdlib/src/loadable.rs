// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! An independently owned image paired with its mutable execution machine.

use vihaco_abi::{traits::Reset, word::Word};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Composite, Dispatch, HasProgram};

use crate::{ExecutionState, Image, execution_state};

/// A program image and the composite that executes its borrowed instructions.
///
/// Construct the image explicitly before pairing it with a machine. Only
/// [`Image`] is supported; the image remains unchanged throughout an invocation.
/// Ordinary outer structs can own several loadables with different instruction
/// sets. Section selection and image construction remain caller responsibilities.
#[derive(Debug)]
pub struct Loadable<I, M: Composite> {
    image: I,
    machine: M,
}

impl<Inst, V, Ty, Info, M: Composite> Loadable<Image<Inst, V, Ty, Info>, M> {
    /// Inspect the paired image without changing an invocation's program data.
    pub fn image(&self) -> &Image<Inst, V, Ty, Info> {
        &self.image
    }

    /// Inspect the execution machine and its devices.
    pub fn machine(&self) -> &M {
        &self.machine
    }

    /// Configure the execution machine or its devices.
    ///
    /// Direct changes to execution state can affect whether an invocation can
    /// resume. The caller is responsible for keeping those changes consistent
    /// with the paired image.
    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.machine
    }

    /// Consume the owner, returning its image and machine without resetting them.
    pub fn into_parts(self) -> (Image<Inst, V, Ty, Info>, M) {
        (self.image, self.machine)
    }
}

impl<Inst, V, Ty, Info, M> Loadable<Image<Inst, V, Ty, Info>, M>
where
    M: Composite + HasProgram<Program = ExecutionState>,
{
    /// Pair an image with a machine, resetting its previous invocation state.
    ///
    /// Device state is retained. No parsing, lowering, or image validation is
    /// performed here; construction of a valid image is the caller's job.
    pub fn new(image: Image<Inst, V, Ty, Info>, mut machine: M) -> Self {
        machine.program_mut().reset();
        Self { image, machine }
    }

    /// Install an already constructed image and reset execution state.
    ///
    /// Device state is retained. Image construction happens before this call;
    /// mutations made by the caller while constructing it are not rolled back.
    ///
    /// # Errors
    ///
    /// Rejects replacement while an invocation is active, including one that
    /// could resume after an execution error. Call [`Self::reset`] to abandon
    /// that invocation first. Rejection leaves the current image and execution
    /// state unchanged.
    pub fn replace_image(&mut self, image: Image<Inst, V, Ty, Info>) -> eyre::Result<()> {
        eyre::ensure!(
            self.machine.program().is_halted(),
            "cannot replace an active program"
        );
        self.image = image;
        self.reset();
        Ok(())
    }

    /// Abandon the invocation, retaining the image and all device state.
    pub fn reset(&mut self) {
        self.machine.program_mut().reset();
    }
}

impl<Inst, V, Ty, Info, M> Loadable<Image<Inst, V, Ty, Info>, M>
where
    M: HasProgram<Program = ExecutionState>
        + Dispatch<LocalModule<Inst, V, Ty, Info>, Instruction = Inst>,
{
    /// Start at the program entrypoint with the supplied arguments.
    ///
    /// Instructions and metadata borrow the image while dispatch mutates the
    /// separate machine. No instruction cloning is required.
    ///
    /// # Errors
    ///
    /// Returns the runner's entry or execution errors. Earlier machine mutations
    /// are retained; reset or resume according to the invocation's state.
    pub fn execute(&mut self, arguments: &[Word]) -> eyre::Result<()> {
        execution_state::execute(&self.image.module, &mut self.machine, arguments)
    }

    /// Continue the existing invocation against the same paired image.
    ///
    /// # Errors
    ///
    /// Returns the runner's state, instruction-fetch, or dispatch errors without
    /// rolling back machine mutations. A halted invocation remains halted.
    pub fn resume(&mut self) -> eyre::Result<()> {
        execution_state::resume(&self.image.module, &mut self.machine)
    }
}

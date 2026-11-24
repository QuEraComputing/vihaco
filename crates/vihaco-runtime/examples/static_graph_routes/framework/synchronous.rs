// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Borrowed inline execution
//!
//! The CPU runner lends a module and current instruction to
//! `Route::run_synchronously`. The same `build_message_at` descriptor used
//! by callback routes now reads the borrowed context through `BuildMessage`.
//! `Execute` borrows the original instruction. The final CPU computation
//! applies the effect to the operand stack before the runner fetches again.
//!
//! `RunSync` interprets nodes and sequences and returns results on the call
//! stack. Its current operations are message preparation, instruction
//! execution, and computation. Metadata remains inspectable. The inline
//! interpreter has no driver hooks, waits, or fork/join.

use super::{
    graph::{Node, Route, Sequence},
    operations::{BuildAt, Compute, ComputeNode, ExecuteAt, Prepared},
};
use eyre::Result;
use vihaco_runtime::{BuildMessage, Execute, Field};

/// Interprets a static path inline and returns its result to the caller.
///
/// The input can borrow the program's current instruction and context.
pub(crate) trait RunSync<M, Input> {
    /// The result passed directly to the next path.
    type Output;

    /// Applies the path and returns its terminal result.
    fn run_sync(machine: &mut M, input: Input) -> Result<Self::Output>;
}

/// Defines the direct-call execution contract for an individual node.
pub(crate) trait SyncOperation<M, Input> {
    /// The output of this operation.
    type Output;

    /// Consumes the input and returns the output inline.
    fn apply(machine: &mut M, input: Input) -> Result<Self::Output>;
}

/// The same static node can have an inline interpretation for borrowed inputs.
impl<M, Input, N: SyncOperation<M, Input>, A> RunSync<M, Input> for Node<N, A> {
    type Output = N::Output;

    fn run_sync(machine: &mut M, input: Input) -> Result<Self::Output> {
        N::apply(machine, input)
    }
}

/// An error ends the sequence at the operation that produced it.
impl<M, Input, L, E, R> RunSync<M, Input> for Sequence<L, E, R>
where
    L: RunSync<M, Input>,
    R: RunSync<M, L::Output>,
{
    type Output = R::Output;

    fn run_sync(machine: &mut M, input: Input) -> Result<Self::Output> {
        let output = L::run_sync(machine, input)?;
        R::run_sync(machine, output)
    }
}

impl<I, P> Route<I, P> {
    /// Runs a unit-output step using the runner's borrowed instruction and context.
    ///
    /// Returns after the path has applied its effects or reported an error.
    pub(crate) fn run_synchronously<'input, M, C: ?Sized>(
        &self,
        machine: &mut M,
        instruction: &'input I,
        context: &'input C,
    ) -> Result<()>
    where
        P: RunSync<M, (&'input I, &'input C), Output = ()>,
    {
        P::run_sync(machine, (instruction, context))
    }
}

/// The entry node reads the borrowed context and passes on the prepared message.
impl<'input, M, F, I, C: ?Sized, Message> SyncOperation<M, (&'input I, &'input C)> for BuildAt<F>
where
    M: BuildMessage<F, I, C, Message>,
    M::Child: Execute<I, Message = Message>,
{
    type Output = Prepared<&'input I, Message>;

    fn apply(
        machine: &mut M,
        (instruction, context): (&'input I, &'input C),
    ) -> Result<Self::Output> {
        let message = machine.build_message(instruction, context)?;
        Ok(Prepared {
            instruction,
            message,
        })
    }
}

/// Execute borrows the original instruction directly from the program.
impl<M, F, I, Message> SyncOperation<M, Prepared<&I, Message>> for ExecuteAt<F>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Execute<I>>::Effect;

    fn apply(machine: &mut M, input: Prepared<&I, Message>) -> Result<Self::Output> {
        machine
            .get_mut()
            .execute(input.instruction, input.message)
            .map_err(Into::into)
    }
}

/// The final inline computation can apply a CPU effect to the program stack.
impl<C: Compute> SyncOperation<C::Machine, C::Input> for ComputeNode<C> {
    type Output = C::Output;

    fn apply(machine: &mut C::Machine, input: C::Input) -> Result<Self::Output> {
        C::apply(machine, input)
    }
}

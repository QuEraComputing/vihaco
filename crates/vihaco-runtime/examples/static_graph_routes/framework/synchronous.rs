// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Borrowed inline execution
//!
//! The CPU runner lends a module and current instruction to `StepContext::start`.
//! A route finished with `finish_inline` uses the inline interpreter. Its preparation nodes
//! read the borrowed context through the recipe or `BuildMessage`.
//! `Execute` borrows the original instruction. The program field
//! applies the CPU effect to the operand stack before the runner fetches again.
//!
//! `RunSync` interprets nodes and sequences and returns results on the call
//! stack. Its current operations are message preparation, instruction
//! execution, and effect delivery. Metadata remains inspectable. The inline
//! interpreter has no driver hooks, waits, or fork/join.

use super::{
    graph::{
        Describe, InlineRoute, Node, ParkableRoute, Route, Run, Sequence, Unannotated, step_graph,
    },
    operations::{
        ApplyEffectAt, BuildAt, Entry, ExecuteAt, HandleEffect, HandleEffectBetween, Instruction,
        PrepareAt, Prepared, execute_instruction_at, prepare_message_at,
    },
};
use eyre::Result;
use vihaco_runtime::{Apply, BuildMessage, Execute, Field, ReusableMessageRecipe};

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

/// Inline execution accepts nodes that need no annotation policy from the driver.
impl<M, Input, N: SyncOperation<M, Input>, Execution> RunSync<M, Input>
    for Node<N, Unannotated, Execution>
{
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

/// Starts a route in the execution mode selected by its declaration.
pub(crate) trait StartRoute<M, Input, C: ?Sized, D> {
    fn start(
        &self,
        machine: &mut M,
        instruction: Input,
        context: &C,
        driver: &impl StartCallback<M, D>,
    ) -> Result<()>;
}

/// Driver services for a route whose callback may be retained across a wait.
pub(crate) trait StartCallback<M, D> {
    fn start_callback<I, P, Mode, Input>(
        &self,
        machine: &mut M,
        route: &Route<I, P, Mode>,
        input: Input,
    ) -> Result<()>
    where
        I: 'static,
        P: Run<M, Input, D, Output = ()> + 'static;
}

impl<'input, M, I, P, C: ?Sized, D> StartRoute<M, &'input I, C, D> for InlineRoute<I, P>
where
    P: for<'a> RunSync<M, (&'a I, &'a C), Output = ()>,
{
    fn start(
        &self,
        machine: &mut M,
        instruction: &'input I,
        context: &C,
        _: &impl StartCallback<M, D>,
    ) -> Result<()> {
        P::run_sync(machine, (instruction, context))
    }
}

impl<M, I, P, C: ?Sized, D> StartRoute<M, I, C, D> for ParkableRoute<I, P>
where
    I: 'static,
    P: for<'a> Run<M, Entry<'a, I, ()>, D, Output = ()> + 'static,
{
    fn start(
        &self,
        machine: &mut M,
        instruction: I,
        _: &C,
        driver: &impl StartCallback<M, D>,
    ) -> Result<()> {
        driver.start_callback(
            machine,
            self,
            Entry {
                instruction: &instruction,
                context: &(),
            },
        )
    }
}

impl<'input, M, I, P, C: ?Sized, D> StartRoute<M, &'input I, C, D> for Route<I, P>
where
    I: 'static,
    P: for<'a> Run<M, Entry<'a, I, C>, D, Output = ()> + 'static,
{
    fn start(
        &self,
        machine: &mut M,
        instruction: &'input I,
        context: &C,
        driver: &impl StartCallback<M, D>,
    ) -> Result<()> {
        driver.start_callback(
            machine,
            self,
            Entry {
                instruction,
                context,
            },
        )
    }
}

/// Prepares input from a recipe and executes an instruction with no outgoing effect.
pub(crate) fn recipe_instruction_route<M, F, I, C: ?Sized>()
-> InlineRoute<I, impl for<'a> RunSync<M, (&'a I, &'a C), Output = ()> + Describe>
where
    M: Field<F>,
    M::Child: Execute<I, Effect = (), Error: Into<eyre::Report>>,
    <M::Child as Execute<I>>::Message: ReusableMessageRecipe<M, F, I, C>,
{
    step_graph::<I>()
        .begin(prepare_message_at::<F, <M::Child as Execute<I>>::Message>())
        .then(execute_instruction_at::<F>())
        .finish_inline()
}

/// The entry node reads the borrowed context and passes on the prepared message.
impl<'input, M, F, I, C: ?Sized, Message> SyncOperation<M, (&'input I, &'input C)>
    for BuildAt<F, Message>
where
    M: BuildMessage<F, I, C, Message>,
{
    type Output = Prepared<'input, I, Message>;

    fn apply(
        machine: &mut M,
        (instruction, context): (&'input I, &'input C),
    ) -> Result<Self::Output> {
        let message = machine.build_message(instruction, context)?;
        Ok(Prepared {
            instruction: Instruction::Borrowed(instruction),
            message,
        })
    }
}

impl<'input, M, F, I, C: ?Sized, Message> SyncOperation<M, (&'input I, &'input C)>
    for PrepareAt<F, Message>
where
    M: Field<F>,
    Message: ReusableMessageRecipe<M, F, I, C>,
{
    type Output = Prepared<'input, I, Message>;

    fn apply(
        machine: &mut M,
        (instruction, context): (&'input I, &'input C),
    ) -> Result<Self::Output> {
        let message = Message::prepare(machine, instruction, context)?;
        Ok(Prepared {
            instruction: Instruction::Borrowed(instruction),
            message,
        })
    }
}

/// Execute borrows the original instruction directly from the program.
impl<M, F, I, Message> SyncOperation<M, Prepared<'_, I, Message>> for ExecuteAt<F>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Execute<I>>::Effect;

    fn apply(machine: &mut M, input: Prepared<'_, I, Message>) -> Result<Self::Output> {
        machine
            .get_mut()
            .execute(input.instruction.get(), input.message)
            .map_err(Into::into)
    }
}

/// Applies an effect directly to the selected field and forwards its output.
impl<M: Field<F>, F, E> SyncOperation<M, E> for ApplyEffectAt<F>
where
    M::Child: Apply<E, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Apply<E>>::Effect;

    fn apply(machine: &mut M, effect: E) -> Result<Self::Output> {
        machine.get_mut().apply(effect).map_err(Into::into)
    }
}

impl<M, Source, Destination, E> SyncOperation<M, E> for HandleEffectBetween<Source, Destination>
where
    M: HandleEffect<Source, Destination, E>,
{
    type Output = M::Effect;

    fn apply(machine: &mut M, effect: E) -> Result<Self::Output> {
        machine.handle_effect(effect, &())
    }
}

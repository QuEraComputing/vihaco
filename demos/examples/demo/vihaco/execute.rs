// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Marker message for instructions whose execution does not require a runtime-supplied message.
#[derive(Debug, Clone, Copy, Default)]
struct NoMessage;

/// Outcome of one instruction step. This is independent of any timing model. It answers whether
/// the parent may advance the program counter or must keep the composite parked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Execution {
    /// The step resolved; the parent may advance to the next instruction.
    Complete,
    /// The step is unresolved; the parent must wait for a completion.
    Parked,
}

/// The standardized result of starting or resuming one instruction route. Effects are handled
/// independently from the route's completion state.
struct StepResult<E> {
    effects: Effects<E>,
    execution: Execution,
}

/// A component executes one fully-resolved runtime instruction against its own state.
trait Execute<I> {
    type Message;
    type Effect;
    type Fault;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault>;
}

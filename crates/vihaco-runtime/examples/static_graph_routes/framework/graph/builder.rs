// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Zero-sized declarations for operations and the paths that connect them.
//!
//! `Plan` composes nodes, sequences, and forks without a machine or a driver.
//! The step builder moves from `Before` to `After` when it designates the
//! instruction site. Only `After` can finish a route. Payload and driver
//! compatibility are checked when choosing an execution interpreter.

use super::annotations::{Annotated, Annotation, Unannotated};
use crate::framework::{
    async_execution::{AsyncExecuteAt, AsyncInstructionAt},
    operations::{ExecuteAt, InstructionAt},
};
use std::marker::PhantomData;

type TypeMarker<T> = PhantomData<fn() -> T>;

/// A static connection descriptor carrying its annotation list in its type.
pub(crate) struct Edge<A>(PhantomData<fn() -> A>);

/// Creates an unannotated connection descriptor.
pub(crate) fn edge() -> Edge<Unannotated> {
    Edge(PhantomData)
}

// Adds metadata to a connection before it is attached to a path.
impl<A> Edge<A> {
    /// Attaches a metadata tag to the edge.
    pub(crate) fn annotate<T: Annotation>(self) -> Edge<Annotated<A, T>> {
        Edge(PhantomData)
    }
}

/// A zero-sized descriptor of a composable path.
pub(crate) struct Plan<P>(PhantomData<fn() -> P>);

/// Pairs an operation type with its node annotations.
pub(crate) struct Node<N, A>(PhantomData<fn() -> (N, A)>);

/// The descriptor for a single unannotated operation.
pub(crate) type NodePlan<N> = Plan<Node<N, Unannotated>>;

/// Turns an operation type into a plan that can participate in the graph.
pub(in crate::framework) fn node<N>() -> NodePlan<N> {
    Plan(PhantomData)
}

// Adds annotations to a node while preserving the operation type.
impl<N, A> Plan<Node<N, A>> {
    /// Attaches a metadata tag to the node.
    pub(crate) fn annotate<T: Annotation>(self) -> Plan<Node<N, Annotated<A, T>>> {
        Plan(PhantomData)
    }
}

/// Connects the left path’s output to the right path’s input through edge E.
pub(crate) struct Sequence<L, E, R>(TypeMarker<(L, E, R)>);

// Composes paths.
impl<L> Plan<L> {
    /// Connects the next path through an unannotated edge.
    pub(crate) fn then<R>(self, next: Plan<R>) -> Plan<Sequence<L, Unannotated, R>> {
        self.then_via(edge(), next)
    }

    /// Connects the next path through an explicitly configured edge.
    pub(crate) fn then_via<E, R>(self, _: Edge<E>, _: Plan<R>) -> Plan<Sequence<L, E, R>> {
        Plan(PhantomData)
    }
}

/// Describes a split path, two arms, and their join node.
pub(crate) struct ForkJoin<S, L, R>(TypeMarker<(S, L, R)>);

/// Builds a path that splits one input and pairs the results of its two arms.
pub(crate) fn fork_join<S, L, R>(_: Plan<S>, _: Plan<L>, _: Plan<R>) -> Plan<ForkJoin<S, L, R>> {
    Plan(PhantomData)
}

/// Marks a step that can still declare its designated instruction.
pub(crate) struct Before;

/// Marks a step whose designated instruction execution is in place.
pub(crate) struct After;

/// Selects the instruction type before the route has an entry path.
pub(crate) struct Start<I>(PhantomData<fn() -> I>);

/// Accumulates a path while tracking whether the instruction has been declared.
pub(crate) struct Step<I, P, State>(TypeMarker<(I, P, State)>);

/// The step type after appending a path through an edge.
pub(crate) type ExtendedStep<I, L, E, R, State> = Step<I, Sequence<L, E, R>, State>;

/// Starts describing a step for instruction type I.
pub(crate) fn step_graph<I>() -> Start<I> {
    Start(PhantomData)
}

// Makes the first path the root of the step.
impl<I> Start<I> {
    /// Installs the entry path before the execution node is chosen.
    pub(crate) fn begin<P>(self, _: Plan<P>) -> Step<I, P, Before> {
        Step(PhantomData)
    }
}

// Allows auxiliary paths on either side of the designated instruction.
impl<I, P, State> Step<I, P, State> {
    /// Connects the next path through an unannotated edge.
    pub(crate) fn then<R>(self, next: Plan<R>) -> ExtendedStep<I, P, Unannotated, R, State> {
        self.then_via(edge(), next)
    }

    /// Connects the next path through an explicitly configured edge.
    pub(crate) fn then_via<E, R>(self, _: Edge<E>, _: Plan<R>) -> ExtendedStep<I, P, E, R, State> {
        Step(PhantomData)
    }
}

// Inserts the designated instruction node and advances the builder state.
impl<I, P> Step<I, P, Before> {
    /// Adds the instruction execution node through an unannotated edge.
    pub(crate) fn then_execute<F, A>(
        self,
        site: Plan<Node<InstructionAt<F>, A>>,
    ) -> ExtendedStep<I, P, Unannotated, Node<ExecuteAt<F>, A>, After> {
        self.then_execute_via(edge(), site)
    }

    /// Declares a designated async instruction on the selected field.
    pub(crate) fn then_execute_async<F, A>(
        self,
        _: Plan<Node<AsyncInstructionAt<F>, A>>,
    ) -> ExtendedStep<I, P, Unannotated, Node<AsyncExecuteAt<F>, A>, After> {
        Step(PhantomData)
    }

    /// Adds the instruction execution node through the supplied edge.
    pub(crate) fn then_execute_via<E, F, A>(
        self,
        _: Edge<E>,
        _: Plan<Node<InstructionAt<F>, A>>,
    ) -> ExtendedStep<I, P, E, Node<ExecuteAt<F>, A>, After> {
        Step(PhantomData)
    }
}

// Finishes steps that have declared their instruction node.
impl<I, P> Step<I, P, After> {
    /// Produces the finished route once the execution node is present.
    pub(crate) fn finish(self) -> Route<I, P> {
        Route(PhantomData)
    }
}

/// A finished zero-sized graph that can be inspected and started repeatedly.
pub(crate) struct Route<I, P>(PhantomData<fn() -> (I, P)>);

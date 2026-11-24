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

/// Carries a type in a declaration without storing or owning a value of that type.
type TypeMarker<T> = PhantomData<fn() -> T>;

/// Describes a connection before it is attached to a path.
///
/// The type parameter `A` holds the annotation list projected onto that edge.
pub(crate) struct Edge<A>(PhantomData<fn() -> A>);

/// Creates an unannotated connection descriptor.
pub(crate) fn edge() -> Edge<Unannotated> {
    Edge(PhantomData)
}

impl<A> Edge<A> {
    /// Attaches a metadata tag to the edge.
    pub(crate) fn annotate<T: Annotation>(self) -> Edge<Annotated<A, T>> {
        Edge(PhantomData)
    }
}

/// A zero-sized value used to compose a path before finishing a step.
///
/// `P` describes the complete path and can represent an operation, a
/// sequence, or a fork/join.
pub(crate) struct Plan<P>(PhantomData<fn() -> P>);

/// Describes one operation occurrence of type `N` with annotations `A`.
///
/// Using the same node type twice in a path produces two occurrences in the
/// inspected topology.
pub(crate) struct Node<N, A>(PhantomData<fn() -> (N, A)>);

/// The descriptor for a single unannotated operation.
pub(crate) type NodePlan<N> = Plan<Node<N, Unannotated>>;

/// Turns an operation type into a plan that can participate in the graph.
pub(in crate::framework) fn node<N>() -> NodePlan<N> {
    Plan(PhantomData)
}

impl<N, A> Plan<Node<N, A>> {
    /// Attaches a metadata tag to the node.
    #[allow(dead_code, reason = "used by the checked program driver examples")]
    pub(crate) fn annotate<T: Annotation>(self) -> Plan<Node<N, Annotated<A, T>>> {
        Plan(PhantomData)
    }
}

/// Connects the output of path `L` to the input of path `R`.
///
/// `E` describes the edge's annotations. The execution interpreters check
/// that the payload types match.
pub(crate) struct Sequence<L, E, R>(TypeMarker<(L, E, R)>);

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

/// Describes a split path `S`, two arms `L` and `R`, and their synthetic join.
///
/// The split supplies an owned input to each arm. The join produces their
/// outputs as a pair in left-to-right order.
pub(crate) struct ForkJoin<S, L, R>(TypeMarker<(S, L, R)>);

/// Builds a path that splits one input and pairs the results of its two arms.
pub(crate) fn fork_join<S, L, R>(_: Plan<S>, _: Plan<L>, _: Plan<R>) -> Plan<ForkJoin<S, L, R>> {
    Plan(PhantomData)
}

/// Marks the builder state that can designate a sync or async instruction site.
pub(crate) struct Before;

/// Marks the builder state that can finish because its instruction site is set.
pub(crate) struct After;

/// Carries the selected instruction type `I` until `begin` supplies an entry path.
pub(crate) struct Start<I>(PhantomData<fn() -> I>);

/// Accumulates the path `P` for instruction `I`.
///
/// `State` controls which builder methods are available. Only a step with
/// a designated instruction site can become a finished route.
pub(crate) struct Step<I, P, State>(TypeMarker<(I, P, State)>);

/// Names the builder result after appending path `R` to `L` through edge `E`.
///
/// The caller chooses the resulting state to reflect whether the append
/// designated the instruction site.
pub(crate) type ExtendedStep<I, L, E, R, State> = Step<I, Sequence<L, E, R>, State>;

/// Starts describing a step for instruction type I.
pub(crate) fn step_graph<I>() -> Start<I> {
    Start(PhantomData)
}

impl<I> Start<I> {
    /// Installs the entry path before the execution node is chosen.
    pub(crate) fn begin<P>(self, _: Plan<P>) -> Step<I, P, Before> {
        Step(PhantomData)
    }
}

/// Appends paths in either builder state without designating an instruction site.
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

/// Selects the designated instruction site and transitions to `After`.
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

impl<I, P> Step<I, P, After> {
    /// Produces the finished route once the execution node is present.
    pub(crate) fn finish(self) -> Route<I, P> {
        Route(PhantomData)
    }
}

/// A finished zero-sized declaration for one instruction step.
///
/// The type records the instruction `I` and complete path `P`. Inspection
/// projects that path without a machine. Each execution supplies its own inputs
/// and any state needed to resume.
pub(crate) struct Route<I, P>(PhantomData<fn() -> (I, P)>);

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Zero-sized declarations for operations and the paths that connect them.
//!
//! `Plan` composes nodes, sequences, and forks without a machine or a driver.
//! Execution constructors mark their nodes. A finished step must contain
//! exactly one execution node, with none inside fork/join paths. Payload
//! and driver compatibility are checked when choosing an interpreter.

use super::{
    annotations::{Annotated, Annotation, Named, Unannotated},
    inspection::{Describe, RegisteredTopology, Topology},
};
use std::{any::TypeId, marker::PhantomData};

/// Carries a type in a declaration without storing or owning a value of that type.
type TypeMarker<T> = PhantomData<fn() -> T>;

/// Describes a connection before it is attached to a path.
///
/// The type parameter `A` holds the annotation list projected onto that edge.
pub(crate) struct Edge<A, Names = ()>(TypeMarker<A>, Names);

/// Creates an unannotated connection descriptor.
pub(crate) fn edge() -> Edge<Unannotated> {
    Edge(PhantomData, ())
}

impl<A> Edge<A> {
    /// Attaches a metadata tag to the edge.
    pub(crate) fn annotate<T: Annotation>(self) -> Edge<Annotated<A, T>> {
        Edge(PhantomData, ())
    }
}

impl<A> Edge<A> {
    /// Names this handoff for topology inspection and backend policy.
    pub(crate) fn named(self, name: &'static str) -> Edge<Named<A>, &'static str> {
        Edge(PhantomData, name)
    }
}

impl<A> Edge<Named<A>, &'static str> {
    /// Attaches metadata while preserving the edge's name.
    pub(crate) fn annotate<T: Annotation>(self) -> Edge<Named<Annotated<A, T>>, &'static str> {
        Edge(PhantomData, self.1)
    }
}

/// A path and the metadata values used when registering it.
pub(crate) struct Plan<P, Names = ()>(TypeMarker<P>, Names);

/// Describes one operation occurrence of type `N` with annotations `A`.
///
/// Using the same node type twice in a path produces two occurrences in the
/// inspected topology.
pub(crate) struct Node<N, A, Execution, Origin = PreserveOrigin>(
    TypeMarker<(N, A, Execution, Origin)>,
);

/// The descriptor for a single unannotated operation.
pub(crate) type NodePlan<N, Execution = NoInstruction, Origin = PreserveOrigin> =
    Plan<Node<N, Unannotated, Execution, Origin>>;

/// Turns an operation type into a plan that can participate in the graph.
pub(in crate::framework) fn node<N, Execution, Origin>() -> NodePlan<N, Execution, Origin> {
    Plan(PhantomData, ())
}

impl<N, A, Execution, Origin, Names> Plan<Node<N, A, Execution, Origin>, Names> {
    /// Attaches a metadata tag to the node.
    #[allow(dead_code, reason = "used by the checked program driver examples")]
    pub(crate) fn annotate<T: Annotation>(
        self,
    ) -> Plan<Node<N, Annotated<A, T>, Execution, Origin>, Names> {
        Plan(PhantomData, self.1)
    }
}

impl<N, A, Execution, Origin> Plan<Node<N, A, Execution, Origin>> {
    /// Names this operation occurrence in the topology.
    pub(crate) fn named(
        self,
        name: &'static str,
    ) -> Plan<Node<N, Named<A>, Execution, Origin>, &'static str> {
        Plan(PhantomData, name)
    }
}

/// Connects the output of path `L` to the input of path `R`.
///
/// `E` describes the edge's annotations. The execution interpreters check
/// that the payload types match.
pub(crate) struct Sequence<L, E, R>(TypeMarker<(L, E, R)>);

impl<L, Names> Plan<L, Names> {
    /// Connects the next path through an unannotated edge.
    pub(crate) fn then<R: PathPlan>(
        self,
        next: R,
    ) -> Connected<L, Unannotated, R::Path, Names, (), R::Names> {
        self.then_via(edge(), next)
    }

    /// Connects the next path through an explicitly configured edge.
    pub(crate) fn then_via<E, EdgeNames, R: PathPlan>(
        self,
        connection: Edge<E, EdgeNames>,
        next: R,
    ) -> Connected<L, E, R::Path, Names, EdgeNames, R::Names> {
        Plan(PhantomData, (self.1, connection.1, next.into_names()))
    }
}

type Connected<L, E, R, Left, Link, Right> = Plan<Sequence<L, E, R>, (Left, Link, Right)>;

/// A declaration that can be appended to an existing path.
pub(crate) trait PathPlan {
    type Path;
    type Names;

    fn into_names(self) -> Self::Names;
}

impl<P, Names> PathPlan for Plan<P, Names> {
    type Path = P;
    type Names = Names;

    fn into_names(self) -> Names {
        self.1
    }
}

/// Accumulates one continuation per element of the split payload.
pub(crate) struct Fork<S, Branches, Names = ()>(TypeMarker<(S, Branches)>, Names);

/// Starts a fork whose input node supplies a tuple of branch payloads.
pub(crate) fn split_with<S, Names>(plan: Plan<S, Names>) -> Fork<S, (), Names> {
    Fork(PhantomData, plan.1)
}

/// Appends an arm to a type-only branch tuple.
pub(crate) trait AppendBranch<Next> {
    type Branches;
}

impl<Next> AppendBranch<Next> for () {
    type Branches = (Next,);
}

impl<A, Next> AppendBranch<Next> for (A,) {
    type Branches = (A, Next);
}

macro_rules! append_branch {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<$($branch,)+ Next> AppendBranch<Next> for ($($branch,)+) {
            type Branches = ($($branch,)+ Next);
        }
    };
}
for_each_branch_tuple!(append_branch);

impl<S, Branches, Names> Fork<S, Branches, Names> {
    /// Sends the next tuple element into this branch.
    pub(crate) fn then<P: PathPlan>(self, path: P) -> Fork<S, Branches::Branches, (Names, P::Names)>
    where
        Branches: AppendBranch<P::Path>,
    {
        Fork(PhantomData, (self.1, path.into_names()))
    }
}

/// Splits an input and returns all branch results in declaration order.
pub(crate) struct ForkJoin<S, Branches>(TypeMarker<(S, Branches)>);

/// Splits an input into terminal delivery branches.
pub(crate) struct DeliverFork<S, Branches>(TypeMarker<(S, Branches)>);

impl<S, Branches, Names> PathPlan for Fork<S, Branches, Names> {
    type Path = DeliverFork<S, Branches>;
    type Names = Names;

    fn into_names(self) -> Names {
        self.1
    }
}

/// Joins the results of a fork for use by the next route node.
pub(crate) fn fork_join<S, Branches, Names>(
    fork: Fork<S, Branches, Names>,
) -> Plan<ForkJoin<S, Branches>, Names> {
    Plan(PhantomData, fork.1)
}

/// Marks a path with no instruction execution.
pub(crate) struct NoInstruction;

/// Marks a path with exactly one instruction execution.
pub(crate) struct OneInstruction;

/// Counts instruction executions in a valid path.
pub(crate) trait InstructionCount {
    type Count;
}

impl<N, A, Execution, Origin> InstructionCount for Node<N, A, Execution, Origin> {
    type Count = Execution;
}

/// Combines the counts of two sequential paths. Two execution sites conflict.
pub(crate) trait CombineCount<R> {
    type Count;
}

impl<R> CombineCount<R> for NoInstruction {
    type Count = R;
}

impl CombineCount<NoInstruction> for OneInstruction {
    type Count = OneInstruction;
}

impl<L, E, R> InstructionCount for Sequence<L, E, R>
where
    L: InstructionCount,
    R: InstructionCount,
    L::Count: CombineCount<R::Count>,
{
    type Count = <L::Count as CombineCount<R::Count>>::Count;
}

impl<S: InstructionCount<Count = NoInstruction>, B: BranchPaths> InstructionCount
    for ForkJoin<S, B>
{
    type Count = NoInstruction;
}

impl<S: InstructionCount<Count = NoInstruction>, B: BranchPaths> InstructionCount
    for DeliverFork<S, B>
{
    type Count = NoInstruction;
}

/// A supported branch tuple with no designated instruction execution.
pub(crate) trait BranchPaths {}

macro_rules! branch_paths {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<$($branch: InstructionCount<Count = NoInstruction>,)+> BranchPaths for ($($branch,)+) {}
    };
}
for_each_branch_tuple!(branch_paths);

/// No single field is known to have produced the payload.
pub(crate) struct NoOrigin;

/// A payload produced by field `F`.
pub(crate) struct FieldOrigin<F>(TypeMarker<F>);

/// A node such as a wait preserves the provenance of the incoming payload.
pub(crate) struct PreserveOrigin;

/// A field operation attributes its output to the field that produced it.
pub(crate) struct EmitsFrom<F>(TypeMarker<F>);

/// A composite effect node needs the input's field to select its implementation.
pub(crate) struct BindOrigin;

pub(crate) trait ResolveOperation<N, Incoming> {
    type Operation;
    type Origin;
}

impl<N, Incoming> ResolveOperation<N, Incoming> for PreserveOrigin {
    type Operation = N;
    type Origin = Incoming;
}

impl<N, Incoming, F> ResolveOperation<N, Incoming> for EmitsFrom<F> {
    type Operation = N;
    type Origin = FieldOrigin<F>;
}

/// Resolves each composite node's source from the topology before execution.
pub(crate) trait ResolveOrigins<Incoming> {
    type Path;
    type Origin;
    type Exit;
}

pub(crate) struct SingleExit;
pub(crate) struct TerminalDelivery;

impl<N, A, Execution, Flow, Incoming> ResolveOrigins<Incoming> for Node<N, A, Execution, Flow>
where
    Flow: ResolveOperation<N, Incoming>,
{
    type Path = Node<Flow::Operation, A, Execution>;
    type Origin = Flow::Origin;
    type Exit = SingleExit;
}

impl<L, E, R, Incoming> ResolveOrigins<Incoming> for Sequence<L, E, R>
where
    L: ResolveOrigins<Incoming, Exit = SingleExit>,
    R: ResolveOrigins<L::Origin>,
{
    type Path = Sequence<L::Path, E, R::Path>;
    type Origin = R::Origin;
    type Exit = R::Exit;
}

impl<S, B, Incoming> ResolveOrigins<Incoming> for ForkJoin<S, B>
where
    S: ResolveOrigins<Incoming, Exit = SingleExit>,
    B: ResolveJoinedBranches<S::Origin>,
{
    type Path = ForkJoin<S::Path, B::Paths>;
    type Origin = NoOrigin;
    type Exit = SingleExit;
}

impl<S, B, Incoming> ResolveOrigins<Incoming> for DeliverFork<S, B>
where
    S: ResolveOrigins<Incoming, Exit = SingleExit>,
    B: ResolveBranches<S::Origin>,
{
    type Path = DeliverFork<S::Path, B::Paths>;
    type Origin = NoOrigin;
    type Exit = TerminalDelivery;
}

pub(crate) trait ResolveBranches<Origin> {
    type Paths;
}

pub(crate) trait ResolveJoinedBranches<Origin> {
    type Paths;
}

macro_rules! resolve_branches {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<Origin, $($branch: ResolveOrigins<Origin>,)+> ResolveBranches<Origin> for ($($branch,)+) {
            type Paths = ($($branch::Path,)+);
        }

        impl<Origin, $($branch: ResolveOrigins<Origin, Exit = SingleExit>,)+> ResolveJoinedBranches<Origin>
            for ($($branch,)+)
        {
            type Paths = ($($branch::Path,)+);
        }
    };
}
for_each_branch_tuple!(resolve_branches);

/// Carries the selected instruction type `I` until `begin` supplies an entry path.
pub(crate) struct Start<I>(PhantomData<fn() -> I>);

/// Accumulates the path `P` for instruction `I`.
pub(crate) struct Step<I, P, Names>(TypeMarker<(I, P)>, Names);

/// Names the builder result after appending path `R` to `L` through edge `E`.
pub(crate) type ExtendedStep<I, L, E, R, Left, Link, Right> =
    Step<I, Sequence<L, E, R>, (Left, Link, Right)>;

/// Starts describing a step for instruction type `I`.
pub(crate) fn step_graph<I>() -> Start<I> {
    Start(PhantomData)
}

impl<I> Start<I> {
    /// Installs the entry path.
    pub(crate) fn begin<P, Names>(self, plan: Plan<P, Names>) -> Step<I, P, Names> {
        Step(PhantomData, plan.1)
    }
}

impl<I, P, Names> Step<I, P, Names> {
    /// Connects the next path through an unannotated edge.
    pub(crate) fn then<R: PathPlan>(
        self,
        next: R,
    ) -> ExtendedStep<I, P, Unannotated, R::Path, Names, (), R::Names> {
        self.then_via(edge(), next)
    }

    /// Connects the next path through an explicitly configured edge.
    pub(crate) fn then_via<E, EdgeNames, R: PathPlan>(
        self,
        connection: Edge<E, EdgeNames>,
        next: R,
    ) -> ExtendedStep<I, P, E, R::Path, Names, EdgeNames, R::Names> {
        Step(PhantomData, (self.1, connection.1, next.into_names()))
    }
}

/// Supplies names in the order in which the topology visits their occurrences.
pub(crate) trait CollectNames {
    fn collect(self, names: &mut Vec<&'static str>);
}

impl CollectNames for () {
    fn collect(self, _: &mut Vec<&'static str>) {}
}

impl CollectNames for &'static str {
    fn collect(self, names: &mut Vec<&'static str>) {
        names.push(self);
    }
}

impl<L: CollectNames, R: CollectNames> CollectNames for (L, R) {
    fn collect(self, names: &mut Vec<&'static str>) {
        self.0.collect(names);
        self.1.collect(names);
    }
}

impl<L: CollectNames, E: CollectNames, R: CollectNames> CollectNames for (L, E, R) {
    fn collect(self, names: &mut Vec<&'static str>) {
        self.0.collect(names);
        self.1.collect(names);
        self.2.collect(names);
    }
}

/// Finishes executable code or projects the named topology at registration time.
pub(crate) trait RouteDeclaration<I> {
    type Path;

    fn finish(self) -> Route<I, Self::Path>;
    fn registered_topology(self) -> RegisteredTopology
    where
        I: 'static,
        Self::Path: Describe + 'static;
}

impl<I, P, Names: CollectNames> RouteDeclaration<I> for Step<I, P, Names>
where
    P: InstructionCount<Count = OneInstruction> + ResolveOrigins<NoOrigin>,
{
    type Path = P::Path;

    fn finish(self) -> Route<I, Self::Path> {
        Route(PhantomData)
    }

    fn registered_topology(self) -> RegisteredTopology
    where
        I: 'static,
        Self::Path: Describe + 'static,
    {
        let mut names = Vec::new();
        self.1.collect(&mut names);
        RegisteredTopology {
            topology: Topology::describe::<Self::Path>(&mut names.into_iter()),
            route_key: TypeId::of::<(I, Self::Path)>(),
        }
    }
}

impl<I, P, Names> Step<I, P, Names>
where
    P: InstructionCount<Count = OneInstruction> + ResolveOrigins<NoOrigin>,
{
    /// Produces the executable route, with names retained by topology registration.
    pub(crate) fn finish(self) -> Route<I, P::Path> {
        Route(PhantomData)
    }

    /// Finishes a route that runs inline with the fetched instruction borrowed.
    #[cfg(feature = "program-image-execution")]
    pub(crate) fn finish_inline(self) -> InlineRoute<I, P::Path> {
        Route(PhantomData)
    }

    /// Finishes a route that can park and owns its instruction at startup.
    #[cfg(feature = "program-image-execution")]
    pub(crate) fn finish_parkable(self) -> ParkableRoute<I, P::Path> {
        Route(PhantomData)
    }
}

/// A finished zero-sized declaration for one instruction step.
///
/// The type records the instruction `I` and complete path `P`. Inspection
/// projects that path without a machine. Each execution supplies its own inputs
/// and any state needed to resume.
pub(crate) struct Route<I, P, Mode = ()>(TypeMarker<(I, P, Mode)>);

#[cfg(feature = "program-image-execution")]
pub(crate) struct Inline;
#[cfg(feature = "program-image-execution")]
pub(crate) struct Parkable;

#[cfg(feature = "program-image-execution")]
pub(crate) type InlineRoute<I, P> = Route<I, P, Inline>;
#[cfg(feature = "program-image-execution")]
pub(crate) type ParkableRoute<I, P> = Route<I, P, Parkable>;

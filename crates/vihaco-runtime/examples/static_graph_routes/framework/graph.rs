// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Graph constructors build type-level descriptions. Execution and inspection
//! interpret those same descriptions independently.

use super::{
    async_execution::{AsyncExecuteAt, AsyncInstructionAt},
    operations::{ExecuteAt, InstructionAt, Operation},
};
use eyre::Result;
use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
};

/// Represents a type without storing or owning a value of that type.
type TypeMarker<T> = PhantomData<fn() -> T>;

/// Declares an application-defined metadata tag and its static value.
pub(crate) trait Annotation: 'static {
    /// The concrete type recovered by a metadata lookup.
    type Value: Any + Sync;

    /// The static value shared by every occurrence of this annotation.
    const VALUE: &'static Self::Value;
}

/// Erases the value type for inspection while retaining the annotation tag’s identity.
struct MetadataEntry {
    tag: TypeId,
    name: &'static str,
    value: &'static (dyn Any + Sync),
}

/// The annotations projected for one node or edge.
#[derive(Default)]
pub(crate) struct Metadata(Vec<MetadataEntry>);

// Looks up typed annotation values and prepares tag names for display.
impl Metadata {
    /// Returns the most recently attached value for this annotation tag.
    pub(crate) fn get<A: Annotation>(&self) -> Option<&'static A::Value> {
        let entry = self
            .0
            .iter()
            .rev()
            .find(|entry| entry.tag == TypeId::of::<A>())?;
        (entry.value as &dyn Any).downcast_ref()
    }

    /// Returns concise tag names for the diagnostic display.
    fn names(&self) -> Vec<&'static str> {
        self.0.iter().map(|entry| short_name(entry.name)).collect()
    }
}

/// Marks an empty annotation list.
pub(crate) struct Unannotated;

/// Appends annotation type A to an existing type-level annotation list.
pub(crate) struct Annotated<Previous, A>(PhantomData<fn() -> (Previous, A)>);

/// Projects a type-level annotation list into inspectable entries.
pub(crate) trait Annotations {
    /// Appends this list’s entries in declaration order.
    fn collect(metadata: &mut Metadata);

    /// Materializes the annotations for one inspected graph item.
    fn metadata() -> Metadata {
        let mut metadata = Metadata::default();
        Self::collect(&mut metadata);
        metadata
    }
}

// An empty list contributes no entries.
impl Annotations for Unannotated {
    fn collect(_: &mut Metadata) {}
}

// Collects the earlier annotations before adding the current one.
impl<Previous: Annotations, A: Annotation> Annotations for Annotated<Previous, A> {
    fn collect(metadata: &mut Metadata) {
        Previous::collect(metadata);
        metadata.0.push(MetadataEntry {
            tag: TypeId::of::<A>(),
            name: type_name::<A>(),
            value: A::VALUE,
        });
    }
}

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
pub(super) fn node<N>() -> NodePlan<N> {
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

// Composes paths and configures the edges around a fork arm.
impl<L> Plan<L> {
    /// Connects the next path through an unannotated edge.
    pub(crate) fn then<R>(self, next: Plan<R>) -> Plan<Sequence<L, Unannotated, R>> {
        self.then_via(edge(), next)
    }

    /// Connects the next path through an explicitly configured edge.
    pub(crate) fn then_via<E, R>(self, _: Edge<E>, _: Plan<R>) -> Plan<Sequence<L, E, R>> {
        Plan(PhantomData)
    }

    /// Sets the two boundary edges used when this path becomes a fork arm.
    pub(crate) fn fork_edges<I, O>(self, _: Edge<I>, _: Edge<O>) -> Branch<L, I, O> {
        Branch(PhantomData)
    }
}

/// A path with explicitly configured entry and exit edges for use in a fork.
pub(crate) struct Branch<P, I, O>(TypeMarker<(P, I, O)>);

/// Exposes an arm’s path and the metadata on its two boundary edges.
pub(crate) trait ForkArm {
    /// The path between the arm’s two boundary edges.
    type Path;
    /// Metadata on the connection from the split.
    type Incoming: Annotations;
    /// Metadata on the connection into the join.
    type Outgoing: Annotations;
}

// Uses unannotated boundary edges for a plain path.
impl<P> ForkArm for Plan<P> {
    type Path = P;
    type Incoming = Unannotated;
    type Outgoing = Unannotated;
}

// Preserves the metadata from both explicitly configured boundary edges.
impl<P, I: Annotations, O: Annotations> ForkArm for Branch<P, I, O> {
    type Path = P;
    type Incoming = I;
    type Outgoing = O;
}

/// Describes a split operation, two paths, and their join node.
pub(crate) struct ForkJoin<S, L, R, A>(TypeMarker<(S, L, R, A)>);

/// Builds a path that splits one input and pairs the results of its two arms.
pub(crate) fn fork_join<S, L: ForkArm, R: ForkArm>(
    _: Plan<S>,
    _: L,
    _: R,
) -> Plan<ForkJoin<S, L, R, Unannotated>> {
    Plan(PhantomData)
}

// Adds metadata to the join node.
impl<S, L, R, A> Plan<ForkJoin<S, L, R, A>> {
    /// Attaches metadata to the node that waits for both results.
    pub(crate) fn annotate_join<T: Annotation>(self) -> Plan<ForkJoin<S, L, R, Annotated<A, T>>> {
        Plan(PhantomData)
    }
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

// Exposes the same static graph for inspection and execution.
impl<I, P> Route<I, P> {
    /// Materializes the graph using operation types and static annotations.
    pub(crate) fn topology(&self) -> Topology
    where
        P: Describe,
    {
        let mut topology = Topology::default();
        P::describe(None, &mut topology);
        topology
    }

    /// Starts an invocation and reports its terminal result to its caller.
    pub(crate) fn start_with<M, D>(
        &self,
        machine: &mut M,
        driver: &D,
        instruction: I,
        done: impl FnOnce(&mut M, &D, Result<P::Output>) + 'static,
    ) where
        P: Run<M, I, D>,
    {
        P::run(machine, driver, instruction, done);
    }
}

/// Executes a path with a typed input and reports its result through a one-shot callback.
pub(crate) trait Run<M, I, D> {
    /// The owned value passed to the next path when it succeeds.
    type Output: 'static;

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    );
}

impl<M, I, D, N: Operation<M, I, D>, A> Run<M, I, D> for Node<N, A> {
    type Output = N::Output;

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        N::start(machine, driver, input, done);
    }
}

// Starts the right path with the left output or propagates the left failure.
impl<M, I, D, L, E, R> Run<M, I, D> for Sequence<L, E, R>
where
    L: Run<M, I, D>,
    R: Run<M, L::Output, D>,
{
    type Output = R::Output;

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        L::run(
            machine,
            driver,
            input,
            move |machine, driver, result| match result {
                Ok(output) => R::run(machine, driver, output, done),
                Err(error) => done(machine, driver, Err(error)),
            },
        );
    }
}

/// Stores the branch outputs and continuation for one fork invocation.
struct Join<L, R, C> {
    left: Option<L>,
    right: Option<R>,
    done: Option<C>,
}

impl<L, R, C> Join<L, R, C> {
    /// Records a result and releases the continuation once.
    fn arrive<M, D, T>(
        state: &RefCell<Self>,
        machine: &mut M,
        driver: &D,
        result: Result<T>,
        store: impl FnOnce(&mut Self, T),
    ) where
        C: FnOnce(&mut M, &D, Result<(L, R)>),
    {
        let (done, outcome) = {
            let mut state = state.borrow_mut();
            if state.done.is_none() {
                return; // A sibling failure already completed this route.
            }
            match result {
                Err(error) => (state.done.take(), Err(error)),
                Ok(output) => {
                    store(&mut state, output);
                    if state.left.is_none() || state.right.is_none() {
                        return;
                    }
                    (
                        state.done.take(),
                        Ok((
                            state.left.take().expect("left branch completed"),
                            state.right.take().expect("right branch completed"),
                        )),
                    )
                }
            }
        };
        if let Some(done) = done {
            done(machine, driver, outcome);
        }
    }
}

// Runs the split and gives each arm ownership of its input and callback.
impl<M, I, D, LI: 'static, RI: 'static, S, L, R, A> Run<M, I, D> for ForkJoin<S, L, R, A>
where
    S: Run<M, I, D, Output = (LI, RI)>,
    L: ForkArm,
    R: ForkArm,
    L::Path: Run<M, LI, D>,
    R::Path: Run<M, RI, D>,
{
    type Output = (
        <L::Path as Run<M, LI, D>>::Output,
        <R::Path as Run<M, RI, D>>::Output,
    );

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        S::run(machine, driver, input, move |machine, driver, result| {
            let (left, right) = match result {
                Ok(inputs) => inputs,
                Err(error) => return done(machine, driver, Err(error)),
            };
            let join = Rc::new(RefCell::new(Join {
                left: None,
                right: None,
                done: Some(done),
            }));
            let left_join = Rc::clone(&join);
            L::Path::run(machine, driver, left, move |machine, driver, result| {
                Join::arrive(&left_join, machine, driver, result, |join, value| {
                    join.left = Some(value)
                });
            });
            R::Path::run(machine, driver, right, move |machine, driver, result| {
                Join::arrive(&join, machine, driver, result, |join, value| {
                    join.right = Some(value)
                });
            });
        });
    }
}

/// Identifies an operation and any selected field for inspection.
pub(crate) trait Label {
    /// The diagnostic name for an operation occurrence.
    const LABEL: &'static str;

    /// Returns the selected field marker’s type name when the operation targets a field.
    fn field() -> Option<&'static str> {
        None
    }
}

/// Uses the final path segment as a concise diagnostic label.
fn short_name(name: &'static str) -> &'static str {
    name.rsplit("::").next().unwrap_or(name)
}

/// The inspectable projection of node occurrences and directed edges.
#[derive(Default)]
pub(crate) struct Topology {
    /// Nodes in traversal order, indexed by their occurrence IDs.
    pub(crate) nodes: Vec<Vertex>,

    /// Connections between occurrences in this projection.
    pub(crate) edges: Vec<Link>,
}

/// A node occurrence identified by its index in the topology’s node list.
pub(crate) struct Vertex {
    /// The operation’s diagnostic label.
    pub(crate) name: &'static str,

    /// The field marker’s type name for a field-directed operation.
    pub(crate) field: Option<&'static str>,

    /// Metadata attached to this operation occurrence.
    pub(crate) metadata: Metadata,
}

/// A directed edge between two node occurrence indices.
pub(crate) struct Link {
    /// The node occurrence producing the payload.
    pub(crate) from: usize,

    /// The node occurrence receiving the payload.
    pub(crate) to: usize,

    /// Metadata attached to this connection.
    pub(crate) metadata: Metadata,
}

// Assigns occurrence indices and renders the projected graph.
impl Topology {
    /// Adds a node and connects its incoming edge when it has a predecessor.
    fn vertex(
        &mut self,
        name: &'static str,
        field: Option<&'static str>,
        metadata: Metadata,
        from: Option<(usize, Metadata)>,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Vertex {
            name,
            field,
            metadata,
        });
        if let Some((from, metadata)) = from {
            self.edges.push(Link {
                from,
                to: id,
                metadata,
            });
        }
        id
    }

    /// Displays node indices, edge endpoints, and annotation tag names.
    pub(crate) fn print(&self, title: &str) {
        println!("{title}");
        for (id, node) in self.nodes.iter().enumerate() {
            let site = node.field.map(short_name).unwrap_or("");
            println!("  {id}  {} {site} {:?}", node.name, node.metadata.names());
        }
        for link in &self.edges {
            println!("  {} → {}  {:?}", link.from, link.to, link.metadata.names());
        }
    }
}

/// Projects a path into the topology and returns its final node index.
pub(crate) trait Describe {
    /// Projects the path beginning at the supplied predecessor and returns the final node index.
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize;
}

// Projects one operation and its incoming edge when a predecessor exists.
impl<N: Label, A: Annotations> Describe for Node<N, A> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        topology.vertex(N::LABEL, N::field(), A::metadata(), from)
    }
}

// Connects the left path’s last node to the right path’s entry node.
impl<L: Describe, E: Annotations, R: Describe> Describe for Sequence<L, E, R> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        let predecessor = L::describe(from, topology);
        R::describe(Some((predecessor, E::metadata())), topology)
    }
}

// Projects both paths from the split and records both edges into the join.
impl<S, L, R, A> Describe for ForkJoin<S, L, R, A>
where
    S: Describe,
    L: ForkArm,
    R: ForkArm,
    L::Path: Describe,
    R::Path: Describe,
    A: Annotations,
{
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        let split = S::describe(from, topology);
        let left = L::Path::describe(Some((split, L::Incoming::metadata())), topology);
        let right = R::Path::describe(Some((split, R::Incoming::metadata())), topology);
        let join = topology.vertex(
            "join results",
            None,
            A::metadata(),
            Some((left, L::Outgoing::metadata())),
        );
        topology.edges.push(Link {
            from: right,
            to: join,
            metadata: R::Outgoing::metadata(),
        });
        join
    }
}

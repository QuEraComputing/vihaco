// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Proposed vihaco routing API. Computation nodes and payload-transfer edges
//! have separate identities and annotations. A step executes one instruction.

use eyre::Result;
use std::marker::PhantomData;
use vihaco_runtime::{BuildMessage, Execute, Field};

pub(super) trait Receive<E> {
    fn receive(&mut self, effect: E) -> Result<()>;
}

// Nodes, edges, and annotations are represented by types throughout.
pub(super) struct Computation;
pub(super) struct Wire;
pub(super) trait Resume<M, C: ?Sized> {
    type Kind;
    type Input;
    type Operation: Stage<M, Self::Input, Kind = Self::Kind>;
    fn resume(self, machine: &mut M, context: &C) -> Result<()>;
}
// Families identify semantics; concrete bindings retain the exact const values
// or runtime representation in the graph and in the generic submission call.
pub(super) trait AnnotationFamily {
    type Value;
}
pub(super) trait StaticAnnotation {
    type Family: AnnotationFamily;
    fn value() -> <Self::Family as AnnotationFamily>::Value;
}
pub(super) trait RuntimeAnnotation {
    type Family: AnnotationFamily;
    fn into_value(self) -> <Self::Family as AnnotationFamily>::Value;
}
pub(super) trait AnnotationBinding {
    type Family: AnnotationFamily;
    fn into_value(self) -> <Self::Family as AnnotationFamily>::Value;
}
pub(super) trait Context<M, Family: AnnotationFamily, Kind>: Sized {
    fn submit<A, W>(&self, machine: &mut M, annotation: A, work: W) -> Result<()>
    where
        A: AnnotationBinding<Family = Family>,
        W: Resume<M, Self, Kind = Kind> + 'static;
}
pub(super) struct Unannotated;
pub(super) struct Static<A>(PhantomData<fn() -> A>);
pub(super) struct Dynamic<A>(pub(super) A);
impl<A: StaticAnnotation> AnnotationBinding for Static<A> {
    type Family = A::Family;
    fn into_value(self) -> <Self::Family as AnnotationFamily>::Value {
        A::value()
    }
}
impl<A: RuntimeAnnotation> AnnotationBinding for Dynamic<A> {
    type Family = A::Family;
    fn into_value(self) -> <Self::Family as AnnotationFamily>::Value {
        self.0.into_value()
    }
}
pub(super) trait AnnotationInfo {
    type Value;
    const STORAGE: &'static str;
}
impl AnnotationInfo for Unannotated {
    type Value = ();
    const STORAGE: &'static str = "unannotated";
}
impl<A> AnnotationInfo for Static<A> {
    type Value = A;
    const STORAGE: &'static str = "static";
}
impl<A> AnnotationInfo for Dynamic<A> {
    type Value = A;
    const STORAGE: &'static str = "runtime";
}

// Optional inspection visits concrete stage/annotation types directly. The kind
// selects the callback through trait resolution, with no materialized graph tags.
pub(super) trait Visitor<M> {
    fn node<N, Input, A>(&mut self, annotation: &A)
    where
        N: Stage<M, Input, Kind = Computation>,
        A: AnnotationInfo;
    fn edge<N, Input, A>(&mut self, annotation: &A)
    where
        N: Stage<M, Input, Kind = Wire>,
        A: AnnotationInfo;
}
pub(super) trait GraphKind: Sized {
    fn visit<M, Input, N, A, V>(annotation: &A, visitor: &mut V)
    where
        N: Stage<M, Input, Kind = Self>,
        A: AnnotationInfo,
        V: Visitor<M>;
}
impl GraphKind for Computation {
    fn visit<M, Input, N, A, V>(annotation: &A, visitor: &mut V)
    where
        N: Stage<M, Input, Kind = Self>,
        A: AnnotationInfo,
        V: Visitor<M>,
    {
        visitor.node::<N, Input, A>(annotation);
    }
}
impl GraphKind for Wire {
    fn visit<M, Input, N, A, V>(annotation: &A, visitor: &mut V)
    where
        N: Stage<M, Input, Kind = Self>,
        A: AnnotationInfo,
        V: Visitor<M>,
    {
        visitor.edge::<N, Input, A>(annotation);
    }
}

pub(super) trait Policy<M, C, W> {
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()>;
}
impl<M, C, W: Resume<M, C>> Policy<M, C, W> for Unannotated {
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        work.resume(machine, context)
    }
}
impl<M, C, A, W> Policy<M, C, W> for Static<A>
where
    Static<A>: AnnotationBinding,
    C: Context<M, <Static<A> as AnnotationBinding>::Family, W::Kind>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit(machine, self, work)
    }
}
impl<M, C, A, W> Policy<M, C, W> for Dynamic<A>
where
    Dynamic<A>: AnnotationBinding,
    C: Context<M, <Dynamic<A> as AnnotationBinding>::Family, W::Kind>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit(machine, self, work)
    }
}

// Internal typed graph operations. `apply` covers both computation and transfer;
// only ExecuteAt invokes the component's Execute implementation.
pub(super) trait Stage<M, Input> {
    type Kind: GraphKind;
    type Source;
    type Target;
    type Instruction;
    type Payload;
    type Output;
    fn source() -> &'static str;
    fn target() -> &'static str;
    fn apply(self, machine: &mut M, input: Input) -> Result<Self::Output>;
}
pub(super) struct PreparedInput<I, Message> {
    instruction: I,
    message: Message,
}
pub(super) struct BuildAt<F, I>(PhantomData<fn() -> (F, I)>);
impl<M, F, I, Message> Stage<M, I> for BuildAt<F, I>
where
    M: BuildMessage<F, I, (), Message>,
    M::Child: Execute<I, Message = Message>,
{
    type Kind = Computation;
    type Output = PreparedInput<I, Message>;
    type Source = Self;
    type Target = Self;
    type Instruction = I;
    type Payload = Message;
    fn source() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn target() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn apply(self, machine: &mut M, instruction: I) -> Result<Self::Output> {
        let message = machine.build_message(&instruction, &())?;
        Ok(PreparedInput {
            instruction,
            message,
        })
    }
}
pub(super) struct MessageWire<F>(PhantomData<fn() -> F>);
impl<M: Field<F>, F, I, Message> Stage<M, PreparedInput<I, Message>> for MessageWire<F> {
    type Kind = Wire;
    type Output = PreparedInput<I, Message>;
    type Source = BuildAt<F, I>;
    type Target = ExecuteAt<F>;
    type Instruction = I;
    type Payload = Message;
    fn source() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn target() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn apply(self, _: &mut M, input: Self::Output) -> Result<Self::Output> {
        Ok(input)
    }
}
pub(super) struct ExecuteAt<F>(PhantomData<fn() -> F>);
impl<M, F, I, Message> Stage<M, PreparedInput<I, Message>> for ExecuteAt<F>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message>,
    <M::Child as Execute<I>>::Error: Into<eyre::Report>,
{
    type Kind = Computation;
    type Output = <M::Child as Execute<I>>::Effect;
    type Source = Self;
    type Target = Self;
    type Instruction = I;
    type Payload = Self::Output;
    fn source() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn target() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn apply(self, machine: &mut M, input: PreparedInput<I, Message>) -> Result<Self::Output> {
        <M as Field<F>>::get_mut(machine)
            .execute(&input.instruction, input.message)
            .map_err(Into::into)
    }
}
pub(super) struct EffectWire<From, To>(PhantomData<fn() -> (From, To)>);
impl<M: Field<From> + Field<To>, From, To, E> Stage<M, E> for EffectWire<From, To> {
    type Kind = Wire;
    type Output = E;
    type Source = ExecuteAt<From>;
    type Target = HandleAt<To>;
    type Instruction = ();
    type Payload = E;
    fn source() -> &'static str {
        <M as Field<From>>::SOURCE[0]
    }
    fn target() -> &'static str {
        <M as Field<To>>::SOURCE[0]
    }
    fn apply(self, _: &mut M, effect: E) -> Result<E> {
        Ok(effect)
    }
}
pub(super) struct HandleAt<F>(PhantomData<fn() -> F>);
impl<M: Field<F>, F, E> Stage<M, E> for HandleAt<F>
where
    M::Child: Receive<E>,
{
    type Kind = Computation;
    type Output = ();
    type Source = Self;
    type Target = Self;
    type Instruction = ();
    type Payload = E;
    fn source() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn target() -> &'static str {
        <M as Field<F>>::SOURCE[0]
    }
    fn apply(self, machine: &mut M, effect: E) -> Result<()> {
        <M as Field<F>>::get_mut(machine).receive(effect)
    }
}

pub(super) trait Run<M, C, Input> {
    fn run(self, machine: &mut M, context: &C, input: Input) -> Result<()>;
}
pub(super) trait Inspect<M, Input> {
    fn inspect<V: Visitor<M>>(&self, visitor: &mut V);
}
pub(super) struct End;
impl<M, C, Input> Run<M, C, Input> for End {
    fn run(self, _: &mut M, _: &C, _: Input) -> Result<()> {
        Ok(())
    }
}
impl<M, Input> Inspect<M, Input> for End {
    fn inspect<V: Visitor<M>>(&self, _: &mut V) {}
}
pub(super) struct Then<N, P, Next> {
    node: N,
    policy: P,
    next: Next,
}
pub(super) struct Work<N, Input, Next> {
    node: N,
    input: Input,
    next: Next,
}
impl<M, C, N, Input, Next> Resume<M, C> for Work<N, Input, Next>
where
    N: Stage<M, Input>,
    Next: Run<M, C, N::Output>,
{
    type Kind = N::Kind;
    type Input = Input;
    type Operation = N;
    fn resume(self, machine: &mut M, context: &C) -> Result<()> {
        let output = self.node.apply(machine, self.input)?;
        self.next.run(machine, context, output)
    }
}
impl<M, C, N, P, Input, Next> Run<M, C, Input> for Then<N, P, Next>
where
    N: Stage<M, Input>,
    Next: Run<M, C, N::Output>,
    P: Policy<M, C, Work<N, Input, Next>>,
{
    fn run(self, machine: &mut M, context: &C, input: Input) -> Result<()> {
        self.policy.submit(
            machine,
            context,
            Work {
                node: self.node,
                input,
                next: self.next,
            },
        )
    }
}
impl<M, N, P, Input, Next> Inspect<M, Input> for Then<N, P, Next>
where
    N: Stage<M, Input>,
    P: AnnotationInfo,
    Next: Inspect<M, N::Output>,
{
    fn inspect<V: Visitor<M>>(&self, visitor: &mut V) {
        N::Kind::visit::<M, Input, N, P, V>(&self.policy, visitor);
        self.next.inspect(visitor);
    }
}
pub(super) struct FanOut<L, R> {
    left: L,
    right: R,
}
impl<M, C, E: Clone, L: Run<M, C, E>, R: Run<M, C, E>> Run<M, C, E> for FanOut<L, R> {
    fn run(self, machine: &mut M, context: &C, effect: E) -> Result<()> {
        self.left.run(machine, context, effect.clone())?;
        self.right.run(machine, context, effect)
    }
}
impl<M, E, L: Inspect<M, E>, R: Inspect<M, E>> Inspect<M, E> for FanOut<L, R> {
    fn inspect<V: Visitor<M>>(&self, visitor: &mut V) {
        self.left.inspect(visitor);
        self.right.inspect(visitor);
    }
}

// Builder states encode exactly one Build -> message edge -> Execute -> fan-out.
pub(super) struct Through<F>(PhantomData<fn() -> F>);
pub(super) fn route_through<F>() -> Through<F> {
    Through(PhantomData)
}
pub(super) struct Building<F, I, B> {
    build: B,
    types: PhantomData<fn() -> (F, I)>,
}
pub(super) struct Forwarding<F, I, B, T> {
    build: B,
    transfer: T,
    types: PhantomData<fn() -> (F, I)>,
}
pub(super) struct Executing<F, I, B, T, X> {
    build: B,
    transfer: T,
    execution: X,
    types: PhantomData<fn() -> (F, I)>,
}
impl<F> Through<F> {
    pub(super) fn build_message<I>(self) -> Building<F, I, Unannotated> {
        Building {
            build: Unannotated,
            types: PhantomData,
        }
    }
}
impl<F, I> Building<F, I, Unannotated> {
    pub(super) fn annotate<A>(self) -> Building<F, I, Static<A>> {
        Building {
            build: Static(PhantomData),
            types: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Building<F, I, Dynamic<A>> {
        Building {
            build: Dynamic(annotation),
            types: PhantomData,
        }
    }
}
impl<F, I, B> Building<F, I, B> {
    pub(super) fn forward_message(self) -> Forwarding<F, I, B, Unannotated> {
        Forwarding {
            build: self.build,
            transfer: Unannotated,
            types: PhantomData,
        }
    }
}
impl<F, I, B> Forwarding<F, I, B, Unannotated> {
    pub(super) fn annotate<A>(self) -> Forwarding<F, I, B, Static<A>> {
        Forwarding {
            build: self.build,
            transfer: Static(PhantomData),
            types: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Forwarding<F, I, B, Dynamic<A>> {
        Forwarding {
            build: self.build,
            transfer: Dynamic(annotation),
            types: PhantomData,
        }
    }
}
impl<F, I, B, T> Forwarding<F, I, B, T> {
    pub(super) fn execute(self) -> Executing<F, I, B, T, Unannotated> {
        Executing {
            build: self.build,
            transfer: self.transfer,
            execution: Unannotated,
            types: PhantomData,
        }
    }
}
impl<F, I, B, T> Executing<F, I, B, T, Unannotated> {
    pub(super) fn annotate<A>(self) -> Executing<F, I, B, T, Static<A>> {
        Executing {
            build: self.build,
            transfer: self.transfer,
            execution: Static(PhantomData),
            types: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Executing<F, I, B, T, Dynamic<A>> {
        Executing {
            build: self.build,
            transfer: self.transfer,
            execution: Dynamic(annotation),
            types: PhantomData,
        }
    }
}
pub(super) struct Delivering<F, D> {
    delivery: D,
    field: PhantomData<fn() -> F>,
}
pub(super) struct Handling<F, D, H> {
    delivery: D,
    handling: H,
    field: PhantomData<fn() -> F>,
}
pub(super) fn deliver_to<F>() -> Delivering<F, Unannotated> {
    Delivering {
        delivery: Unannotated,
        field: PhantomData,
    }
}
impl<F> Delivering<F, Unannotated> {
    pub(super) fn annotate<A>(self) -> Delivering<F, Static<A>> {
        Delivering {
            delivery: Static(PhantomData),
            field: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Delivering<F, Dynamic<A>> {
        Delivering {
            delivery: Dynamic(annotation),
            field: PhantomData,
        }
    }
}
impl<F, D> Delivering<F, D> {
    pub(super) fn handle_effect(self) -> Handling<F, D, Unannotated> {
        Handling {
            delivery: self.delivery,
            handling: Unannotated,
            field: PhantomData,
        }
    }
}
impl<F, D> Handling<F, D, Unannotated> {
    pub(super) fn annotate<A>(self) -> Handling<F, D, Static<A>> {
        Handling {
            delivery: self.delivery,
            handling: Static(PhantomData),
            field: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Handling<F, D, Dynamic<A>> {
        Handling {
            delivery: self.delivery,
            handling: Dynamic(annotation),
            field: PhantomData,
        }
    }
}
type ReceiverGraph<From, To, D, H> = Then<EffectWire<From, To>, D, Then<HandleAt<To>, H, End>>;
impl<To, D, H> Handling<To, D, H> {
    fn from<From>(self) -> ReceiverGraph<From, To, D, H> {
        Then {
            node: EffectWire(PhantomData),
            policy: self.delivery,
            next: Then {
                node: HandleAt(PhantomData),
                policy: self.handling,
                next: End,
            },
        }
    }
}
type StepGraph<F, I, B, T, X, Next> =
    Then<BuildAt<F, I>, B, Then<MessageWire<F>, T, Then<ExecuteAt<F>, X, Next>>>;
pub(super) struct DeclaredRoute<I, Graph> {
    graph: Graph,
    instruction: PhantomData<fn() -> I>,
}
type ForkedRoute<F, I, B, T, X, L, LD, LH, R, RD, RH> = DeclaredRoute<
    I,
    StepGraph<F, I, B, T, X, FanOut<ReceiverGraph<F, L, LD, LH>, ReceiverGraph<F, R, RD, RH>>>,
>;
impl<F, I, B, T, X> Executing<F, I, B, T, X> {
    pub(super) fn fan_out<L, LD, LH, R, RD, RH>(
        self,
        branches: (Handling<L, LD, LH>, Handling<R, RD, RH>),
    ) -> ForkedRoute<F, I, B, T, X, L, LD, LH, R, RD, RH> {
        DeclaredRoute {
            instruction: PhantomData,
            graph: Then {
                node: BuildAt(PhantomData),
                policy: self.build,
                next: Then {
                    node: MessageWire(PhantomData),
                    policy: self.transfer,
                    next: Then {
                        node: ExecuteAt(PhantomData),
                        policy: self.execution,
                        next: FanOut {
                            left: branches.0.from(),
                            right: branches.1.from(),
                        },
                    },
                },
            },
        }
    }
}
// One canonical graph type contains every operation, connection, and annotation.
// Graph values contain only runtime bindings; static-only graphs are zero-sized.
pub(super) trait Route: Sized {
    type Instruction;
    type Graph;
    fn graph(&self) -> &Self::Graph;
    fn into_graph(self) -> Self::Graph;

    fn start<M, C>(
        self,
        machine: &mut M,
        context: &C,
        instruction: &Self::Instruction,
    ) -> Result<()>
    where
        Self::Instruction: Clone,
        Self::Graph: Run<M, C, Self::Instruction>,
    {
        self.into_graph().run(machine, context, instruction.clone())
    }
    fn inspect<M, V: Visitor<M>>(&self, visitor: &mut V)
    where
        Self::Graph: Inspect<M, Self::Instruction>,
    {
        self.graph().inspect(visitor);
    }
}
impl<I, Graph> Route for DeclaredRoute<I, Graph> {
    type Instruction = I;
    type Graph = Graph;
    fn graph(&self) -> &Graph {
        &self.graph
    }
    fn into_graph(self) -> Graph {
        self.graph
    }
}
pub(super) struct Immediate;
impl<M, Family: AnnotationFamily, Kind> Context<M, Family, Kind> for Immediate {
    fn submit<A, W>(&self, machine: &mut M, _: A, work: W) -> Result<()>
    where
        A: AnnotationBinding<Family = Family>,
        W: Resume<M, Self, Kind = Kind> + 'static,
    {
        work.resume(machine, self)
    }
}

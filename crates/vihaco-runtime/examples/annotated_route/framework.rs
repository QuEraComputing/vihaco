// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Proposed vihaco routing API. Computation nodes and payload-transfer edges
//! have separate identities and annotations. A step executes one instruction.

use eyre::Result;
use std::{any::type_name, marker::PhantomData};
use vihaco_runtime::{BuildMessage, Execute, Field};

pub(super) trait Receive<E> {
    fn receive(&mut self, effect: E) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NodeKind {
    BuildMessage,
    ExecuteInstruction,
    HandleEffect,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EdgeKind {
    MessageTransfer,
    EffectDelivery,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Site {
    pub(super) machine_type: &'static str,
    pub(super) field_type: &'static str,
    pub(super) name: &'static str,
    pub(super) kind: NodeKind,
}
fn site<M: Field<F>, F>(kind: NodeKind) -> Site {
    Site {
        machine_type: type_name::<M>(),
        field_type: type_name::<F>(),
        name: M::SOURCE[0],
        kind,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Element {
    Node {
        site: Site,
        instruction: Option<&'static str>,
        input: &'static str,
        output: &'static str,
    },
    Edge {
        kind: EdgeKind,
        from: Site,
        to: Site,
        payload: &'static str,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Annotation {
    None,
    Static(&'static str),
    Dynamic(&'static str),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Part {
    pub(super) element: Element,
    pub(super) annotation: Annotation,
}

// Interpreters see structural metadata as well as an owned continuation.
pub(super) trait Resume<M, C: ?Sized> {
    fn element(&self) -> Element;
    fn resume(self, machine: &mut M, context: &C) -> Result<()>;
}
pub(super) trait Context<M, A>: Sized {
    fn submit<W>(&self, machine: &mut M, annotation: A, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static;
}
pub(super) struct Unannotated;
pub(super) struct Static<A>(PhantomData<fn() -> A>);
pub(super) struct Dynamic<A>(pub(super) A);
trait AnnotationInfo {
    fn info() -> Annotation;
}
impl AnnotationInfo for Unannotated {
    fn info() -> Annotation {
        Annotation::None
    }
}
impl<A> AnnotationInfo for Static<A> {
    fn info() -> Annotation {
        Annotation::Static(type_name::<A>())
    }
}
impl<A> AnnotationInfo for Dynamic<A> {
    fn info() -> Annotation {
        Annotation::Dynamic(type_name::<A>())
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
    C: Context<M, Static<A>>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit(machine, self, work)
    }
}
impl<M, C, A, W> Policy<M, C, W> for Dynamic<A>
where
    C: Context<M, Dynamic<A>>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit(machine, self, work)
    }
}

// Internal typed graph operations. `apply` covers both computation and transfer;
// only ExecuteAt invokes the component's Execute implementation.
pub(super) trait Stage<M, Input> {
    type Output;
    fn element() -> Element;
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
    type Output = PreparedInput<I, Message>;
    fn element() -> Element {
        Element::Node {
            site: site::<M, F>(NodeKind::BuildMessage),
            instruction: Some(type_name::<I>()),
            input: type_name::<I>(),
            output: type_name::<Message>(),
        }
    }
    fn apply(self, machine: &mut M, instruction: I) -> Result<Self::Output> {
        let message = machine.build_message(&instruction, &())?;
        Ok(PreparedInput {
            instruction,
            message,
        })
    }
}
pub(super) struct MessageTransfer<F>(PhantomData<fn() -> F>);
impl<M: Field<F>, F, I, Message> Stage<M, PreparedInput<I, Message>> for MessageTransfer<F> {
    type Output = PreparedInput<I, Message>;
    fn element() -> Element {
        Element::Edge {
            kind: EdgeKind::MessageTransfer,
            from: site::<M, F>(NodeKind::BuildMessage),
            to: site::<M, F>(NodeKind::ExecuteInstruction),
            payload: type_name::<Message>(),
        }
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
    type Output = <M::Child as Execute<I>>::Effect;
    fn element() -> Element {
        Element::Node {
            site: site::<M, F>(NodeKind::ExecuteInstruction),
            instruction: Some(type_name::<I>()),
            input: type_name::<Message>(),
            output: type_name::<Self::Output>(),
        }
    }
    fn apply(self, machine: &mut M, input: PreparedInput<I, Message>) -> Result<Self::Output> {
        <M as Field<F>>::get_mut(machine)
            .execute(&input.instruction, input.message)
            .map_err(Into::into)
    }
}
pub(super) struct EffectTransfer<From, To>(PhantomData<fn() -> (From, To)>);
impl<M: Field<From> + Field<To>, From, To, E> Stage<M, E> for EffectTransfer<From, To> {
    type Output = E;
    fn element() -> Element {
        Element::Edge {
            kind: EdgeKind::EffectDelivery,
            from: site::<M, From>(NodeKind::ExecuteInstruction),
            to: site::<M, To>(NodeKind::HandleEffect),
            payload: type_name::<E>(),
        }
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
    type Output = ();
    fn element() -> Element {
        Element::Node {
            site: site::<M, F>(NodeKind::HandleEffect),
            instruction: None,
            input: type_name::<E>(),
            output: type_name::<()>(),
        }
    }
    fn apply(self, machine: &mut M, effect: E) -> Result<()> {
        <M as Field<F>>::get_mut(machine).receive(effect)
    }
}

pub(super) trait Run<M, C, Input> {
    fn run(self, machine: &mut M, context: &C, input: Input) -> Result<()>;
}
pub(super) trait Describe<M, Input> {
    fn describe(&self, parts: &mut Vec<Part>);
}
pub(super) struct End;
impl<M, C, Input> Run<M, C, Input> for End {
    fn run(self, _: &mut M, _: &C, _: Input) -> Result<()> {
        Ok(())
    }
}
impl<M, Input> Describe<M, Input> for End {
    fn describe(&self, _: &mut Vec<Part>) {}
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
    fn element(&self) -> Element {
        N::element()
    }
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
impl<M, N, P, Input, Next> Describe<M, Input> for Then<N, P, Next>
where
    N: Stage<M, Input>,
    P: AnnotationInfo,
    Next: Describe<M, N::Output>,
{
    fn describe(&self, parts: &mut Vec<Part>) {
        parts.push(Part {
            element: N::element(),
            annotation: P::info(),
        });
        self.next.describe(parts);
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
impl<M, E, L: Describe<M, E>, R: Describe<M, E>> Describe<M, E> for FanOut<L, R> {
    fn describe(&self, parts: &mut Vec<Part>) {
        self.left.describe(parts);
        self.right.describe(parts);
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
type ReceiverPlan<From, To, D, H> = Then<EffectTransfer<From, To>, D, Then<HandleAt<To>, H, End>>;
impl<To, D, H> Handling<To, D, H> {
    fn from<From>(self) -> ReceiverPlan<From, To, D, H> {
        Then {
            node: EffectTransfer(PhantomData),
            policy: self.delivery,
            next: Then {
                node: HandleAt(PhantomData),
                policy: self.handling,
                next: End,
            },
        }
    }
}
type SourcePlan<F, I, B, T, X, Next> =
    Then<BuildAt<F, I>, B, Then<MessageTransfer<F>, T, Then<ExecuteAt<F>, X, Next>>>;
pub(super) struct Route<I, Plan> {
    plan: Plan,
    instruction: PhantomData<fn() -> I>,
}
type ForkedRoute<F, I, B, T, X, L, LD, LH, R, RD, RH> = Route<
    I,
    SourcePlan<F, I, B, T, X, FanOut<ReceiverPlan<F, L, LD, LH>, ReceiverPlan<F, R, RD, RH>>>,
>;
impl<F, I, B, T, X> Executing<F, I, B, T, X> {
    pub(super) fn fan_out<L, LD, LH, R, RD, RH>(
        self,
        branches: (Handling<L, LD, LH>, Handling<R, RD, RH>),
    ) -> ForkedRoute<F, I, B, T, X, L, LD, LH, R, RD, RH> {
        Route {
            instruction: PhantomData,
            plan: Then {
                node: BuildAt(PhantomData),
                policy: self.build,
                next: Then {
                    node: MessageTransfer(PhantomData),
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
impl<I, Plan> Route<I, Plan> {
    pub(super) fn start<M, C>(self, machine: &mut M, context: &C, instruction: &I) -> Result<()>
    where
        I: Clone,
        Plan: Run<M, C, I>,
    {
        self.plan.run(machine, context, instruction.clone())
    }
    pub(super) fn topology<M>(&self) -> Vec<Part>
    where
        Plan: Describe<M, I>,
    {
        let mut parts = Vec::new();
        self.plan.describe(&mut parts);
        parts
    }
}
pub(super) struct Immediate;
impl<M, A> Context<M, A> for Immediate {
    fn submit<W>(&self, machine: &mut M, _: A, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        work.resume(machine, self)
    }
}

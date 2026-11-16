// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Proposed vihaco API: a typed topology with exactly one instruction execution.
//! Field/instruction/annotation types describe the topology independently of its
//! interpreter. Only owned execution inputs and effects cross deferred boundaries.

use eyre::Result;
use std::marker::PhantomData;
use vihaco_runtime::{BuildMessage, Execute, Field};

pub(super) trait Receive<E> {
    fn receive(&mut self, effect: E) -> Result<()>;
}

pub(super) trait Resume<M, C: ?Sized> {
    fn resume(self, machine: &mut M, context: &C) -> Result<()>;
}
pub(super) trait Context<M, A>: Sized {
    fn submit<W>(&self, machine: &mut M, annotation: A, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static;
}
pub(super) trait StaticContext<M, A>: Sized {
    fn submit_static<W>(&self, machine: &mut M, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static;
}

pub(super) struct Unannotated;
pub(super) struct Static<A>(PhantomData<fn() -> A>);
pub(super) struct Dynamic<A>(A);

// Annotation policies are separate from the topology operation they decorate.
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
    C: StaticContext<M, A>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit_static(machine, work)
    }
}
impl<M, C, A, W> Policy<M, C, W> for Dynamic<A>
where
    C: Context<M, A>,
    W: Resume<M, C> + 'static,
{
    fn submit(self, machine: &mut M, context: &C, work: W) -> Result<()> {
        context.submit(machine, self.0, work)
    }
}

// Propagation nodes have no instruction execution API.
pub(super) struct Branch;
pub(super) struct Delivery<F, P> {
    policy: P,
    field: PhantomData<fn() -> F>,
}
pub(super) fn branch() -> Branch {
    Branch
}
impl Branch {
    pub(super) fn deliver_to<F>(self) -> Delivery<F, Unannotated> {
        Delivery {
            policy: Unannotated,
            field: PhantomData,
        }
    }
}
impl<F> Delivery<F, Unannotated> {
    pub(super) fn annotate<A>(self) -> Delivery<F, Static<A>> {
        Delivery {
            policy: Static(PhantomData),
            field: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Delivery<F, Dynamic<A>> {
        Delivery {
            policy: Dynamic(annotation),
            field: PhantomData,
        }
    }
}

pub(super) trait Propagate<M, C, E> {
    fn propagate(self, machine: &mut M, context: &C, effect: E) -> Result<()>;
}
pub(super) struct DeliveryWork<F, E> {
    effect: E,
    field: PhantomData<fn() -> F>,
}
impl<M, C, F, E> Resume<M, C> for DeliveryWork<F, E>
where
    M: Field<F>,
    M::Child: Receive<E>,
{
    fn resume(self, machine: &mut M, _: &C) -> Result<()> {
        <M as Field<F>>::get_mut(machine).receive(self.effect)
    }
}
impl<M, C, F, E, P> Propagate<M, C, E> for Delivery<F, P>
where
    M: Field<F>,
    M::Child: Receive<E>,
    P: Policy<M, C, DeliveryWork<F, E>>,
{
    fn propagate(self, machine: &mut M, context: &C, effect: E) -> Result<()> {
        self.policy.submit(
            machine,
            context,
            DeliveryWork {
                effect,
                field: PhantomData,
            },
        )
    }
}
pub(super) struct FanOut<L, R> {
    left: L,
    right: R,
}
impl<M, C, E, L, R> Propagate<M, C, E> for FanOut<L, R>
where
    E: Clone,
    L: Propagate<M, C, E>,
    R: Propagate<M, C, E>,
{
    fn propagate(self, machine: &mut M, context: &C, effect: E) -> Result<()> {
        self.left.propagate(machine, context, effect.clone())?;
        self.right.propagate(machine, context, effect)
    }
}

// Typestate: destination -> message preparation -> one Execute -> propagation.
pub(super) struct RouteStart<M>(PhantomData<fn() -> M>);
pub(super) struct At<M, F>(PhantomData<fn() -> (M, F)>);
type Selection<M, F, I> = PhantomData<fn() -> (M, F, I)>;
pub(super) struct Prepared<M, F, I>(Selection<M, F, I>);
pub(super) struct Execution<M, F, I, P> {
    policy: P,
    types: Selection<M, F, I>,
}
pub(super) struct Route<M, F, I, P, Next> {
    policy: P,
    next: Next,
    types: Selection<M, F, I>,
}

pub(super) fn route<M>() -> RouteStart<M> {
    RouteStart(PhantomData)
}
impl<M> RouteStart<M> {
    pub(super) fn at<F>(self) -> At<M, F>
    where
        M: Field<F>,
    {
        At(PhantomData)
    }
}
impl<M, F> At<M, F> {
    pub(super) fn build_message<I>(self) -> Prepared<M, F, I> {
        Prepared(PhantomData)
    }
}
impl<M, F, I> Prepared<M, F, I> {
    pub(super) fn execute(self) -> Execution<M, F, I, Unannotated> {
        Execution {
            policy: Unannotated,
            types: PhantomData,
        }
    }
}
impl<M, F, I> Execution<M, F, I, Unannotated> {
    pub(super) fn annotate<A>(self) -> Execution<M, F, I, Static<A>> {
        Execution {
            policy: Static(PhantomData),
            types: PhantomData,
        }
    }
    pub(super) fn annotate_with<A>(self, annotation: A) -> Execution<M, F, I, Dynamic<A>> {
        Execution {
            policy: Dynamic(annotation),
            types: PhantomData,
        }
    }
}
impl<M, F, I, P> Execution<M, F, I, P> {
    pub(super) fn fan_out<L, R>(self, branches: (L, R)) -> Route<M, F, I, P, FanOut<L, R>> {
        Route {
            policy: self.policy,
            next: FanOut {
                left: branches.0,
                right: branches.1,
            },
            types: PhantomData,
        }
    }
}

pub(super) struct ExecutionWork<F, I, Message, Next> {
    instruction: I,
    message: Message,
    next: Next,
    field: PhantomData<fn() -> F>,
}
impl<M, C, F, I, Message, Next> Resume<M, C> for ExecutionWork<F, I, Message, Next>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message>,
    <M::Child as Execute<I>>::Error: Into<eyre::Report>,
    Next: Propagate<M, C, <M::Child as Execute<I>>::Effect>,
{
    fn resume(self, machine: &mut M, context: &C) -> Result<()> {
        let effect = <M as Field<F>>::get_mut(machine)
            .execute(&self.instruction, self.message)
            .map_err(Into::into)?;
        self.next.propagate(machine, context, effect)
    }
}
impl<M, F, I, P, Next> Route<M, F, I, P, Next> {
    pub(super) fn start<C, Message>(
        self,
        machine: &mut M,
        context: &C,
        instruction: &I,
    ) -> Result<()>
    where
        M: BuildMessage<F, I, (), Message>,
        I: Clone,
        M::Child: Execute<I, Message = Message>,
        <M::Child as Execute<I>>::Error: Into<eyre::Report>,
        Next: Propagate<M, C, <M::Child as Execute<I>>::Effect>,
        P: Policy<M, C, ExecutionWork<F, I, Message, Next>>,
    {
        // The message is a snapshot at Step submission, before deferred execution.
        let message = machine.build_message(instruction, &())?;
        self.policy.submit(
            machine,
            context,
            ExecutionWork {
                instruction: instruction.clone(),
                message,
                next: self.next,
                field: PhantomData,
            },
        )
    }
}

// Structure is inspectable without executing message builders or handlers.
pub(super) trait Describe<M> {
    fn describe(&self, lines: &mut Vec<String>);
}
impl<M, F, P> Describe<M> for Delivery<F, P>
where
    M: Field<F>,
{
    fn describe(&self, lines: &mut Vec<String>) {
        lines.push(format!(
            "deliver effect to {} [{}]",
            M::SOURCE[0],
            std::any::type_name::<P>()
        ));
    }
}
impl<M, L: Describe<M>, R: Describe<M>> Describe<M> for FanOut<L, R> {
    fn describe(&self, lines: &mut Vec<String>) {
        lines.push("fan out".into());
        self.left.describe(lines);
        self.right.describe(lines);
    }
}
impl<M, F, I, P, Next> Route<M, F, I, P, Next>
where
    M: Field<F>,
    Next: Describe<M>,
{
    pub(super) fn topology(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "build message for {} at {}",
                std::any::type_name::<I>(),
                M::SOURCE[0]
            ),
            format!(
                "execute instruction at {} [{}]",
                M::SOURCE[0],
                std::any::type_name::<P>()
            ),
        ];
        self.next.describe(&mut lines);
        lines
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
impl<M, A> StaticContext<M, A> for Immediate {
    fn submit_static<W>(&self, machine: &mut M, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        work.resume(machine, self)
    }
}

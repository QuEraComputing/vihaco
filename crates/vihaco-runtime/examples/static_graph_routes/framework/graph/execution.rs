// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Drive a typed path with callbacks that may run inline or park.
//!
//! A sequence passes the left output to the right. A fork starts the left arm
//! before the remaining arms. The join holds branch results until all succeed
//! and calls its continuation once.
//! A failure stops the join, while already-started work may finish.
//!
//! Unannotated nodes dispatch directly to the operation. An annotated
//! operation instead passes its metadata to the driver's `run_annotated`
//! hook. Edge metadata is used by inspection. The synthetic join needs
//! neither an authored operation nor a driver hook.

use super::{
    annotations::{Annotated, Annotation, Annotations, Metadata, Named, Unannotated},
    builder::{DeliverFork, ForkJoin, Node, Route, Sequence},
    inspection::EdgeId,
};
use crate::framework::operations::Operation;
use eyre::Result;
use std::{cell::RefCell, rc::Rc};

impl<I, P, Mode> Route<I, P, Mode> {
    /// Starts an invocation and reports its terminal result to its caller.
    ///
    /// The terminal callback can run before this method returns or after a
    /// driver delivers pending work.
    pub(crate) fn start_with<M, Input, D>(
        &self,
        machine: &mut M,
        driver: &D,
        input: Input,
        done: impl FnOnce(&mut M, &D, Result<P::Output>) + 'static,
    ) where
        P: Run<M, Input, D>,
    {
        P::run(machine, driver, input, done);
    }
}

/// Interprets a path for a machine `M`, input `I`, and driver `D`.
///
/// Bounds on composed paths check payload compatibility and require only
/// the driver capabilities their operations use.
pub(crate) trait Run<M, I, D> {
    /// The value passed to the next path when it succeeds.
    type Output;

    /// Number of indexed handoffs in this path, in topology order.
    const NAMED_EDGES: usize = 0;

    /// Takes ownership of the input and passes the terminal result to `done`.
    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        Self::run_at(machine, driver, input, 0, done);
    }

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    );
}

/// Lets the driver deliver owned results immediately or hold them for later.
pub(crate) trait RouteDelivery<M>: Sized {
    /// Starts an operation and optionally controls when its result is delivered.
    fn run_annotated<I, N: Operation<M, I, Self, Output: 'static>>(
        &self,
        machine: &mut M,
        _: &Metadata,
        input: I,
        done: impl FnOnce(&mut M, &Self, Result<N::Output>) + 'static,
    ) {
        N::start(machine, self, input, done);
    }

    /// Delivers the owned payload of a named handoff.
    fn deliver_edge<E: 'static>(
        &self,
        machine: &mut M,
        _: EdgeId,
        effect: E,
        done: impl FnOnce(&mut M, &Self, Result<E>) + 'static,
    ) {
        done(machine, self, Ok(effect));
    }
}

/// Executes an ordinary operation directly.
impl<M, I, D, N: Operation<M, I, D>, Execution> Run<M, I, D> for Node<N, Unannotated, Execution> {
    type Output = N::Output;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        _: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        N::start(machine, driver, input, done);
    }
}

/// Gives the driver a chance to apply policy to an annotated operation.
impl<M, I, D, N, Previous, A, Execution> Run<M, I, D> for Node<N, Annotated<Previous, A>, Execution>
where
    D: RouteDelivery<M>,
    N: Operation<M, I, D, Output: 'static>,
    Previous: Annotations,
    A: Annotation,
{
    type Output = N::Output;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        _: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        driver.run_annotated::<I, N>(machine, &Annotated::<Previous, A>::metadata(), input, done);
    }
}

impl<M, I, D, N, A, Execution> Run<M, I, D> for Node<N, Named<A>, Execution>
where
    Node<N, A, Execution>: Run<M, I, D>,
{
    type Output = <Node<N, A, Execution> as Run<M, I, D>>::Output;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        Node::<N, A, Execution>::run_at(machine, driver, input, edge_offset, done);
    }
}

trait RunEdge<M, Payload, D> {
    const COUNT: usize = 0;

    fn deliver(
        machine: &mut M,
        driver: &D,
        _: EdgeId,
        payload: Payload,
        done: impl FnOnce(&mut M, &D, Result<Payload>) + 'static,
    ) {
        done(machine, driver, Ok(payload));
    }
}

impl<M, Payload, D> RunEdge<M, Payload, D> for Unannotated {}
impl<M, Payload, D, Previous, A> RunEdge<M, Payload, D> for Annotated<Previous, A> {}

impl<M, Payload: 'static, D: RouteDelivery<M>, A> RunEdge<M, Payload, D> for Named<A> {
    const COUNT: usize = 1;

    fn deliver(
        machine: &mut M,
        driver: &D,
        id: EdgeId,
        payload: Payload,
        done: impl FnOnce(&mut M, &D, Result<Payload>) + 'static,
    ) {
        driver.deliver_edge(machine, id, payload, done);
    }
}

/// Forwards the left output through the connecting edge and starts the right path.
impl<M, I, D, L, E, R> Run<M, I, D> for Sequence<L, E, R>
where
    L: Run<M, I, D>,
    E: RunEdge<M, L::Output, D>,
    R: Run<M, L::Output, D>,
{
    type Output = R::Output;
    const NAMED_EDGES: usize = L::NAMED_EDGES + E::COUNT + R::NAMED_EDGES;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let edge = EdgeId(edge_offset + L::NAMED_EDGES);
        L::run_at(
            machine,
            driver,
            input,
            edge_offset,
            move |machine, driver, result| match result {
                Ok(output) => E::deliver(
                    machine,
                    driver,
                    edge,
                    output,
                    move |machine, driver, result| match result {
                        Ok(output) => R::run_at(machine, driver, output, edge.0 + E::COUNT, done),
                        Err(error) => done(machine, driver, Err(error)),
                    },
                ),
                Err(error) => done(machine, driver, Err(error)),
            },
        );
    }
}

/// Tracks branch results until every arm succeeds or one fails.
struct Join<Slots, C> {
    slots: Slots,
    done: Option<C>,
}

trait BranchSlots: Default {
    type Output;
    fn take_complete(&mut self) -> Option<Self::Output>;
}

impl<Slots: BranchSlots, C> Join<Slots, C> {
    /// Releases the join's mutable borrow before running a continuation.
    fn arrive<M, D, T>(
        state: &RefCell<Self>,
        machine: &mut M,
        driver: &D,
        result: Result<T>,
        store: impl FnOnce(&mut Slots, T),
    ) where
        C: FnOnce(&mut M, &D, Result<Slots::Output>),
    {
        let (done, outcome) = {
            let mut state = state.borrow_mut();
            if state.done.is_none() {
                return;
            }
            let outcome = match result {
                Err(error) => Err(error),
                Ok(value) => {
                    store(&mut state.slots, value);
                    let Some(outputs) = state.slots.take_complete() else {
                        return;
                    };
                    Ok(outputs)
                }
            };
            (state.done.take().expect("join has a continuation"), outcome)
        };
        done(machine, driver, outcome);
    }
}

pub(crate) trait RunBranches<M, Input, D> {
    type Output: 'static;
    const NAMED_EDGES: usize;

    fn run(
        machine: &mut M,
        driver: &D,
        input: Input,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    );
}

/// A terminal delivery path consumes each branch's effect.
trait UnitOutputs {}

macro_rules! unit {
    ($index:tt) => {
        ()
    };
}

macro_rules! run_branches {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<$($input,)+> BranchSlots for ($(Option<$input>,)+) {
            type Output = ($($input,)+);

            fn take_complete(&mut self) -> Option<Self::Output> {
                if $(self.$index.is_some())&&+ {
                    Some(($(self.$index.take().expect("branch completed"),)+))
                } else {
                    None
                }
            }
        }

        impl UnitOutputs for ($(unit!($index),)+) {}

        impl<M, Driver, $($input: 'static, $branch: Run<M, $input, Driver, Output: 'static>,)+>
            RunBranches<M, ($($input,)+), Driver> for ($($branch,)+)
        {
            type Output = ($($branch::Output,)+);
            const NAMED_EDGES: usize = 0 $(+ $branch::NAMED_EDGES)+;

            fn run(
                machine: &mut M,
                driver: &Driver,
                ($($value,)+): ($($input,)+),
                edge_offset: usize,
                done: impl FnOnce(&mut M, &Driver, Result<Self::Output>) + 'static,
            ) {
                let join = Rc::new(RefCell::new(Join {
                    slots: <($(Option<$branch::Output>,)+)>::default(),
                    done: Some(done),
                }));
                let counts = [$($branch::NAMED_EDGES),+];
                $(
                    // An inline failure has already consumed the continuation.
                    if join.borrow().done.is_none() {
                        return;
                    }
                    let branch = Rc::clone(&join);
                    $branch::run_at(machine, driver, $value, edge_offset + counts[..$index].iter().sum::<usize>(), move |machine, driver, result| {
                        Join::arrive(&branch, machine, driver, result, |slots, value| {
                            slots.$index = Some(value);
                        });
                    });
                )+
            }
        }
    };
}
for_each_branch_tuple!(run_branches);

impl<M, I, D, S, B> Run<M, I, D> for ForkJoin<S, B>
where
    S: Run<M, I, D>,
    B: RunBranches<M, S::Output, D>,
{
    type Output = B::Output;
    const NAMED_EDGES: usize = S::NAMED_EDGES + B::NAMED_EDGES;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        S::run_at(
            machine,
            driver,
            input,
            edge_offset,
            move |machine, driver, result| match result {
                Ok(inputs) => B::run(machine, driver, inputs, edge_offset + S::NAMED_EDGES, done),
                Err(error) => done(machine, driver, Err(error)),
            },
        );
    }
}

impl<M, I, D, S, B> Run<M, I, D> for DeliverFork<S, B>
where
    S: Run<M, I, D>,
    B: RunBranches<M, S::Output, D, Output: UnitOutputs>,
{
    type Output = ();
    const NAMED_EDGES: usize = S::NAMED_EDGES + B::NAMED_EDGES;

    fn run_at(
        machine: &mut M,
        driver: &D,
        input: I,
        edge_offset: usize,
        done: impl FnOnce(&mut M, &D, Result<()>) + 'static,
    ) {
        ForkJoin::<S, B>::run_at(
            machine,
            driver,
            input,
            edge_offset,
            move |machine, driver, result| {
                done(machine, driver, result.map(|_| ()));
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Split;
    struct Fail;
    struct Record;

    impl Run<Vec<&'static str>, (), ()> for Split {
        type Output = ((), ());

        fn run_at(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            _: usize,
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<Self::Output>) + 'static,
        ) {
            done(machine, driver, Ok(((), ())));
        }
    }

    impl Run<Vec<&'static str>, (), ()> for Fail {
        type Output = ();

        fn run_at(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            _: usize,
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<()>) + 'static,
        ) {
            done(machine, driver, Err(eyre::eyre!("left failed")));
        }
    }

    impl Run<Vec<&'static str>, (), ()> for Record {
        type Output = ();

        fn run_at(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            _: usize,
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<()>) + 'static,
        ) {
            machine.push("right started");
            done(machine, driver, Ok(()));
        }
    }

    struct StartDelivery;
    struct SplitDelivery;
    struct RecordDelivery;

    impl<D> Operation<Vec<String>, (), D> for StartDelivery {
        type Output = ();

        fn start(
            machine: &mut Vec<String>,
            driver: &D,
            (): (),
            done: impl FnOnce(&mut Vec<String>, &D, Result<()>) + 'static,
        ) {
            done(machine, driver, Ok(()));
        }
    }

    impl<D> Operation<Vec<String>, (), D> for SplitDelivery {
        type Output = (u8, bool, &'static str);

        fn start(
            machine: &mut Vec<String>,
            driver: &D,
            (): (),
            done: impl FnOnce(&mut Vec<String>, &D, Result<Self::Output>) + 'static,
        ) {
            done(machine, driver, Ok((42, true, "alarm")));
        }
    }

    impl<T: std::fmt::Display, D> Operation<Vec<String>, T, D> for RecordDelivery {
        type Output = ();

        fn start(
            machine: &mut Vec<String>,
            driver: &D,
            input: T,
            done: impl FnOnce(&mut Vec<String>, &D, Result<()>) + 'static,
        ) {
            machine.push(input.to_string());
            done(machine, driver, Ok(()));
        }
    }

    impl super::super::inspection::Label for StartDelivery {
        const LABEL: &'static str = "start delivery";
    }

    impl super::super::inspection::Label for SplitDelivery {
        const LABEL: &'static str = "split delivery";
    }

    impl super::super::inspection::Label for RecordDelivery {
        const LABEL: &'static str = "record delivery";
    }

    #[test]
    fn a_typed_fork_can_deliver_three_branches_without_a_join_node() {
        use super::super::builder::{NodePlan, OneInstruction, node, split_with, step_graph};

        let start: NodePlan<StartDelivery, OneInstruction> = node();
        let split: NodePlan<SplitDelivery> = node();
        let record = || -> NodePlan<RecordDelivery> { node() };
        let route = step_graph::<()>()
            .begin(start)
            .then(
                split_with(split)
                    .then(record())
                    .then(record())
                    .then(record()),
            )
            .finish();

        let topology = route.topology();
        let edges: Vec<_> = topology
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to))
            .collect();
        assert_eq!(edges, [(0, 1), (1, 2), (1, 3), (1, 4)]);
        assert_eq!(topology.nodes.len(), 5);

        let mut deliveries = Vec::new();
        route.start_with(&mut deliveries, &(), (), |deliveries, _, result| {
            result.expect("all deliveries succeeded");
            deliveries.push("complete".to_owned());
        });
        assert_eq!(deliveries, ["42", "true", "alarm", "complete"]);
    }

    #[test]
    fn registered_names_select_distinct_edges_in_reused_branches() {
        use super::super::builder::{
            NodePlan, OneInstruction, RouteDeclaration, edge, node, split_with, step_graph,
        };

        struct RecordEdges;
        impl RouteDelivery<Vec<String>> for RecordEdges {
            fn deliver_edge<E: 'static>(
                &self,
                machine: &mut Vec<String>,
                id: EdgeId,
                effect: E,
                done: impl FnOnce(&mut Vec<String>, &Self, Result<E>) + 'static,
            ) {
                machine.push(format!("edge {}", id.0));
                done(machine, self, Ok(effect));
            }
        }

        let branch = |name| {
            let record: NodePlan<RecordDelivery> = node();
            let finish: NodePlan<StartDelivery> = node();
            record.then_via(edge().named(name), finish)
        };
        let declare = |second| {
            let start: NodePlan<StartDelivery, OneInstruction> = node();
            let split: NodePlan<SplitDelivery> = node();
            let alarm: NodePlan<RecordDelivery> = node();
            step_graph::<()>().begin(start).then(
                split_with(split)
                    .then(branch("first"))
                    .then(branch(second))
                    .then(alarm),
            )
        };

        let registered = declare("second").registered_topology();
        assert_eq!(registered.topology.named_edge("first").unwrap(), EdgeId(0));
        assert_eq!(registered.topology.named_edge("second").unwrap(), EdgeId(1));
        let duplicate = declare("first").registered_topology();
        assert!(duplicate.topology.named_edge("first").is_err());

        let mut deliveries = Vec::new();
        let route = declare("second").finish();
        assert_eq!(std::mem::size_of_val(&route), 0);
        route.start_with(&mut deliveries, &RecordEdges, (), |_, _, result| {
            result.unwrap()
        });
        assert_eq!(deliveries, ["42", "edge 0", "true", "edge 1", "alarm"]);
    }

    #[test]
    fn a_join_keeps_tuple_order_when_branches_complete_out_of_order() {
        use super::super::{Unannotated, builder::NoInstruction};
        use crate::framework::{ChannelDriver, operations::AwaitWork};

        type Arm = Node<AwaitWork, Unannotated, NoInstruction>;
        let driver = ChannelDriver::new();
        let (first, wait_first) = driver.signal();
        let (second, wait_second) = driver.signal();
        let (third, wait_third) = driver.signal();
        let mut results = None;

        <(Arm, Arm, Arm) as RunBranches<_, _, _>>::run(
            &mut results,
            &driver,
            (wait_first, wait_second, wait_third),
            0,
            |results, _, result| *results = Some(result.expect("all three branches succeeded")),
        );
        third.complete(Ok("alarm"));
        second.complete(Ok(true));
        driver.poll_ready(&mut results);
        assert!(results.is_none());

        first.complete(Ok(42_u8));
        driver.poll_ready(&mut results);
        assert_eq!(results, Some((42, true, "alarm")));
    }

    #[test]
    fn inline_left_failure_skips_the_right_arm() {
        let mut events = Vec::new();
        ForkJoin::<Split, (Fail, Record)>::run(&mut events, &(), (), |events, _, result| {
            assert_eq!(
                result.expect_err("left arm fails").to_string(),
                "left failed"
            );
            events.push("done");
        });
        assert_eq!(events, ["done"]);
    }
}

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
    annotations::{Annotated, Annotation, Annotations, Metadata, Unannotated},
    builder::{DeliverFork, ForkJoin, Node, Route, Sequence},
};
use crate::framework::operations::Operation;
use eyre::Result;
use std::{cell::RefCell, rc::Rc};

impl<I, P> Route<I, P> {
    /// Starts an invocation and reports its terminal result to its caller.
    ///
    /// The terminal callback can run before this method returns or after a
    /// driver delivers pending work.
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

/// Interprets a path for a machine `M`, input `I`, and driver `D`.
///
/// Bounds on composed paths check payload compatibility and require only
/// the driver capabilities their operations use.
pub(crate) trait Run<M, I, D> {
    /// The owned value passed to the next path when it succeeds.
    type Output: 'static;

    /// Takes ownership of the input and passes the terminal result to `done`.
    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    );
}

/// Lets a driver interpret an operation node's annotations.
///
/// The default policy invokes the node directly. A backend can override the
/// hook to change when the result is delivered, as the Warehouse simulator does.
pub(crate) trait AnnotatedDriver<M>: Sized {
    /// Starts an operation and optionally controls when its result is delivered.
    fn run_annotated<I, N: Operation<M, I, Self>>(
        &self,
        machine: &mut M,
        _: &Metadata,
        input: I,
        done: impl FnOnce(&mut M, &Self, Result<N::Output>) + 'static,
    ) {
        N::start(machine, self, input, done);
    }
}

/// Dispatches a node according to its type-level annotation list.
///
/// The empty list dispatches directly. A nonempty list projects its metadata
/// and requires the driver to implement `AnnotatedDriver`.
pub(crate) trait RunAnnotations<M, D> {
    /// Starts the operation using this annotation list's dispatch policy.
    fn run<I, N: Operation<M, I, D>>(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<N::Output>) + 'static,
    );
}

/// Starts the operation without materializing metadata or consulting a hook.
impl<M, D> RunAnnotations<M, D> for Unannotated {
    fn run<I, N: Operation<M, I, D>>(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<N::Output>) + 'static,
    ) {
        N::start(machine, driver, input, done);
    }
}

/// Projects the annotation list for the driver's hook.
impl<M, D: AnnotatedDriver<M>, Previous: Annotations, A: Annotation> RunAnnotations<M, D>
    for Annotated<Previous, A>
{
    fn run<I, N: Operation<M, I, D>>(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<N::Output>) + 'static,
    ) {
        driver.run_annotated::<I, N>(machine, &Self::metadata(), input, done);
    }
}

/// Runs one operation using the dispatch policy of its node annotations.
impl<M, I, D, N: Operation<M, I, D>, A: RunAnnotations<M, D>, Execution> Run<M, I, D>
    for Node<N, A, Execution>
{
    type Output = N::Output;

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        A::run::<I, N>(machine, driver, input, done);
    }
}

/// Starts the right path after left succeeds, forwarding the left payload.
/// A left error goes straight to the sequence's continuation.
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

    fn run(
        machine: &mut M,
        driver: &D,
        input: Input,
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

        impl<M, Driver, $($input: 'static, $branch: Run<M, $input, Driver>,)+>
            RunBranches<M, ($($input,)+), Driver> for ($($branch,)+)
        {
            type Output = ($($branch::Output,)+);

            fn run(
                machine: &mut M,
                driver: &Driver,
                ($($value,)+): ($($input,)+),
                done: impl FnOnce(&mut M, &Driver, Result<Self::Output>) + 'static,
            ) {
                let join = Rc::new(RefCell::new(Join {
                    slots: <($(Option<$branch::Output>,)+)>::default(),
                    done: Some(done),
                }));
                $(
                    // An inline failure has already consumed the continuation.
                    if join.borrow().done.is_none() {
                        return;
                    }
                    let branch = Rc::clone(&join);
                    $branch::run(machine, driver, $value, move |machine, driver, result| {
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

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        S::run(
            machine,
            driver,
            input,
            move |machine, driver, result| match result {
                Ok(inputs) => B::run(machine, driver, inputs, done),
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

    fn run(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<()>) + 'static,
    ) {
        ForkJoin::<S, B>::run(machine, driver, input, move |machine, driver, result| {
            done(machine, driver, result.map(|_| ()));
        });
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

        fn run(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<Self::Output>) + 'static,
        ) {
            done(machine, driver, Ok(((), ())));
        }
    }

    impl Run<Vec<&'static str>, (), ()> for Fail {
        type Output = ();

        fn run(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<()>) + 'static,
        ) {
            done(machine, driver, Err(eyre::eyre!("left failed")));
        }
    }

    impl Run<Vec<&'static str>, (), ()> for Record {
        type Output = ();

        fn run(
            machine: &mut Vec<&'static str>,
            driver: &(),
            (): (),
            done: impl FnOnce(&mut Vec<&'static str>, &(), Result<()>) + 'static,
        ) {
            machine.push("right started");
            done(machine, driver, Ok(()));
        }
    }

    struct StartDelivery;
    struct SplitDelivery;
    struct RecordDelivery;

    impl Operation<Vec<String>, (), ()> for StartDelivery {
        type Output = ();

        fn start(
            machine: &mut Vec<String>,
            driver: &(),
            (): (),
            done: impl FnOnce(&mut Vec<String>, &(), Result<()>) + 'static,
        ) {
            done(machine, driver, Ok(()));
        }
    }

    impl Operation<Vec<String>, (), ()> for SplitDelivery {
        type Output = (u8, bool, &'static str);

        fn start(
            machine: &mut Vec<String>,
            driver: &(),
            (): (),
            done: impl FnOnce(&mut Vec<String>, &(), Result<Self::Output>) + 'static,
        ) {
            done(machine, driver, Ok((42, true, "alarm")));
        }
    }

    impl<T: std::fmt::Display> Operation<Vec<String>, T, ()> for RecordDelivery {
        type Output = ();

        fn start(
            machine: &mut Vec<String>,
            driver: &(),
            input: T,
            done: impl FnOnce(&mut Vec<String>, &(), Result<()>) + 'static,
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

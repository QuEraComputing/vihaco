// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Drive a typed path with callbacks that may run inline or park.
//!
//! A sequence passes the left output to the right. A fork starts the left arm
//! first, then starts the right if the left has succeeded or parked. The join
//! holds branch results until both succeed and calls its continuation once.
//! A failure stops the join, while already-started work may finish.
//!
//! Unannotated nodes dispatch directly to the operation. An annotated
//! operation instead passes its metadata to the driver's `run_annotated`
//! hook. Edge metadata is used by inspection. The synthetic join needs
//! neither an authored operation nor a driver hook.

use super::{
    annotations::{Annotated, Annotation, Annotations, Metadata, Unannotated},
    builder::{ForkJoin, Node, Route, Sequence},
};
use crate::framework::operations::Operation;
use eyre::Result;
use std::{cell::RefCell, rc::Rc};

impl<I, P> Route<I, P> {
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

/// Lets a driver interpret annotations while running a node.
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

// The empty list can dispatch directly. Annotated nodes consult the driver.
pub(crate) trait RunAnnotations<M, D> {
    fn run<I, N: Operation<M, I, D>>(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<N::Output>) + 'static,
    );
}

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

impl<M, I, D, N: Operation<M, I, D>, A: RunAnnotations<M, D>> Run<M, I, D> for Node<N, A> {
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
impl<M, I, D, LI: 'static, RI: 'static, S, L, R> Run<M, I, D> for ForkJoin<S, L, R>
where
    S: Run<M, I, D, Output = (LI, RI)>,
    L: Run<M, LI, D>,
    R: Run<M, RI, D>,
{
    type Output = (L::Output, R::Output);

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
            L::run(machine, driver, left, move |machine, driver, result| {
                Join::arrive(&left_join, machine, driver, result, |join, value| {
                    join.left = Some(value)
                });
            });
            // An inline left failure has already consumed the terminal callback.
            if join.borrow().done.is_none() {
                return;
            }
            R::run(machine, driver, right, move |machine, driver, result| {
                Join::arrive(&join, machine, driver, result, |join, value| {
                    join.right = Some(value)
                });
            });
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

    #[test]
    fn inline_left_failure_skips_the_right_arm() {
        let mut events = Vec::new();
        ForkJoin::<Split, Fail, Record>::run(&mut events, &(), (), |events, _, result| {
            assert_eq!(
                result.expect_err("left arm fails").to_string(),
                "left failed"
            );
            events.push("done");
        });
        assert_eq!(events, ["done"]);
    }
}

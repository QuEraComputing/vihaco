// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Static route descriptions and their interpreters.
//!
//! See `design/annotated-step-routes/framework-guide.md` in the repository
//! for the Warehouse walkthrough and a map of the implementation.

// A parked async instruction can hold an exclusive component lease.
mod async_execution;

// Static graph composition, metadata, and topology inspection.
mod graph;

// Node constructors and the operation traits implemented by consumers.
mod operations;

// Runtime signals and continuations.
mod runtime;

// Inline interpretation for the stdlib example's borrowed program invocations.
#[cfg(feature = "program-image-execution")]
#[allow(dead_code)]
pub(crate) mod synchronous;

pub(crate) use async_execution::{AsyncExecute, AsyncSlot, async_instruction_at};

// The graph builder and the bounds used in route signatures.
pub(crate) use graph::{
    AnnotatedDriver, Annotation, Describe, Metadata, Route, Run, edge, fork_join, step_graph,
};

// Built-in constructors and application extension points.
pub(crate) use operations::{
    Compute, Operation, Receive, await_work, build_message_at, compute, instruction_at,
    receive_effect_at,
};

// The runtime driver and the owned wait handle used by asynchronous nodes.
pub(crate) use runtime::{Await, ChannelDriver, FutureDriver, PollJob, Wait, poll_future};

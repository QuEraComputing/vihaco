// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Static route descriptions and the driver that interprets them.

// Static graph composition, metadata, and topology inspection.
mod graph;

// Node constructors and the operation traits implemented by consumers.
mod operations;

// Runtime signals, workers, and continuations.
mod runtime;

// Inline interpretation for the stdlib example's borrowed program invocations.
#[cfg(feature = "program-image-execution")]
#[allow(dead_code)]
pub(crate) mod synchronous;

// The graph builder and the bounds used in route signatures.
pub(crate) use graph::{Annotation, Describe, Route, Run, edge, fork_join, step_graph};

// Built-in constructors and application extension points.
pub(crate) use operations::{
    Compute, Prepared, Receive, SignalInput, Worker, await_device_completion, await_task,
    build_message_at, compute, instruction_at, receive_effect_at, spawn_task, wait_for_signal,
};

// The runtime driver and the owned wait handle used by asynchronous nodes.
pub(crate) use runtime::{Driver, Wait};

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run a program with ExecutionState and synchronous static-graph steps.
//!
//! cargo run -p vihaco-stdlib --example static_execution_state --features program-image-execution

// This entry point uses the synchronous subset of the shared example framework.
#[allow(dead_code, unused_imports)]
#[path = "../../vihaco-runtime/examples/static_graph_routes/framework/mod.rs"]
mod framework;

#[path = "../../vihaco-runtime/examples/static_graph_routes/machines/execution_state.rs"]
mod machine;

fn main() -> eyre::Result<()> {
    machine::demo()
}

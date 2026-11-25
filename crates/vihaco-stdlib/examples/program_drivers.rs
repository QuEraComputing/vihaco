// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Five ways to drive loaded programs and their routes.
//!
//! Run with `mise run program-drivers`.

#[allow(dead_code, unused_imports)]
#[path = "../../vihaco-runtime/examples/static_graph_routes/framework/mod.rs"]
mod framework;
#[path = "../../vihaco-runtime/examples/static_graph_routes/simulation.rs"]
mod simulation;

#[path = "program_drivers/backends.rs"]
mod backends;
#[path = "program_drivers/checked.rs"]
mod checked;
#[path = "static_execution_state/machine.rs"]
mod cpu_machine;
#[path = "program_drivers/machine.rs"]
mod machine;
#[path = "program_drivers/portable/mod.rs"]
mod portable;
#[path = "program_drivers/scenarios.rs"]
mod scenarios;
#[path = "program_drivers/warehouse/mod.rs"]
mod warehouse;

fn main() -> eyre::Result<()> {
    let workers = std::env::args()
        .nth(1)
        .map(|value| value.parse())
        .transpose()?;
    run(workers.unwrap_or(2))
}

fn run(workers: usize) -> eyre::Result<()> {
    cpu_machine::demo()?;
    scenarios::debugger()?;
    scenarios::channel()?;
    warehouse::demo()?;
    portable::demo(workers)
}

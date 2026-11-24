// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Five ways to drive a program through the same checked driver API.
//!
//! Run with `mise run program-drivers`.

#[allow(dead_code, unused_imports)]
#[path = "../../vihaco-runtime/examples/static_graph_routes/framework/mod.rs"]
mod framework;
#[path = "../../vihaco-runtime/examples/static_graph_routes/simulation.rs"]
mod simulation;
#[path = "../../vihaco-runtime/examples/static_graph_routes/machines/async_capture/tokio_driver.rs"]
mod tokio_driver;

#[path = "program_drivers/backends.rs"]
mod backends;
#[path = "program_drivers/checked.rs"]
mod checked;
#[path = "static_execution_state/machine.rs"]
mod cpu_machine;
#[path = "program_drivers/machine.rs"]
mod machine;
#[path = "program_drivers/scenarios.rs"]
mod scenarios;
#[path = "program_drivers/tokio_threads.rs"]
mod tokio_threads;
#[path = "program_drivers/warehouse/mod.rs"]
mod warehouse;

fn main() -> eyre::Result<()> {
    cpu_machine::demo()?;
    scenarios::debugger()?;
    scenarios::channel()?;
    warehouse::demo()?;
    scenarios::tokio_threads()
}

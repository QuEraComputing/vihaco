// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Machines built from static, inspectable route descriptions.
//!
//! cargo run -p vihaco-runtime --example static_graph_routes --features typed-declarations

#[path = "static_graph_routes/framework/mod.rs"]
mod framework;
#[path = "static_graph_routes/machines/mod.rs"]
mod machines;

fn main() -> eyre::Result<()> {
    machines::counter::demo()?;
    machines::waveform::demo()?;
    machines::capture::demo()?;
    machines::async_capture::demo()?;
    machines::warehouse::demo()
}

#[cfg(test)]
mod tests {
    use super::machines;

    #[test]
    fn simulated_printer_completes_at_scheduled_tick() -> eyre::Result<()> {
        machines::warehouse::demo()
    }

    #[test]
    fn linear_counter_step() -> eyre::Result<()> {
        machines::counter::demo()
    }

    #[test]
    fn annotated_waveform_delivery() -> eyre::Result<()> {
        machines::waveform::demo()
    }

    #[test]
    fn reuse_capture_topology_with_fresh_signals() -> eyre::Result<()> {
        machines::capture::demo()
    }
}

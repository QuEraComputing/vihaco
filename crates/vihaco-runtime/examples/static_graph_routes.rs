// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Machines built from static, inspectable route descriptions.
//!
//! mise run static-graph-routes
//! cargo run -p vihaco-runtime --example static_graph_routes --features typed-declarations

#[path = "static_graph_routes/framework/mod.rs"]
mod framework;
#[path = "static_graph_routes/machines/mod.rs"]
mod machines;
#[path = "static_graph_routes/simulation.rs"]
mod simulation;

fn main() -> eyre::Result<()> {
    machines::waveform::demo()?;
    machines::device_completion::demo()?;
    machines::async_capture::demo()?;
    machines::warehouse::demo()
}

#[cfg(test)]
mod tests {
    use super::machines;

    #[test]
    fn waveform_routes_clamped_and_original_samples() -> eyre::Result<()> {
        machines::waveform::demo()
    }

    #[test]
    fn device_wait_resumes_without_reissuing_capture() -> eyre::Result<()> {
        machines::device_completion::demo()
    }

    #[test]
    fn warehouse_waits_for_each_label_before_verifying_it() -> eyre::Result<()> {
        machines::warehouse::demo()
    }
}

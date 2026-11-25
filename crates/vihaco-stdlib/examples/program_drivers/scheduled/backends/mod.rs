// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub(crate) mod sim;
pub(crate) mod threaded;

use super::tasks::{Submit, TaskId};
use eyre::Result;
use tokio::sync::oneshot;

/// A device input for the demo. The two backends use virtual ticks and wall-clock
/// milliseconds, respectively. The digitizer receives the same sample type.
pub(crate) struct Input {
    pub(crate) delay: u64,
    pub(crate) sample: u16,
    pub(crate) port: oneshot::Sender<u16>,
}

/// Orders runnable program turns and task completions in one backend.
pub(crate) trait Drive {
    type Submitter: Submit;

    fn submitter(&self) -> Self::Submitter;
    fn enqueue_turn(&self) -> Result<TaskId>;
    fn next_ready(&mut self) -> Result<TaskId>;
}

/// Supplies scripted device input for the demo.
pub(crate) trait DemoInput {
    fn schedule_input(&mut self, input: Input) -> Result<()>;
}

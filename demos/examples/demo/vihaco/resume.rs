// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::execute::StepResult;

/// A component resumes a previously parked operation from an owned completion.
pub trait Resume<C> {
    type Effect;
    type Fault;

    fn resume(&mut self, completion: C) -> Result<StepResult<Self::Effect>, Self::Fault>;
}

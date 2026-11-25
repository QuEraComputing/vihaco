// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Timing metadata shared by route backends.

#[cfg(feature = "program-image-execution")]
use crate::framework::Annotation;
use crate::framework::Metadata;

/// Minimum time from starting the node to delivering its successful result.
#[cfg(feature = "program-image-execution")]
pub(crate) struct MinExecution<const TICKS: u64>;

pub(crate) struct ExecutionTicks(u64);

#[cfg(feature = "program-image-execution")]
impl<const TICKS: u64> Annotation for MinExecution<TICKS> {
    type Value = ExecutionTicks;
    const VALUE: &'static ExecutionTicks = &ExecutionTicks(TICKS);
}

pub(crate) fn minimum_execution(metadata: &Metadata) -> u64 {
    metadata
        .get_value::<ExecutionTicks>()
        .map_or(0, |time| time.0)
}

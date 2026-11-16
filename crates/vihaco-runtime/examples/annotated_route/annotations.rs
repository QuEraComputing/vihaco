// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Reusable timing annotations: semantic meaning and amount representation are
//! independent. Static amounts occupy no storage; runtime amounts own a Duration.

use super::framework::{AnnotationFamily, RuntimeAnnotation, StaticAnnotation};
use std::time::Duration;

pub(super) trait TimeAmount {
    fn duration(&self) -> Duration;
}

pub(super) trait StaticTimeAmount: TimeAmount {
    const DURATION: Duration;
}

pub(super) struct ConstNs<const NS: u64>;
impl<const NS: u64> TimeAmount for ConstNs<NS> {
    fn duration(&self) -> Duration {
        Self::DURATION
    }
}
impl<const NS: u64> StaticTimeAmount for ConstNs<NS> {
    const DURATION: Duration = Duration::from_nanos(NS);
}

pub(super) struct RuntimeTime(pub(super) Duration);
impl TimeAmount for RuntimeTime {
    fn duration(&self) -> Duration {
        self.0
    }
}

pub(super) struct ComputeTime<Amount = ()>(pub(super) Amount);
pub(super) struct DeliveryLatency<Amount = ()>(pub(super) Amount);
pub(super) struct TraceLabel(pub(super) &'static str);

// The amount-free forms identify capabilities; no extra family markers are needed.
impl AnnotationFamily for ComputeTime {
    type Value = Duration;
}
impl AnnotationFamily for DeliveryLatency {
    type Value = Duration;
}
impl AnnotationFamily for TraceLabel {
    type Value = &'static str;
}

impl<Amount: StaticTimeAmount> StaticAnnotation for ComputeTime<Amount> {
    type Family = ComputeTime;
    fn value() -> Duration {
        Amount::DURATION
    }
}
impl<Amount: TimeAmount> RuntimeAnnotation for ComputeTime<Amount> {
    type Family = ComputeTime;
    fn into_value(self) -> Duration {
        self.0.duration()
    }
}
impl<Amount: StaticTimeAmount> StaticAnnotation for DeliveryLatency<Amount> {
    type Family = DeliveryLatency;
    fn value() -> Duration {
        Amount::DURATION
    }
}
impl<Amount: TimeAmount> RuntimeAnnotation for DeliveryLatency<Amount> {
    type Family = DeliveryLatency;
    fn into_value(self) -> Duration {
        self.0.duration()
    }
}
impl RuntimeAnnotation for TraceLabel {
    type Family = TraceLabel;
    fn into_value(self) -> &'static str {
        self.0
    }
}

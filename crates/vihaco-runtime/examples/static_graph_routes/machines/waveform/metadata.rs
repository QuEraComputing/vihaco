// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Application-defined static labels for the waveform graph.

use crate::framework::Annotation;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Bus {
    pub(super) name: &'static str,
    pub(super) lanes: u8,
}
pub(super) struct DacBus;
impl Annotation for DacBus {
    type Value = Bus;
    const VALUE: &'static Bus = &Bus {
        name: "pulse",
        lanes: 4,
    };
}
pub(super) struct AuditBus;
impl Annotation for AuditBus {
    type Value = Bus;
    const VALUE: &'static Bus = &Bus {
        name: "audit",
        lanes: 1,
    };
}
pub(super) struct Critical;
impl Annotation for Critical {
    type Value = bool;
    const VALUE: &'static bool = &true;
}
pub(super) struct JoinLabel;
impl Annotation for JoinLabel {
    type Value = &'static str;
    const VALUE: &'static &str = &"both receivers completed";
}

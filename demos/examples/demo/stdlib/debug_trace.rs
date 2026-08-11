// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{
    Effects,
    handle::{Absorb, Observe},
};
use vihaco::{LoadSstSubtree, SstSectionView};

vihaco::component! {
    component DebugTrace {
        pub records: Vec<DebugRecord>,
        pub loaded_section: Option<String>,
    }
}

pub use debug_trace::DebugTrace;

impl debug_trace::DebugTrace {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            loaded_section: None,
        }
    }

    fn record<E: std::fmt::Debug>(&mut self, route: &'static str, effect: &E) {
        self.records.push(DebugRecord {
            route,
            effect: format!("{effect:?}"),
        });
    }

    /// Record an effect produced by a clock-driven component together with its global tick.
    pub fn record_at<E: std::fmt::Debug>(&mut self, tick: u64, effect: &E) {
        self.records.push(DebugRecord {
            route: "clock",
            effect: format!("tick {tick}: {effect:?}"),
        });
    }
}

impl LoadSstSubtree<vihaco::NoContext> for debug_trace::DebugTrace {
    fn load_sst_subtree<'src>(
        &mut self,
        section: SstSectionView<'src, vihaco::NoContext>,
    ) -> eyre::Result<()> {
        self.loaded_section = Some(section.sst().to_owned());
        Ok(())
    }
}

#[derive(Debug)]
pub struct DebugRecord {
    pub route: &'static str,
    pub effect: String,
}

impl<E, R> Observe<E, R> for debug_trace::DebugTrace
where
    E: std::fmt::Debug,
    R: 'static,
{
    type Effect = ();
    type Error = std::convert::Infallible;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error> {
        self.record(std::any::type_name::<R>(), effect);
        Ok(Effects::none())
    }
}

impl<E> Absorb<E> for debug_trace::DebugTrace
where
    E: std::fmt::Debug,
{
    type Fault = std::convert::Infallible;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault> {
        self.record("absorbed", &effect);
        Ok(())
    }
}

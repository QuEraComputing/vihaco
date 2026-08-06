// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{Effects, handle::Observe};

vihaco::component! {
    component DebugTrace {
        pub records: Vec<DebugRecord>,
    }
}

pub use debug_trace::DebugTrace;

impl debug_trace::DebugTrace {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct DebugRecord {
    route: &'static str,
    effect: String,
}

impl<E, R> Observe<E, R> for debug_trace::DebugTrace
where
    E: std::fmt::Debug,
    R: 'static,
{
    type Effect = ();
    type Error = std::convert::Infallible;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error> {
        self.records.push(DebugRecord {
            route: std::any::type_name::<R>(),
            effect: format!("{effect:?}"),
        });
        Ok(Effects::none())
    }
}

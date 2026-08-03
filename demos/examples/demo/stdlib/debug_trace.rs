// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A generic debug component that records every observed effect with its route's type name.
#[derive(Debug, Default)]
struct DebugTrace {
    records: Vec<DebugRecord>,
}

#[derive(Debug)]
struct DebugRecord {
    route: &'static str,
    effect: String,
}

impl<E, R> Observe<E, R> for DebugTrace
where
    E: std::fmt::Debug,
    R: Route<Effect = E>,
{
    type Error = R::Error;

    fn observe(&mut self, effect: &E) -> Result<(), Self::Error> {
        self.records.push(DebugRecord {
            route: std::any::type_name::<R>(),
            effect: format!("{effect:?}"),
        });
        Ok(())
    }
}

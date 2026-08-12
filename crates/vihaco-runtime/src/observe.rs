// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Effects;

/// A non-consuming effect observer selected by a composite-specific route marker.
///
/// The associated effect is routed to nested observers by generated composites.
pub trait Observe<E, R = ()> {
    type Effect;
    type Error;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error>;
}

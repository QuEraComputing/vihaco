// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Handle an effect at its destination and return the next effect.
///
/// Use `()` for a terminal handler and [`std::convert::Infallible`] for an
/// infallible handler. The caller or route chooses the next destination.
pub trait Apply<E> {
    /// The effect emitted after successful application.
    type Effect;
    /// The failure reported by this destination.
    type Error;

    /// Apply the effect and return the next value in the chain.
    ///
    /// # Errors
    ///
    /// Returns the destination's validation or application error. Mutations
    /// already made by this handler or previous handlers are retained.
    fn apply(&mut self, effect: E) -> Result<Self::Effect, Self::Error>;
}

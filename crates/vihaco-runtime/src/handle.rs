// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A reusable, machine-agnostic capability: this component knows how to
/// consume an effect of the specified type.
pub trait Absorb<E> {
    type Fault;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault>;
}

/// Effect handling selected by a composite-specific route marker.
pub trait Handle<E, R> {
    type Error;

    fn handle(&mut self, effect: E) -> Result<(), Self::Error>;
}

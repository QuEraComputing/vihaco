// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A reusable, machine-agnostic capability: this component knows how to swallow this effect.
trait Absorb<E> {
    type Fault;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault>;
}

/// A non-consuming effect observer. Observers borrow effects before their semantic handler
/// consumes them and do not determine the effect's destination.
trait Observe<Effect, Route> {
    type Error;

    fn observe(&mut self, effect: &Effect) -> Result<(), Self::Error>;
}

/// Effect handling, disambiguated by `Route`. The macro normally generates implementations that
/// forward to `Absorb`.
trait Handle<Effect, Route> {
    type Error;

    fn handle(&mut self, effect: Effect) -> Result<(), Self::Error>;
}

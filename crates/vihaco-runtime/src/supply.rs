// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// The dual of [`Absorb`](crate::Absorb): this component knows how to hand
/// out a message of the specified type.
pub trait Supply<M> {
    type Fault;

    fn supply(&mut self) -> Result<M, Self::Fault>;
}

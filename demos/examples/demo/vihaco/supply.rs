// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// The dual of `Absorb`: this component knows how to hand out this message type.
trait Supply<M> {
    type Fault;

    fn supply(&mut self) -> Result<M, Self::Fault>;
}

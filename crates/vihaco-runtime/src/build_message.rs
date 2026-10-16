// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{Execute, Field, NoMessage};

/// Build execution input for one composite field route and instruction type.
///
/// Available with the experimental `typed-declarations` feature. The selected
/// child's [`Execute<I>`] implementation determines the message type. The `M`
/// parameter defaults to that associated type, so consumers can normally write
/// `C: BuildMessage<F, I>`.
///
/// A blanket implementation supplies [`NoMessage`] for every compatible field
/// route and instruction. Consumers only implement builders for messages that
/// require runtime input; they cannot override the no-message builder.
///
/// Specify `M` explicitly in those implementations, such as
/// `BuildMessage<F, Call, FunctionInfo>`. This keeps their trait identities
/// disjoint from the `NoMessage` blanket without relying on cross-crate
/// associated-type normalization during coherence checking. The equality bound
/// still requires `M` to match the child's execution message; it cannot select
/// a different one.
///
/// A [`crate::Step`] implementation can use this optional helper to separate
/// message construction from execution and effect handling. Implementing this
/// trait neither executes an instruction nor supplies a `Step` implementation.
///
/// ```
/// use vihaco_runtime::{BuildMessage, Execute, Field};
///
/// fn prepare<C, F, I>(
///     composite: &mut C,
///     instruction: &I,
/// ) -> vihaco_runtime::eyre::Result<<C::Child as Execute<I>>::Message>
/// where
///     C: BuildMessage<F, I>,
///     C::Child: Execute<I>,
/// {
///     composite.build_message(instruction)
/// }
/// ```
pub trait BuildMessage<F, I, M = <<Self as Field<F>>::Child as Execute<I>>::Message>:
    Field<F>
where
    Self::Child: Execute<I, Message = M>,
{
    /// Resolve and validate the input needed to execute `instruction`.
    ///
    /// The builder may mutate the composite, for example by consuming stack
    /// operands. Mutations made before an error are retained.
    ///
    /// # Errors
    ///
    /// Returns message-preparation failures, such as invalid program references
    /// or missing runtime inputs. Callers can propagate the original report
    /// through [`crate::Step`] before invoking the child.
    fn build_message(&mut self, instruction: &I) -> eyre::Result<M>;
}

impl<C, F, I> BuildMessage<F, I, NoMessage> for C
where
    C: Field<F>,
    C::Child: Execute<I, Message = NoMessage>,
{
    fn build_message(&mut self, _: &I) -> eyre::Result<NoMessage> {
        Ok(NoMessage::new())
    }
}

// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{Execute, Field, NoMessage};

/// Build execution input for one composite field route and instruction type.
///
/// Available with the experimental `typed-declarations` feature. The message
/// type defaults to the selected child's synchronous [`Execute<I>`] message.
/// An explicit message type also supports asynchronous execution. The caller
/// that executes the instruction checks that its message type matches.
/// `Context` defaults to `()` and can carry immutable runner data.
///
/// Composite authors construct the input directly or delegate to a message's
/// [`ReusableMessageRecipe`]. A recipe supplies message construction without
/// implementing this trait on a composite.
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
///     composite.build_message(instruction, &())
/// }
/// ```
pub trait BuildMessage<
    F,
    I,
    Context: ?Sized = (),
    M = <<Self as Field<F>>::Child as Execute<I>>::Message,
>: Field<F>
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
    fn build_message(&mut self, instruction: &I, context: &Context) -> eyre::Result<M>;
}

/// Reusable message construction over machines with specified capabilities.
///
/// Upstream message authors implement this trait generically over `Parent`,
/// requiring capabilities such as [`crate::Field`] or operand-stack access
/// instead of depending on a particular composite type. `Route` identifies the
/// destination device, `I` supplies instruction data, and `Context` supplies
/// shared read-only runtime data. Context may be unsized.
///
/// A recipe does not provide a [`BuildMessage`] implementation. Composite
/// authors choose whether to call `Self::prepare` on the message type from the
/// body of their own `BuildMessage::build_message` implementation, or construct
/// the message using machine-specific logic instead.
///
/// Available with the experimental `typed-declarations` feature. "Reusable"
/// describes the intended use; implementations may also name a concrete parent.
pub trait ReusableMessageRecipe<Parent, Route, I, Context: ?Sized>: Sized {
    /// Construct the message from the parent's capabilities and runtime inputs.
    ///
    /// # Errors
    ///
    /// Returns construction or validation failures. Mutations made before an
    /// error, including successful operand pops, are retained.
    fn prepare(parent: &mut Parent, instruction: &I, context: &Context) -> eyre::Result<Self>;
}

impl<C, F, I, Context: ?Sized> ReusableMessageRecipe<C, F, I, Context> for NoMessage
where
    C: Field<F>,
{
    #[inline]
    fn prepare(_: &mut C, _: &I, _: &Context) -> eyre::Result<Self> {
        Ok(NoMessage::new())
    }
}

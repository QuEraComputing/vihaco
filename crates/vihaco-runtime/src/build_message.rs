// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{Execute, Field, NoMessage};

/// Build execution input for one composite field route and instruction type.
///
/// Available with the experimental `typed-declarations` feature. The selected
/// child's [`Execute<I>`] implementation determines the message type. The `M`
/// parameter defaults to that associated type. `Context` defaults to `()`;
/// runners can instead supply immutable runtime data such as a borrowed module.
/// Consumers can write `C: BuildMessage<F, I, Context>`.
///
/// Standard message types supply framework preparation automatically, including
/// [`NoMessage`] and typed stack inputs. Custom message types can have explicit
/// author-provided builders; authors do not implement the internal preparation
/// hook used by standard messages.
///
/// Specify `M` explicitly in those implementations, such as
/// `BuildMessage<F, Call, (), FunctionInfo>`. This keeps their trait identities
/// disjoint from standard message preparation without relying on cross-crate
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
///     composite.build_message(instruction, &())
/// }
/// ```
pub trait BuildMessage<
    F,
    I,
    Context: ?Sized = (),
    M = <<Self as Field<F>>::Child as Execute<I>>::Message,
>: Field<F> where
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
    fn build_message(&mut self, instruction: &I, context: &Context) -> eyre::Result<M>;
}

/// Internal framework message-preparation hook. Ordinary component authors only
/// select message types. This permits stdlib-owned messages to supply preparation
/// without an orphan impl on every downstream composite.
#[doc(hidden)]
pub trait PrepareMessage<Parent, Route, I, Context: ?Sized>: Sized {
    fn prepare(parent: &mut Parent, instruction: &I, context: &Context) -> eyre::Result<Self>;
}

impl<C, F, I, Context: ?Sized, M> BuildMessage<F, I, Context, M> for C
where
    C: Field<F>,
    C::Child: Execute<I, Message = M>,
    M: PrepareMessage<C, F, I, Context>,
{
    #[inline(always)]
    fn build_message(&mut self, instruction: &I, context: &Context) -> eyre::Result<M> {
        M::prepare(self, instruction, context)
    }
}

impl<C, F, I, Context: ?Sized> PrepareMessage<C, F, I, Context> for NoMessage
where
    C: Field<F>,
    C::Child: Execute<I, Message = NoMessage>,
{
    #[inline]
    fn prepare(_: &mut C, _: &I, _: &Context) -> eyre::Result<Self> {
        Ok(NoMessage::new())
    }
}

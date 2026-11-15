// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The engine's contract with a verifier context.

use crate::diagnostic::VerifyError;

/// The abstract state a verifier context carries, as the engine sees it.
///
/// The engine saves a [`Snapshot`](AbstractState::Snapshot) at every leader,
/// [`restore`](AbstractState::restore)s it before transferring a block,
/// [`snapshot`](AbstractState::snapshot)s the post-state at the block's end,
/// and [`join`](AbstractState::join)s it into each successor's saved state.
///
/// [`FrameState`](crate::FrameState) implements this for the operand stack and
/// locals. A machine-specific context that wraps it joins its own state
/// alongside, for example a set of open channels joined by intersection; that
/// state must also have finite height (widen unbounded values, such as
/// elapsed time, after a fixed number of joins).
pub trait AbstractState {
    /// A saved copy of the state.
    type Snapshot: Clone;

    /// Copy out the current state.
    fn snapshot(&self) -> Self::Snapshot;

    /// Replace the current state with `snapshot`.
    fn restore(&mut self, snapshot: &Self::Snapshot);

    /// Join `incoming` into the state saved at a leader.
    ///
    /// Returns whether `saved` strictly changed, or a structural mismatch
    /// (typically [`DiagnosticKind::StackDepthMismatch`]). On error `saved`
    /// must be left unchanged: the state already recorded stands.
    ///
    /// [`DiagnosticKind::StackDepthMismatch`]: crate::DiagnosticKind::StackDepthMismatch
    fn join(saved: &mut Self::Snapshot, incoming: &Self::Snapshot) -> Result<bool, VerifyError>;

    /// Called before each instruction is verified, with its function and
    /// section-local address.
    fn set_address(&mut self, function: u32, address: u32);
}
